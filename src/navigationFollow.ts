import { setCollapsed, type Stage } from "./navigation";

/** 選択が変わった時だけ追従し、録音状態などの再描画で手動スクロールを奪わない。 */
export function createNavigationFollower() {
  let previous = "";
  return (root: ParentNode, projectKey: string, stage: Stage, selected: string): boolean => {
    const key = JSON.stringify([projectKey, stage, selected]);
    if (key === previous) return false;
    previous = key;
    if (stage !== "recording" || !selected) return false;
    const scroller = root.querySelector<HTMLElement>("[data-navigation-scroll]");
    const row = Array.from(scroller?.querySelectorAll<HTMLElement>("[data-segment]") ?? [])
      .find(element => element.dataset.segment === selected);
    if (!scroller || !row) return false;
    for (let parent = row.parentElement; parent && parent !== scroller; parent = parent.parentElement) {
      if (parent instanceof HTMLDetailsElement) {
        parent.open = true;
        if (parent.dataset.tree) setCollapsed(stage, parent.dataset.tree, false);
      }
    }
    const bounds = scroller.getBoundingClientRect();
    const item = row.getBoundingClientRect();
    const top = Math.max(0, Math.min(scroller.scrollHeight - scroller.clientHeight,
      scroller.scrollTop + item.top - bounds.top - scroller.clientTop - (scroller.clientHeight - item.height) / 2));
    const reducedMotion = globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    if (typeof scroller.scrollTo === "function") scroller.scrollTo({ top, behavior: reducedMotion ? "auto" : "smooth" });
    else scroller.scrollTop = top;
    return true;
  };
}
