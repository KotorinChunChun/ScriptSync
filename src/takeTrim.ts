import {confirmAction} from "./confirmAction";

export type TrimTake={id:string;duration:number;file:string;hash:string;trim_start?:number;trim_end?:number};
export type TakeTrimContext={
  segment:{id:string;takes:TrimTake[]};
  rpc:<T>(action:string,payload?:unknown)=>Promise<T>;
  /** `loop` は境界試聴でだけ使う。未対応の再生器でも全体試聴は保存条件を満たす。 */
  play:(file:string,hash:string,takeId?:string,start?:number,duration?:number,loop?:boolean)=>Promise<void>;
  isPlaying?:(takeId:string)=>boolean;
  stop?:()=>void;
  notice:(message:string,error?:boolean)=>void;
  /** WAV を実加工した後に、親画面でテイクと波形を読み直す。 */
  onSaved?:(take:TrimTake)=>Promise<void>|void;
};
type Range={start:number;end:number;cutStart?:number;cutEnd?:number};
type Entry={context:TakeTrimContext;take:TrimTake;root:HTMLElement};
const drafts=new Map<string,Range>();
const saved=new Map<string,Range>();
const previewed=new Map<string,string>();
const contexts=new Map<string,Entry>();
const pending=new Set<string>();
const eps=.0001;
let activeModal:HTMLElement|undefined;

const rangeFor=(take:TrimTake):Range=>({start:take.trim_start??0,end:take.trim_end||take.duration});
const copy=(range:Range):Range=>({...range});
const number=(value:number|undefined)=>value===undefined?"":value.toFixed(2);
const fingerprint=(range:Range)=>[range.start,range.end,range.cutStart??"",range.cutEnd??""].map(value=>typeof value==="number"?value.toFixed(4):value).join(":");
const same=(a:Range,b:Range)=>fingerprint(a)===fingerprint(b);
export const isTakeTrimDirty=(takeId:string)=>{const draft=drafts.get(takeId),baseline=saved.get(takeId);return !!draft&&!!baseline&&!same(draft,baseline);};
/** 旧呼出側の互換用。途中カットはこの戻り値には含めない。 */
export const getTakeTrimRange=(takeId:string)=>{const range=drafts.get(takeId);return range?{start:range.start,end:range.end}:undefined;};
/** 削除済みテイクやプロジェクト切替時に編集状態を破棄する。 */
export function discardTakeTrim(takeId:string){drafts.delete(takeId);saved.delete(takeId);previewed.delete(takeId);contexts.delete(takeId);if(activeModal?.dataset.takeId===takeId)activeModal.remove();}
function valid(range:Range,duration:number){
  if(!Number.isFinite(range.start)||!Number.isFinite(range.end)||range.start<0||range.end<=range.start||range.end>duration)return false;
  if((range.cutStart===undefined)!==(range.cutEnd===undefined))return false;
  if(range.cutStart===undefined||range.cutEnd===undefined)return true;
  return Number.isFinite(range.cutStart)&&Number.isFinite(range.cutEnd)&&range.cutStart>range.start&&range.cutEnd<range.end&&range.cutEnd>range.cutStart;
}
function setDraft(takeId:string,next:Range){drafts.set(takeId,next);previewed.delete(takeId);}
function inject(take:TrimTake){
  const wave=document.getElementById(`take-wave-${take.id}`)?.parentElement;
  const actions=wave?.parentElement?.querySelector<HTMLElement>(".take-actions");
  if(!wave||!actions||actions.querySelector(`[data-trim-edit="${take.id}"]`))return;
  actions.insertAdjacentHTML("afterbegin",`<button type="button" data-trim-edit="${take.id}">編集</button>`);
}
function input(modal:HTMLElement,name:string){return modal.querySelector<HTMLInputElement>(`[data-trim-${name}]`)!;}
function updateModal(entry:Entry,preserve=false){
  const modal=activeModal;if(!modal||modal.dataset.takeId!==entry.take.id)return;
  const range=drafts.get(entry.take.id)!,duration=Math.max(entry.take.duration,eps),cutEnabled=range.cutStart!==undefined;
  for(const [name,value] of [["start",range.start],["end",range.end],["cut-start",range.cutStart],["cut-end",range.cutEnd]] as const){
    const field=input(modal,name);if(!preserve||document.activeElement!==field)field.value=number(value);
    field.disabled=(name.startsWith("cut-")&&!cutEnabled)||pending.has(entry.take.id);
  }
  input(modal,"cut-enabled").checked=cutEnabled;
  const keep=modal.querySelector<HTMLElement>(".take-trim-keep")!;
  keep.style.left=`${range.start/duration*100}%`;keep.style.width=`${(range.end-range.start)/duration*100}%`;
  modal.querySelector<HTMLElement>('[data-trim-edge="start"]')!.style.left=`calc(${range.start/duration*100}% - 6px)`;
  modal.querySelector<HTMLElement>('[data-trim-edge="end"]')!.style.left=`calc(${range.end/duration*100}% - 6px)`;
  const cut=modal.querySelector<HTMLElement>(".take-trim-cut")!;
  cut.hidden=!cutEnabled;
  const cutStart=modal.querySelector<HTMLElement>('[data-trim-edge="cut-start"]')!,cutEnd=modal.querySelector<HTMLElement>('[data-trim-edge="cut-end"]')!;
  cutStart.hidden=!cutEnabled;cutEnd.hidden=!cutEnabled;
  if(cutEnabled){cut.style.left=`${range.cutStart!/duration*100}%`;cut.style.width=`${(range.cutEnd!-range.cutStart!)/duration*100}%`;cutStart.style.left=`calc(${range.cutStart!/duration*100}% - 6px)`;cutEnd.style.left=`calc(${range.cutEnd!/duration*100}% - 6px)`;}
  modal.querySelector<HTMLElement>("[data-trim-summary]")!.textContent=cutEnabled
    ?`残す範囲 ${range.start.toFixed(2)}–${range.cutStart!.toFixed(2)} 秒 と ${range.cutEnd!.toFixed(2)}–${range.end.toFixed(2)} 秒`
    :`残す範囲 ${range.start.toFixed(2)}–${range.end.toFixed(2)} 秒`;
  const checked=previewed.get(entry.take.id)===fingerprint(range);
  const save=modal.querySelector<HTMLButtonElement>("[data-trim-save]")!;
  save.disabled=!isTakeTrimDirty(entry.take.id)||!checked||pending.has(entry.take.id);
  modal.querySelector<HTMLElement>("[data-trim-check]")!.textContent=checked?"現在の編集内容を試聴済みです":"編集すると、保存前にもう一度試聴が必要です";
}
function closeModal(){
  const modal=activeModal;if(!modal)return;
  const restore=modal._restore as HTMLElement|undefined;
  for(const node of modal._siblings??[])node.inert=false;
  modal.remove();activeModal=undefined;restore?.focus();
}
declare global { interface HTMLElement { _restore?:HTMLElement;_siblings?:HTMLElement[]; } }
function openModal(entry:Entry){
  closeModal();
  const modal=document.createElement("div");
  modal.className="take-trim-modal take-trim-editor";modal.dataset.takeId=entry.take.id;
  modal.setAttribute("role","dialog");modal.setAttribute("aria-modal","true");modal.setAttribute("aria-labelledby","take-trim-title");
  modal.innerHTML=`<section><header><h2 id="take-trim-title">テイクを編集</h2><button type="button" data-trim-close aria-label="編集を閉じる">閉じる</button></header><p class="muted">前後の残す範囲と、途中の削除範囲を一組指定できます。保存前に編集後の音声を試聴してください。</p><div class="take-trim-timeline" aria-label="音声編集範囲"><i class="take-trim-keep"></i><i class="take-trim-cut" hidden></i><button type="button" class="take-trim-marker start" data-trim-edge="start" aria-label="残す範囲の開始"></button><button type="button" class="take-trim-marker end" data-trim-edge="end" aria-label="残す範囲の終了"></button><button type="button" class="take-trim-marker cut-start" data-trim-edge="cut-start" aria-label="途中カットの開始"></button><button type="button" class="take-trim-marker cut-end" data-trim-edge="cut-end" aria-label="途中カットの終了"></button></div><div class="take-trim-fields"><label>開始 <input data-trim-start type="number" min="0" step="0.01" inputmode="decimal"></label><label>終了 <input data-trim-end type="number" min="0" step="0.01" inputmode="decimal"></label><label class="take-trim-cut-toggle"><input data-trim-cut-enabled type="checkbox"> 途中を削除</label><label>削除開始 <input data-trim-cut-start type="number" min="0" step="0.01" inputmode="decimal"></label><label>削除終了 <input data-trim-cut-end type="number" min="0" step="0.01" inputmode="decimal"></label></div><output data-trim-summary></output><p data-trim-check class="take-trim-check"></p><div class="toolbar take-trim-preview-actions"><button type="button" data-trim-preview="full">編集後を全体試聴</button><button type="button" data-trim-preview="boundaries">境界を反復試聴</button></div><div class="toolbar"><button type="button" data-trim-cancel>キャンセル</button><button type="button" data-trim-save class="primary">音声を加工して保存</button></div></section>`;
  const siblings=[...document.body.children] as HTMLElement[];modal._siblings=siblings;modal._restore=document.activeElement as HTMLElement;siblings.forEach(node=>node.inert=true);document.body.append(modal);activeModal=modal;
  const edit=(name:"start"|"end"|"cut-start"|"cut-end",value:number)=>{
    if(pending.has(entry.take.id)||!Number.isFinite(value))return;
    const next=copy(drafts.get(entry.take.id)!);
    if(name==="start")next.start=Math.max(0,Math.min(value,next.end-eps));
    else if(name==="end")next.end=Math.min(entry.take.duration,Math.max(value,next.start+eps));
    else if(name==="cut-start"&&next.cutEnd!==undefined)next.cutStart=Math.max(next.start+eps,Math.min(value,next.cutEnd-eps));
    else if(name==="cut-end"&&next.cutStart!==undefined)next.cutEnd=Math.min(next.end-eps,Math.max(value,next.cutStart+eps));
    setDraft(entry.take.id,next);updateModal(entry,true);
  };
  (["start","end","cut-start","cut-end"] as const).forEach(name=>{const field=input(modal,name);field.oninput=()=>edit(name,field.valueAsNumber);field.onchange=()=>{edit(name,field.valueAsNumber);updateModal(entry);};});
  input(modal,"cut-enabled").onchange=event=>{const enabled=(event.currentTarget as HTMLInputElement).checked,next=copy(drafts.get(entry.take.id)!);if(enabled){const third=(next.end-next.start)/3;next.cutStart=next.start+third;next.cutEnd=next.end-third;}else{delete next.cutStart;delete next.cutEnd;}setDraft(entry.take.id,next);updateModal(entry);};
  let dragging:"start"|"end"|"cut-start"|"cut-end"|undefined;
  modal.querySelectorAll<HTMLButtonElement>("[data-trim-edge]").forEach(marker=>marker.onpointerdown=event=>{if(marker.classList.contains("cut-")&&!input(modal,"cut-enabled").checked)return;dragging=marker.dataset.trimEdge as typeof dragging;marker.setPointerCapture?.(event.pointerId);event.preventDefault();});
  const timeline=modal.querySelector<HTMLElement>(".take-trim-timeline")!;
  const move=(event:PointerEvent)=>{if(!dragging)return;const rect=timeline.getBoundingClientRect();edit(dragging,Math.max(0,Math.min(entry.take.duration,(event.clientX-rect.left)/Math.max(rect.width,1)*entry.take.duration)));};
  timeline.onpointermove=move;timeline.onpointerup=()=>dragging=undefined;timeline.onpointercancel=()=>dragging=undefined;
  modal.querySelector<HTMLButtonElement>("[data-trim-close]")!.onclick=closeModal;modal.querySelector<HTMLButtonElement>("[data-trim-cancel]")!.onclick=closeModal;
  modal.querySelectorAll<HTMLButtonElement>("[data-trim-preview]").forEach(button=>button.onclick=()=>void preview(entry,button.dataset.trimPreview as "full"|"boundaries"));
  modal.querySelector<HTMLButtonElement>("[data-trim-save]")!.onclick=()=>void saveTake(entry.take.id);
  modal.onkeydown=event=>{
    if(event.key==="Escape"){event.preventDefault();event.stopPropagation();closeModal();return;}
    if(event.key==="Tab"){const focusable=[...modal.querySelectorAll<HTMLElement>('button:not([disabled]),input:not([disabled])')];const at=focusable.indexOf(document.activeElement as HTMLElement),next=(at+(event.shiftKey?focusable.length-1:1))%focusable.length;event.preventDefault();focusable[next]?.focus();}
  };
  updateModal(entry);input(modal,"start").focus();
}
async function preview(entry:Entry,mode:"full"|"boundaries"){
  const range=copy(drafts.get(entry.take.id)!);
  if(!valid(range,entry.take.duration)){entry.context.notice("開始・終了と途中カットを正しい範囲にしてください",true);return;}
  try{
    if(entry.context.isPlaying?.(entry.take.id)){entry.context.stop?.();return;}
    const clip=await entry.context.rpc<{file:string;hash:string}>("preview_take_edit",{segment:entry.context.segment.id,take:entry.take.id,start:range.start,end:range.end,cut_start:range.cutStart,cut_end:range.cutEnd,mode});
    await entry.context.play(clip.file,clip.hash,entry.take.id,0,undefined,mode==="boundaries");
    previewed.set(entry.take.id,fingerprint(range));updateModal(entry);
  }catch(error){entry.context.notice(String(error),true);}
}
export function bindTakeTrim(context:TakeTrimContext){
  for(const take of context.segment.takes){
    if(!take.duration)continue;
    if(!saved.has(take.id))saved.set(take.id,rangeFor(take));
    if(!drafts.has(take.id))drafts.set(take.id,rangeFor(take));
    inject(take);
    const wave=document.getElementById(`take-wave-${take.id}`)?.parentElement;if(!wave)continue;
    const entry={context,take,root:wave};contexts.set(take.id,entry);
    const button=wave.parentElement?.querySelector<HTMLButtonElement>(`[data-trim-edit="${take.id}"]`);
    if(button)button.onclick=()=>openModal(entry);
  }
}
async function saveTake(takeId:string):Promise<boolean>{
  const entry=contexts.get(takeId);if(!entry||pending.has(takeId))return false;
  const range=copy(drafts.get(takeId)!);
  if(!valid(range,entry.take.duration)){entry.context.notice("開始・終了と途中カットを正しい範囲にしてください",true);return false;}
  if(previewed.get(takeId)!==fingerprint(range)){entry.context.notice("現在の編集内容を試聴してから保存してください",true);return false;}
  if(!await confirmAction("音声を加工して保存しますか？ 保存すると元の音声へ復元できません。"))return false;
  pending.add(takeId);updateModal(entry);
  try{
    const result=await entry.context.rpc<TrimTake|{take?:TrimTake}>("trim_take",{segment:entry.context.segment.id,take:entry.take.id,start:range.start,end:range.end,cut_start:range.cutStart,cut_end:range.cutEnd});
    const persisted=result&&typeof result==="object"&&"take" in result?(result as {take?:TrimTake}).take:result as TrimTake|undefined;
    if(!persisted?.duration)throw new Error("保存後のテイク情報を取得できませんでした");
    Object.assign(entry.take,persisted);const reset={start:0,end:entry.take.duration};drafts.set(takeId,copy(reset));saved.set(takeId,copy(reset));previewed.delete(takeId);
    await entry.context.onSaved?.(entry.take);entry.context.notice("テイクを加工して保存し、再文字起こしを予約しました");closeModal();return true;
  }catch(error){entry.context.notice(String(error),true);return false;}finally{pending.delete(takeId);updateModal(entry);}
}
export function confirmTakeTrimBeforeMove():Promise<"save"|"discard"|"cancel"> { return new Promise(resolve=>{const modal=document.createElement("div"),siblings=[...document.body.children] as HTMLElement[];modal.className="take-trim-modal";modal.setAttribute("role","dialog");modal.setAttribute("aria-modal","true");modal.innerHTML=`<section><h2>未保存の音声編集があります</h2><p>保存してから移動しますか？</p><div><button data-choice="save">保存して移動</button><button data-choice="discard">破棄して移動</button><button data-choice="cancel">戻る</button></div></section>`;const close=(choice:"save"|"discard"|"cancel")=>{siblings.forEach(node=>node.inert=false);modal.remove();resolve(choice);};modal.querySelectorAll<HTMLButtonElement>("[data-choice]").forEach(button=>button.onclick=()=>close(button.dataset.choice as "save"|"discard"|"cancel"));modal.onkeydown=event=>{if(event.key==="Escape"){event.preventDefault();event.stopPropagation();close("cancel");}if(event.key==="Tab"){event.preventDefault();const buttons=[...modal.querySelectorAll<HTMLButtonElement>("button")],index=buttons.indexOf(document.activeElement as HTMLButtonElement);buttons[(index+(event.shiftKey?buttons.length-1:1))%buttons.length].focus();}};siblings.forEach(node=>node.inert=true);document.body.append(modal);modal.querySelector<HTMLButtonElement>("[data-choice=save]")!.focus();}); }
async function performTrimMove(move:()=>Promise<void>|void):Promise<boolean>{const dirty=[...drafts.keys()].filter(isTakeTrimDirty);if(!dirty.length){await move();return true;}const choice=await confirmTakeTrimBeforeMove();if(choice==="cancel")return false;if(choice==="discard"){for(const id of dirty){drafts.set(id,copy(saved.get(id)!));previewed.delete(id);}await move();return true;}for(const id of dirty)if(!await saveTake(id))return false;await move();return true;}
let guardedMove:Promise<boolean>|null=null;
export function guardTakeTrimMove(move:()=>Promise<void>|void):Promise<boolean>{if(guardedMove)return guardedMove;guardedMove=performTrimMove(move).finally(()=>{guardedMove=null;});return guardedMove;}
