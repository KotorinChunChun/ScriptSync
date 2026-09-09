import { beforeEach, describe, expect, it, vi } from "vitest";

type Module = typeof import("../../src/exportView");

const project = (id: string, settings: unknown = null) => ({
  id,
  export_settings: settings,
  segments: [
    { id: "h1", text: "第一部", source_text: "第一部", kind: 1, hierarchy: [1, 0, 0, 0] },
    { id: "text", text: "本文", source_text: "本文", kind: 0, hierarchy: [1, 0, 0, 0] },
    { id: "h2", text: "第一章", source_text: "第一章", kind: 2, hierarchy: [1, 1, 0, 0] },
    { id: "h3", text: "第一節", source_text: "第一節", kind: 3, hierarchy: [1, 1, 1, 0] },
    { id: "item", text: "項目", source_text: "項目", kind: 4, hierarchy: [1, 1, 1, 1] },
  ],
});

async function settle() {
  for (let i = 0; i < 8; i += 1) await Promise.resolve();
}

let view: Module;
let rpc: ReturnType<typeof vi.fn>;
let play: ReturnType<typeof vi.fn>;
let select: ReturnType<typeof vi.fn>;
let player: HTMLAudioElement;

async function mount(value = project("project-a")) {
  document.body.innerHTML = '<audio id="player"></audio>';
  player = document.querySelector<HTMLAudioElement>("#player")!;
  Object.defineProperty(player, "currentTime", { value: 0, writable: true });
  const context = {
    project: value,
    selected: "h1",
    select,
    rpc,
    play,
    notice: vi.fn(),
    player,
  };
  document.body.insertAdjacentHTML("beforeend", view.exportView(context));
  view.bindExportView(context);
  return context;
}

beforeEach(async () => {
  vi.resetModules();
  document.body.innerHTML = "";
  rpc = vi.fn(async (action: string) => {
    if (action === "preview_blocks") {
      return {
        files: [
          { mode: "raw", file: "raw.wav", hash: "raw", frames: 1 },
          { mode: "normalize", file: "normalize.wav", hash: "normal", frames: 1 },
          { mode: "combined", file: "combined.wav", hash: "combined", frames: 1 },
        ],
        timeline: [
          { block: 0, offset_seconds: 0, duration_seconds: 1 },
          { block: 1, offset_seconds: 1, duration_seconds: 1 },
        ],
      };
    }
    return {};
  });
  play = vi.fn(async () => undefined);
  select = vi.fn();
  player = document.createElement("audio");
  Object.defineProperty(player, "currentTime", { value: 0, writable: true });
  view = await import("../../src/exportView");
});

describe("結合・出力ビュー", () => {
  it("プロジェクトIDごとに未保存の設定状態を分離する", async () => {
    await mount(project("alpha"));
    document.querySelector<HTMLSelectElement>("#export-format")!.value = "wav";
    document.querySelector<HTMLSelectElement>("#export-format")!.dispatchEvent(new Event("change"));
    await mount(project("bravo"));
    expect(document.querySelector<HTMLSelectElement>("#export-format")!.value).toBe("flac");
  });

  it("全命名規則で先頭4桁と規則固有の階層表記を出す", async () => {
    await mount();
    const prefix = () => document.querySelector("#export-prefix")!.textContent;
    expect(prefix()).toBe("0001_第1部_");
    document.querySelector<HTMLButtonElement>("[data-naming=none]")!.click();
    expect(prefix()).toBe("0001_");
    document.querySelector<HTMLButtonElement>("[data-naming=number]")!.click();
    expect(prefix()).toBe("0001_01_");
    document.querySelector<HTMLSelectElement>("#export-format")!.value = "wav";
    document.querySelector<HTMLSelectElement>("#export-format")!.dispatchEvent(new Event("change"));
    expect(document.querySelector(".export-filename span")!.textContent).toBe(".wav");
  });

  it("H3分割はH1・H2・H3をすべて境界にする", async () => {
    await mount();
    document.querySelector<HTMLButtonElement>("[data-unit=section]")!.click();
    expect(document.querySelector<HTMLButtonElement>("[data-boundary=h2]")!.textContent).toBe("🔗");
    expect(document.querySelector<HTMLButtonElement>("[data-boundary=h3]")!.textContent).toBe("🔗");
  });

  it("自動境界の解除は他の実効境界を保った手動設定として保存する", async () => {
    await mount();
    document.querySelector<HTMLButtonElement>("[data-unit=section]")!.click();
    document.querySelector<HTMLButtonElement>("[data-boundary=h2]")!.click();
    await settle();
    const save = rpc.mock.calls.filter(call => call[0] === "export_settings").at(-1)![1].settings;
    expect(save.unit).toBe("custom");
    expect(save.boundaries).toEqual(["h3"]);
  });

  it("既定値へ戻した間隔は全体値を表示し、手入力値をその場では復元する", async () => {
    await mount();
    let mode = document.querySelector<HTMLSelectElement>("[data-gap-mode=normal]")!;
    mode.value = "manual";
    mode.dispatchEvent(new Event("change"));
    let input = document.querySelector<HTMLInputElement>("[data-gap-value=normal]")!;
    input.value = "1.7";
    input.dispatchEvent(new Event("change"));
    mode = document.querySelector<HTMLSelectElement>("[data-gap-mode=normal]")!;
    mode.value = "default";
    mode.dispatchEvent(new Event("change"));
    const global = document.querySelector<HTMLInputElement>("[data-global-gap=normal]")!;
    global.value = "0.9";
    global.dispatchEvent(new Event("change"));
    expect(document.querySelector<HTMLInputElement>("[data-gap-value=normal]")!.value).toBe("0.9");
    mode = document.querySelector<HTMLSelectElement>("[data-gap-mode=normal]")!;
    mode.value = "manual";
    mode.dispatchEvent(new Event("change"));
    expect(document.querySelector<HTMLInputElement>("[data-gap-value=normal]")!.value).toBe("1.7");
    await settle();
    const saves = rpc.mock.calls.filter(call => call[0] === "export_settings");
    expect(saves.some(call => call[1].settings.blocks.h1.gaps.normal.mode === "default")).toBe(true);
  });

  it("選択ブロック番号をプレビューRPCへ送る", async () => {
    await mount();
    document.querySelector<HTMLButtonElement>("[data-unit=section]")!.click();
    document.querySelector<HTMLElement>(".export-row[data-block='2']")!.click();
    document.querySelector<HTMLButtonElement>("#export-preview")!.click();
    await settle();
    const call = rpc.mock.calls.find(entry => entry[0] === "preview_blocks")!;
    expect(call[1].selected_block).toBe(2);
    expect(play).toHaveBeenCalledWith("combined.wav", "combined");
  });

  it("連続試聴のtimeupdateで対応ブロックを選択表示する", async () => {
    await mount();
    document.querySelector<HTMLButtonElement>("[data-unit=section]")!.click();
    document.querySelector<HTMLButtonElement>("#export-preview")!.click();
    await settle();
    player.currentTime = 1.2;
    player.dispatchEvent(new Event("timeupdate"));
    expect(document.querySelector<HTMLElement>(".export-row.selected")?.dataset.block).toBe("1");
    expect(document.querySelector<HTMLElement>(".export-row.playing")?.dataset.block).toBe("1");
  });
});
