import { beforeEach, describe, expect, it, vi } from "vitest";
import { bindTakeTrim, discardTakeTrim, isTakeTrimDirty, guardTakeTrimMove } from "../../src/takeTrim";

const take={id:"take-1",duration:2,file:"a.wav",hash:"a",trim_start:0,trim_end:2};
const pointer=(type:string,clientX:number)=>Object.assign(new Event(type,{bubbles:true}),{clientX,pointerId:1}) as PointerEvent;
beforeEach(()=>{take.trim_start=0;take.trim_end=2;discardTakeTrim("take-1");document.body.innerHTML=`<article><div class="take-wave-wrap"><svg id="take-wave-take-1"></svg></div><div class="take-actions"><button data-action="select">採用</button><button data-action="delete_take">削除</button></div></article>`;});
describe("テイク別トリム",()=>{
 it("ドラッグ中は採用を隠し、保存成功後だけ編集状態を解消する",async()=>{const rpc=vi.fn(async()=>undefined),notice=vi.fn();bindTakeTrim({segment:{id:"s",takes:[take]},rpc,play:vi.fn(),notice});const handle=document.querySelector<HTMLButtonElement>(".take-trim-handle.start")!;const wave=document.querySelector<HTMLElement>(".take-wave-wrap")!;vi.spyOn(wave,"getBoundingClientRect").mockReturnValue({left:0,width:100} as DOMRect);handle.onpointerdown!(pointer("pointerdown",0));wave.onpointermove!(pointer("pointermove",25));wave.onpointerup!(pointer("pointerup",25));expect(isTakeTrimDirty("take-1")).toBe(true);expect(document.querySelector<HTMLButtonElement>("[data-action=select]")!.hidden).toBe(true);document.querySelector<HTMLButtonElement>("[data-trim-save]")!.click();await Promise.resolve();expect(rpc).toHaveBeenCalledWith("trim_take",{segment:"s",take:"take-1",start:.5,end:2});expect(isTakeTrimDirty("take-1")).toBe(false);});
 it("保存失敗時は編集中の範囲を保持する",async()=>{const rpc=vi.fn(async()=>{throw new Error("保存失敗")}),notice=vi.fn();bindTakeTrim({segment:{id:"s",takes:[take]},rpc,play:vi.fn(),notice});const handle=document.querySelector<HTMLButtonElement>(".take-trim-handle.start")!,wave=document.querySelector<HTMLElement>(".take-wave-wrap")!;vi.spyOn(wave,"getBoundingClientRect").mockReturnValue({left:0,width:100} as DOMRect);handle.onpointerdown!(pointer("pointerdown",0));wave.onpointermove!(pointer("pointermove",25));wave.onpointerup!(pointer("pointerup",25));document.querySelector<HTMLButtonElement>("[data-trim-save]")!.click();await Promise.resolve();expect(isTakeTrimDirty("take-1")).toBe(true);expect(notice).toHaveBeenCalledWith("Error: 保存失敗",true);});
});

it('保存して移動は保存成功を待ち、失敗なら移動しない',async()=>{
 let finish!:(value:unknown)=>void;const rpc=vi.fn(()=>new Promise(resolve=>finish=resolve)),move=vi.fn();
 bindTakeTrim({segment:{id:'s',takes:[take]},rpc:rpc as any,play:vi.fn(),notice:vi.fn()});
 const wave=document.querySelector<HTMLElement>('.take-wave-wrap')!,handle=document.querySelector<HTMLButtonElement>('.take-trim-handle.start')!;
 vi.spyOn(wave,'getBoundingClientRect').mockReturnValue({left:0,width:100} as DOMRect);
 handle.onpointerdown!(pointer('pointerdown',0));wave.onpointermove!(pointer('pointermove',25));wave.onpointerup!(pointer('pointerup',25));
 const request=guardTakeTrimMove(move);document.querySelector<HTMLButtonElement>('[data-choice=save]')!.click();await Promise.resolve();expect(move).not.toHaveBeenCalled();
 finish(null);await request;expect(move).toHaveBeenCalledOnce();expect(isTakeTrimDirty(take.id)).toBe(false);
});
it('移動の取消は編集中の範囲を保持する',async()=>{
 bindTakeTrim({segment:{id:'s',takes:[take]},rpc:vi.fn(),play:vi.fn(),notice:vi.fn()});const wave=document.querySelector<HTMLElement>('.take-wave-wrap')!,handle=document.querySelector<HTMLButtonElement>('.take-trim-handle.end')!;
 vi.spyOn(wave,'getBoundingClientRect').mockReturnValue({left:0,width:100} as DOMRect);handle.onpointerdown!(pointer('pointerdown',100));wave.onpointermove!(pointer('pointermove',70));wave.onpointerup!(pointer('pointerup',70));
 const move=vi.fn(),request=guardTakeTrimMove(move);document.querySelector<HTMLButtonElement>('[data-choice=cancel]')!.click();expect(await request).toBe(false);expect(move).not.toHaveBeenCalled();expect(isTakeTrimDirty(take.id)).toBe(true);
});
