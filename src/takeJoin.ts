import {escapeHtml as esc} from './html';
import type {Take} from './models';

export type TakeJoinContext={
 projectId:string;segment:{id:string;takes:Take[]};disabled:boolean;
 rpc:<T>(action:string,payload:unknown)=>Promise<T>;
 play:(file:string,hash:string)=>Promise<void>;stop:()=>void;
 prepare:()=>Promise<boolean>;onSaved:()=>Promise<void>;notice:(message:string,error?:boolean)=>void;
};
export function bindTakeJoin(context:TakeJoinContext){
 const heading=document.querySelector('.take-column .section-head');if(!heading)return;
 const ready=context.segment.takes.filter(t=>t.status==='ready');
 const button=document.createElement('button');button.type='button';button.textContent='テイクを結合';button.dataset.takeJoin='';
 button.disabled=context.disabled||ready.length<2;button.title=ready.length<2?'保存済みのテイクが2件以上必要です':'前と後のテイクを選び、新しいテイクを作ります';
 heading.append(button);button.onclick=async()=>{button.disabled=true;try{if(await context.prepare())openTakeJoin(context);}catch(e){context.notice(String(e),true);}finally{button.disabled=context.disabled||ready.length<2;}};
}
export function openTakeJoin(context:TakeJoinContext){
 if(document.querySelector('.take-join-modal'))return;
 const takes=context.segment.takes.filter(t=>t.status==='ready');if(takes.length<2)return;
 const previous=document.activeElement as HTMLElement|null,dialog=document.createElement('dialog');dialog.className='take-join-modal';dialog.setAttribute('role','dialog');dialog.setAttribute('aria-labelledby','take-join-title');
 const options=takes.map(t=>`<option value="${esc(t.id)}">Take ${context.segment.takes.indexOf(t)+1}（${t.duration.toFixed(2)}秒）</option>`).join('');
 dialog.innerHTML=`<h2 id="take-join-title">前後のテイクを結合</h2><p>前の音声 → 後の音声の順に結合します。元のテイクは残り、結合結果を新しい未採用テイクとして追加します。</p><div class="take-join-order"><label>前に置くテイク<select data-join-before>${options}</select></label><button type="button" data-join-swap>前後を入れ替え</button><label>後ろに置くテイク<select data-join-after>${options}</select></label></div><p class="muted">保存済みの音声をそのまま連結します。余分な無音は追加しません。必要な部分の切り出しは先に「編集」で行ってください。</p><p data-join-summary></p><p data-join-message role="status" aria-live="polite"></p><div class="toolbar"><button type="button" data-join-preview>結合結果を試聴</button><button type="button" data-join-stop>試聴を停止</button><button type="button" data-join-cancel>キャンセル</button><button type="button" data-join-save class="primary">結合して新しいテイクを保存</button></div>`;
 const before=dialog.querySelector<HTMLSelectElement>('[data-join-before]')!,after=dialog.querySelector<HTMLSelectElement>('[data-join-after]')!,save=dialog.querySelector<HTMLButtonElement>('[data-join-save]')!,preview=dialog.querySelector<HTMLButtonElement>('[data-join-preview]')!,message=dialog.querySelector<HTMLElement>('[data-join-message]')!;
 after.value=takes[1].id;let pending=false,previewed='';
 const fingerprint=()=>`${before.value}/${after.value}`;
 const update=()=>{const first=takes.find(t=>t.id===before.value)!,last=takes.find(t=>t.id===after.value)!;const same=before.value===after.value;dialog.querySelector('[data-join-summary]')!.textContent=same?'前と後には異なるテイクを選んでください':`結合後の長さ：約 ${((first.trim_end||first.duration)-(first.trim_start||0)+(last.trim_end||last.duration)-(last.trim_start||0)).toFixed(2)} 秒`;
   dialog.querySelectorAll<HTMLButtonElement|HTMLSelectElement>('button,select').forEach(el=>el.disabled=pending);preview.disabled=pending||same;save.disabled=pending||same||previewed!==fingerprint();};
 const payload=()=>({project_id:context.projectId,segment:context.segment.id,before:before.value,after:after.value,before_hash:takes.find(t=>t.id===before.value)!.hash,after_hash:takes.find(t=>t.id===after.value)!.hash});
 const close=()=>{if(pending)return;context.stop();dialog.close();dialog.remove();previous?.focus();};
 const changed=()=>{context.stop();previewed='';message.textContent='順序を変更しました。結合結果を試聴してください。';update();};
 before.onchange=changed;after.onchange=changed;
 dialog.querySelector<HTMLButtonElement>('[data-join-swap]')!.onclick=()=>{const value=before.value;before.value=after.value;after.value=value;changed();};
 dialog.querySelector<HTMLButtonElement>('[data-join-cancel]')!.onclick=close;
 dialog.querySelector<HTMLButtonElement>('[data-join-stop]')!.onclick=()=>context.stop();
 dialog.oncancel=e=>{e.preventDefault();close();};
 dialog.onkeydown=e=>{if(e.key==='Escape'){e.preventDefault();e.stopPropagation();close();}};
 preview.onclick=async()=>{if(pending)return;pending=true;update();message.textContent='結合音声を準備しています…';try{context.stop();const clip=await context.rpc<{file:string;hash:string}>('preview_take_join',payload());await context.play(clip.file,clip.hash);previewed=fingerprint();message.textContent='結合結果を試聴中です。確認後に保存してください。';}catch(e){previewed='';message.textContent=String(e);}finally{pending=false;update();}};
 save.onclick=async()=>{if(pending||previewed!==fingerprint())return;pending=true;update();message.textContent='結合したテイクを保存しています…';try{context.stop();await context.rpc('join_takes',payload());pending=false;close();await context.onSaved();context.notice('前後のテイクを結合しました。新しいテイクを確認して採用してください。');}catch(e){message.textContent=String(e);}finally{pending=false;update();}};
 document.body.append(dialog);dialog.showModal();update();before.focus();
}
