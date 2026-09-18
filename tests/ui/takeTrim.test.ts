vi.mock("../../src/confirmAction",()=>({confirmAction:vi.fn(async()=>true)}));
import {beforeEach,describe,expect,it,vi} from "vitest";
import {bindTakeTrim,discardTakeTrim,isTakeTrimDirty,guardTakeTrimMove} from "../../src/takeTrim";

const take={id:"take-1",duration:2,file:"a.wav",hash:"a",trim_start:0,trim_end:2};
const pointer=(x:number)=>Object.assign(new Event("pointermove",{bubbles:true}),{clientX:x,pointerId:1}) as PointerEvent;
const field=(name:string)=>document.querySelector<HTMLInputElement>(`[data-trim-${name}]`)!;
function open(context:any){bindTakeTrim(context);document.querySelector<HTMLButtonElement>("[data-trim-edit]")!.click();}
beforeEach(()=>{take.duration=2;take.file="a.wav";take.hash="a";take.trim_start=0;take.trim_end=2;discardTakeTrim(take.id);document.body.innerHTML=`<article><div class="take-wave-wrap"><svg id="take-wave-take-1"></svg></div><div class="take-actions"><button data-action="play">試聴</button><button data-action="select">採用</button><button data-action="delete_take">削除</button></div></article>`;});

describe("テイク編集モーダル",()=>{
 it("テイク欄には常時編集入口だけを置き、前後入力欄を置かない",()=>{
  bindTakeTrim({segment:{id:"s",takes:[take]},rpc:vi.fn(),play:vi.fn(),notice:vi.fn()});
  expect(document.querySelector<HTMLButtonElement>("[data-trim-edit]")?.textContent).toBe("編集");
  expect(document.querySelector(".take-actions [data-trim-save]")).toBeNull();
  expect(document.querySelector(".take-wave-wrap [data-trim-start]")).toBeNull();
 });
 it("途中カットを含む現在の編集を全体試聴した後だけ保存し、保存後に範囲を初期化する",async()=>{
  const rpc=vi.fn(async(action:string)=>action==="preview_take_edit"?{file:"preview.wav",hash:"preview"}:{id:"take-1",duration:1,file:"edited.wav",hash:"new",trim_start:0,trim_end:1});
  const play=vi.fn(),onSaved=vi.fn(),notice=vi.fn();open({segment:{id:"s",takes:[take]},rpc,play,notice,onSaved});
  field("start").value="0.20";field("start").dispatchEvent(new Event("change"));
  const cut=field("cut-enabled");cut.click();
  field("cut-start").value="0.70";field("cut-start").dispatchEvent(new Event("change"));
  field("cut-end").value="1.10";field("cut-end").dispatchEvent(new Event("change"));
  expect(isTakeTrimDirty(take.id)).toBe(true);
  const save=document.querySelector<HTMLButtonElement>("[data-trim-save]")!;expect(save.disabled).toBe(true);
  document.querySelector<HTMLButtonElement>('[data-trim-preview="after"]')!.click();await vi.waitFor(()=>expect(play).toHaveBeenCalledOnce());
  expect(save.disabled).toBe(false);save.click();await vi.waitFor(()=>expect(onSaved).toHaveBeenCalledWith(take));
  expect(rpc).toHaveBeenLastCalledWith("trim_take",{segment:"s",take:"take-1",start:.2,end:2,cut_start:.7,cut_end:1.1});
  expect(take.duration).toBe(1);expect(isTakeTrimDirty(take.id)).toBe(false);expect(document.querySelector(".take-trim-editor")).toBeNull();
 });
 it("数値とマーカーの変更は試聴済み指紋を解除する",async()=>{
  const rpc=vi.fn(async()=>({file:"preview.wav",hash:"preview"})),play=vi.fn();open({segment:{id:"s",takes:[take]},rpc,play,notice:vi.fn()});
  field("start").value="0.25";field("start").dispatchEvent(new Event("change"));document.querySelector<HTMLButtonElement>('[data-trim-preview="after"]')!.click();await vi.waitFor(()=>expect(play).toHaveBeenCalledOnce());
  const save=document.querySelector<HTMLButtonElement>("[data-trim-save]")!;expect(save.disabled).toBe(false);
  const line=document.querySelector<HTMLElement>(".take-trim-timeline")!;vi.spyOn(line,"getBoundingClientRect").mockReturnValue({left:0,width:100} as DOMRect);
  const marker=document.querySelector<HTMLButtonElement>('[data-trim-edge="start"]')!;marker.onpointerdown!(pointer(25));line.onpointermove!(pointer(35));line.onpointerup!(pointer(35));
  expect(save.disabled).toBe(true);
 });
 it("Escapeで閉じ、Tabはモーダル内のフォーカスを巡回する",()=>{
  const opener=document.querySelector<HTMLButtonElement>("[data-action=play]")!;opener.focus();open({segment:{id:"s",takes:[take]},rpc:vi.fn(),play:vi.fn(),notice:vi.fn()});
  const modal=document.querySelector<HTMLElement>(".take-trim-editor")!,first=document.activeElement;modal.dispatchEvent(new KeyboardEvent("keydown",{key:"Tab",bubbles:true,cancelable:true}));expect(document.activeElement).not.toBe(first);
  modal.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true,cancelable:true}));expect(document.querySelector(".take-trim-editor")).toBeNull();expect(document.activeElement).toBe(opener);
 });
 it("未試聴の編集は移動前保存を拒否し、破棄なら移動する",async()=>{
  open({segment:{id:"s",takes:[take]},rpc:vi.fn(),play:vi.fn(),notice:vi.fn()});field("start").value="0.25";field("start").dispatchEvent(new Event("change"));
  document.querySelector<HTMLButtonElement>("[data-trim-cancel]")!.click();const move=vi.fn(),request=guardTakeTrimMove(move);document.querySelector<HTMLButtonElement>("[data-choice=save]")!.click();expect(await request).toBe(false);expect(move).not.toHaveBeenCalled();
  const discard=guardTakeTrimMove(move);document.querySelector<HTMLButtonElement>("[data-choice=discard]")!.click();expect(await discard).toBe(true);expect(move).toHaveBeenCalledOnce();
 });
});
