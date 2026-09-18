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
    confirm: undefined as undefined | ((message: string) => boolean | Promise<boolean>),
    player,
  };
  document.body.insertAdjacentHTML("beforeend", view.exportView(context));
  view.bindExportView(context);
  return context;
}

beforeEach(async () => {
  vi.resetModules();
  vi.spyOn(HTMLMediaElement.prototype,"pause").mockImplementation(()=>{});
  document.body.innerHTML = "";
  rpc = vi.fn(async (action: string) => {
    if (action === "duration_summary") {
      return {audio_seconds:2.5,wait_seconds:{normal:.3,part:1.2,chapter:.8,section:.5,head:.1,tail:.3},wait_total_seconds:3.2,total_seconds:5.7,unselected_count:1};
    }
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
    expect(rpc.mock.calls.filter(call => call[0] === "export_settings")).toHaveLength(0);
    document.querySelector<HTMLButtonElement>("#export-output")!.click();await settle();
    const save = rpc.mock.calls.filter(call => call[0] === "export_blocks").at(-1)![1];
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
    expect(saves).toHaveLength(0);
    document.querySelector<HTMLButtonElement>("#export-output")!.click();await settle();
    expect(rpc.mock.calls.filter(call => call[0] === "export_blocks").at(-1)![1].blocks.h1.gaps.normal).toEqual({mode:"manual",value:1.7});
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

it('試聴開始と再描画後も停止でき、下段の重複再生ボタンを出さない',async()=>{
 await mount();vi.spyOn(player,'pause').mockImplementation(()=>{});
 document.querySelector<HTMLButtonElement>('#export-preview')!.click();await settle();
 expect(document.querySelector<HTMLButtonElement>('#export-stop')!.disabled).toBe(false);
 document.querySelector<HTMLElement>('.export-row')!.click();
 expect(document.querySelector<HTMLButtonElement>('#export-stop')!.disabled).toBe(false);
 expect(document.querySelector('.export-timeline')).toBeNull();
 document.querySelector<HTMLButtonElement>('#export-stop')!.click();expect(player.pause).toHaveBeenCalled();
 expect(document.querySelector<HTMLButtonElement>('#export-stop')!.disabled).toBe(true);
});
it('先頭と選択ブロック末尾からの試聴に音量チェックを反映する',async()=>{
 await mount();document.querySelector<HTMLButtonElement>('[data-unit=section]')!.click();
 document.querySelector<HTMLElement>('.export-row[data-block="2"]')!.click();
 const norm=document.querySelector<HTMLInputElement>('#export-normalize')!;norm.checked=false;norm.dispatchEvent(new Event('change'));
 document.querySelector<HTMLButtonElement>('#export-last')!.click();await settle();
 expect(rpc.mock.calls.filter(c=>c[0]==='preview_blocks').at(-1)![1]).toMatchObject({selected_block:2,from_last_item:true,normalize:false});
 document.querySelector<HTMLButtonElement>('#export-first')!.click();await settle();
 expect(rpc.mock.calls.filter(c=>c[0]==='preview_blocks').at(-1)![1]).toMatchObject({selected_block:0,from_last_item:false});
});


it('出力前からフォルダーを開ける',async()=>{
 await mount();document.querySelector<HTMLButtonElement>('#export-folder')!.click();await settle();expect(rpc).toHaveBeenCalledWith('open_export_folder',{directory:'exports'});
});
it('試聴の生成中は明示表示し二重要求を抑止して失敗後に解除する',async()=>{
 let reject!:(e:Error)=>void;rpc.mockImplementation((action:string)=>action==='preview_blocks'?new Promise((_,r)=>reject=r):Promise.resolve(null));
 const context=await mount();document.querySelector<HTMLButtonElement>('#export-preview')!.click();
 expect(document.querySelector('.export-preparation')?.textContent).toContain('生成中');expect(document.querySelector<HTMLButtonElement>('#export-first')!.disabled).toBe(true);
 document.querySelector<HTMLButtonElement>('#export-first')!.click();expect(rpc.mock.calls.filter(c=>c[0]==='preview_blocks')).toHaveLength(1);
 reject(new Error('生成失敗'));await settle();expect(document.querySelector<HTMLElement>('.export-preparation')!.hidden).toBe(true);expect(document.querySelector<HTMLButtonElement>('#export-preview')!.disabled).toBe(false);expect(context.notice).toHaveBeenCalledWith('Error: 生成失敗',true);
});

it('選択ブロックは単独範囲、最初から再生は全体範囲を要求する',async()=>{
 await mount();expect(document.querySelector('#export-preview')?.textContent).toBe('選択ブロックを試聴');
 document.querySelector<HTMLButtonElement>('#export-preview')!.click();await settle();expect(rpc).toHaveBeenCalledWith('preview_blocks',expect.objectContaining({preview_scope:'block',selected_block:0}));
 document.querySelector<HTMLButtonElement>('#export-first')!.click();await settle();expect(rpc).toHaveBeenCalledWith('preview_blocks',expect.objectContaining({preview_scope:'all',selected_block:0}));
});

it('ファイル名を禁止文字除去と20文字制限で表示・保存する',async()=>{
 expect(view.cleanFilenameText(' <>:"/\\|?*\n原稿. ')).toBe('原稿');expect(view.cleanFilenameText('<>:')).toBe('script');expect(view.cleanFilenameText('あ😀'.repeat(20))).toBe('あ😀'.repeat(10));
 const p=project('filename');p.segments[0].source_text='題名?'.repeat(20);await mount(p);
 expect(Array.from(document.querySelector<HTMLInputElement>('#export-name')!.value)).toHaveLength(20);
 const input=document.querySelector<HTMLInputElement>('#export-name')!;input.value='編集:*題名'+ '文'.repeat(30);input.dispatchEvent(new Event('change'));await settle();
 expect(rpc.mock.calls.filter(c=>c[0]==='export_settings')).toHaveLength(0);document.querySelector<HTMLButtonElement>('#export-output')!.click();await settle();const saved=rpc.mock.calls.filter(c=>c[0]==='export_blocks').at(-1)![1].blocks.h1.name;expect(saved).not.toMatch(/[:*]/);expect(Array.from(saved)).toHaveLength(20);expect(document.querySelector<HTMLInputElement>('#export-name')!.value).toBe(saved);
});
it('無音時間は部・章・節・発話・先頭・末尾の順に表示する',async()=>{
 await mount();expect([...document.querySelectorAll<HTMLElement>('[data-global-gap]')].map(e=>e.dataset.globalGap)).toEqual(['part','chapter','section','normal','head','tail']);expect([...document.querySelectorAll<HTMLElement>('[data-gap-mode]')].map(e=>e.dataset.gapMode)).toEqual(['part','chapter','section','normal','head','tail']);expect(document.querySelector('.export-defaults h3')?.textContent).toContain('挿入する無音時間');
});

it('編集中の出力設定で時間を即時に再集計して内訳と未採用件数を表示する',async()=>{
 await mount();await settle();
 expect(document.querySelector('.export-duration')?.textContent).toContain('出力時間の見込み 5.70 秒');
 expect(document.querySelector('.export-duration')?.textContent).toContain('未採用発話 1 件');
 const gap=document.querySelector<HTMLInputElement>('[data-global-gap=normal]')!;gap.value='0.9';gap.dispatchEvent(new Event('change'));await settle();
 expect(rpc.mock.calls.filter(call=>call[0]==='duration_summary').at(-1)?.[1]).toMatchObject({global_gaps:{normal:.9}});
});

it('手動境界から自動へ変更する前に確認しキャンセルでは全境界を保持する',async()=>{
 const context=await mount();context.confirm=vi.fn(async()=>false);
 document.querySelector<HTMLButtonElement>('[data-unit=section]')!.click();
 document.querySelector<HTMLButtonElement>('[data-boundary=h2]')!.click();
 document.querySelector<HTMLButtonElement>('[data-unit=utterance]')!.click();await settle();
 expect(context.confirm).toHaveBeenCalledOnce();
 expect(document.querySelector('[data-unit=custom]')?.getAttribute('aria-pressed')).toBe('true');
 expect(document.querySelector('[data-boundary=h2]')?.textContent).toBe('✂');
 expect(document.querySelector('[data-boundary=h3]')?.textContent).toBe('🔗');
 expect(rpc.mock.calls.filter(call=>call[0]!=="duration_summary")).toHaveLength(0);
});

it('確認待ちに自動分割を適用せず承認後に適用しCtrl+Zで手動境界を復元する',async()=>{
 const context=await mount();let accept!:(value:boolean)=>void;
 context.confirm=vi.fn(()=>new Promise<boolean>(resolve=>accept=resolve));
 document.querySelector<HTMLButtonElement>('[data-unit=section]')!.click();
 document.querySelector<HTMLButtonElement>('[data-boundary=h2]')!.click();
 document.querySelector<HTMLButtonElement>('[data-unit=utterance]')!.click();
 expect(document.querySelector('[data-unit=custom]')?.getAttribute('aria-pressed')).toBe('true');
 expect(document.querySelector<HTMLButtonElement>('[data-unit=all]')!.disabled).toBe(true);
 accept(true);await settle();
 expect(document.querySelector('[data-unit=utterance]')?.getAttribute('aria-pressed')).toBe('true');
 // 再描画後にボタンのフォーカスがbodyへ戻ってもCtrl+Zを使える。
 document.body.dispatchEvent(new KeyboardEvent('keydown',{key:'z',ctrlKey:true,bubbles:true}));
 expect(document.querySelector('[data-unit=custom]')?.getAttribute('aria-pressed')).toBe('true');
 expect(document.querySelector('[data-boundary=h2]')?.textContent).toBe('✂');
 expect(document.querySelector('[data-boundary=h3]')?.textContent).toBe('🔗');
});

it('自動から手動に切り替える際は表示中の境界を保ちボタンから元に戻せる',async()=>{
 await mount();document.querySelector<HTMLButtonElement>('[data-unit=section]')!.click();
 document.querySelector<HTMLButtonElement>('[data-unit=custom]')!.click();
 expect(document.querySelector('[data-boundary=h3]')?.textContent).toBe('🔗');
 document.querySelector<HTMLButtonElement>('#export-undo')!.click();
 expect(document.querySelector('[data-unit=section]')?.getAttribute('aria-pressed')).toBe('true');
 document.querySelector<HTMLButtonElement>('#export-undo')!.click();
 expect(document.querySelector('[data-unit=all]')?.getAttribute('aria-pressed')).toBe('true');
});

it('入力欄のCtrl+Zは通常の文字編集に任せ分割設定を戻さない',async()=>{
 await mount();document.querySelector<HTMLButtonElement>('[data-unit=section]')!.click();
 const event=new KeyboardEvent('keydown',{key:'z',ctrlKey:true,bubbles:true,cancelable:true});
 document.querySelector('#export-name')!.dispatchEvent(event);
 expect(event.defaultPrevented).toBe(false);
 expect(document.querySelector('[data-unit=section]')?.getAttribute('aria-pressed')).toBe('true');
});

it('設定を編集しても正本のオブジェクトを変更せず同じプロジェクトを開き直すと破棄する',async()=>{
 const p=project('draft',{unit:'all',blocks:{h1:{name:'確定名'}}});await mount(p);
 const input=document.querySelector<HTMLInputElement>('#export-name')!;input.value='未確定';input.dispatchEvent(new Event('change'));
 expect(p.export_settings).toEqual({unit:'all',blocks:{h1:{name:'確定名'}}});
 expect(document.querySelector('.export-draft-status')?.textContent).toContain('未出力');
 view.resetExportDraft();await mount(p);
 expect(document.querySelector<HTMLInputElement>('#export-name')!.value).toBe('確定名');
 expect(document.querySelector<HTMLButtonElement>('#export-undo')!.disabled).toBe(true);
});

it('字幕の既定はVTTでなしを選べて音声形式と区別する',async()=>{
 await mount();expect(document.querySelector<HTMLSelectElement>('#export-subtitle-format')!.value).toBe('vtt');
 expect(document.querySelector('#export-format')!.parentElement!.textContent).toContain('音声形式');
 const input=document.querySelector<HTMLSelectElement>('#export-subtitle-format')!;input.value='none';input.dispatchEvent(new Event('change'));
 document.querySelector<HTMLButtonElement>('#export-output')!.click();await settle();
 expect(rpc).toHaveBeenCalledWith('export_blocks',expect.objectContaining({subtitle_format:'none'}));
 expect(document.querySelector('.export-draft-status')?.textContent).toBe('');
});

it('出力失敗時はドラフトと未出力表示を保持し成功した設定だけ開き直した際に残る',async()=>{
 const p=project('output');const context=await mount(p);
 const input=document.querySelector<HTMLSelectElement>('#export-format')!;input.value='wav';input.dispatchEvent(new Event('change'));
 rpc.mockRejectedValueOnce(new Error('出力失敗'));document.querySelector<HTMLButtonElement>('#export-output')!.click();await settle();
 expect(p.export_settings).toBeNull();expect(document.querySelector('.export-draft-status')?.textContent).toContain('未出力');
 expect(context.notice).toHaveBeenCalledWith('Error: 出力失敗',true);
 document.querySelector<HTMLButtonElement>('#export-output')!.click();await settle();
 expect(p.export_settings).toMatchObject({format:'wav'});
 view.resetExportDraft();await mount(p);expect(document.querySelector<HTMLSelectElement>('#export-format')!.value).toBe('wav');
});

it('確認待ちに閉じたプロジェクトの変更を新しいプロジェクトへ反映しない',async()=>{
 const context=await mount();let accept!:(value:boolean)=>void;
 context.confirm=vi.fn(()=>new Promise<boolean>(resolve=>accept=resolve));
 document.querySelector<HTMLButtonElement>('[data-boundary=h2]')!.click();
 document.querySelector<HTMLButtonElement>('[data-unit=utterance]')!.click();
 view.resetExportDraft();await mount(project('other'));accept(true);await settle();
 expect(document.querySelector('[data-unit=all]')?.getAttribute('aria-pressed')).toBe('true');
 expect(document.querySelector('.export-draft-status')?.textContent).toBe('');
});
