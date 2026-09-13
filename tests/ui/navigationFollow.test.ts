import { beforeEach, describe, expect, it, vi } from "vitest";
import { createNavigationFollower } from "../../src/navigationFollow";
import { isCollapsed, resetNavigation, setCollapsed } from "../../src/navigation";

function fixture() {
  document.body.innerHTML = `<aside data-navigation-scroll><details data-tree="part"><summary>部</summary><details data-tree="chapter"><summary>章</summary><button data-segment="a">A</button><button data-segment="b">B</button></details></details></aside>`;
  const scroll = document.querySelector<HTMLElement>("aside")!;
  Object.defineProperties(scroll, { clientHeight: { value: 200 }, scrollHeight: { value: 1000 }, clientTop: { value: 0 } });
  scroll.scrollTop = 100;
  scroll.getBoundingClientRect = () => ({ top: 20 } as DOMRect);
  document.querySelectorAll<HTMLElement>("button").forEach(row => row.getBoundingClientRect = () => ({ top: 300, height: 40 } as DOMRect));
  scroll.scrollTo = vi.fn();
  return scroll;
}

describe("台本選択へのスクロール追従", () => {
  beforeEach(() => { resetNavigation(String(Math.random())); vi.unstubAllGlobals(); });
  it("選択行を中央に配置し、閉じた祖先も保存状態ごと開く", () => {
    const scroll = fixture();
    setCollapsed("recording", "part", true);
    setCollapsed("recording", "chapter", true);
    expect(createNavigationFollower()(document, "p", "recording", "b")).toBe(true);
    expect(scroll.scrollTo).toHaveBeenCalledWith({ top: 300, behavior: "smooth" });
    expect(Array.from(document.querySelectorAll("details")).every(node => node.open)).toBe(true);
    expect(isCollapsed("recording", "part")).toBe(false);
    expect(isCollapsed("recording", "chapter")).toBe(false);
  });
  it("同じ選択の再描画では追従せず、次の選択で再び追従する", () => {
    const follow = createNavigationFollower();
    follow(document, "p", "script", "a");
    fixture();
    follow(document, "p", "recording", "a");
    const second = fixture();
    expect(follow(document, "p", "recording", "a")).toBe(false);
    expect(second.scrollTo).not.toHaveBeenCalled();
    expect(follow(document, "p", "recording", "b")).toBe(true);
    expect(second.scrollTo).toHaveBeenCalledOnce();
  });
  it("別プロジェクトの同じIDは新しい選択として扱う", () => {
    const scroll = fixture(), follow = createNavigationFollower();
    follow(document, "one", "recording", "a");
    follow(document, "two", "recording", "a");
    expect(scroll.scrollTo).toHaveBeenCalledTimes(2);
  });
  it("フィルター外の行や録音以外ではスクロールを操作しない", () => {
    const scroll = fixture(), follow = createNavigationFollower();
    expect(follow(document, "p", "recording", "missing")).toBe(false);
    expect(follow(document, "p", "reading", "a")).toBe(false);
    expect(scroll.scrollTo).not.toHaveBeenCalled();
  });
  it("端の選択は範囲内に収め、動きを減らす設定に従う", () => {
    const scroll = fixture();
    vi.stubGlobal("matchMedia", () => ({ matches: true }));
    document.querySelector<HTMLElement>("button")!.getBoundingClientRect = () => ({ top: -100, height: 40 } as DOMRect);
    createNavigationFollower()(document, "p", "recording", "a");
    expect(scroll.scrollTo).toHaveBeenCalledWith({ top: 0, behavior: "auto" });
  });
});
