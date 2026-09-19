import {escapeHtml} from './html';
import {bindBackdropCancel} from './popupDialog';
export type CloseChoice='save'|'discard'|'cancel';
export function askUnsavedClose(message='未保存の変更があります。保存して閉じますか？'):Promise<CloseChoice>{
 return new Promise(resolve=>{
  const dialog=document.createElement('dialog');dialog.className='action-confirm';
  dialog.innerHTML=`<p>${escapeHtml(message)}</p><div class="toolbar popup-actions"><button data-choice="cancel">キャンセル</button><button data-choice="discard">保存せず閉じる</button><button class="primary" data-choice="save">保存して閉じる</button></div>`;
  const finish=(choice:CloseChoice)=>{dialog.remove();resolve(choice);};
  dialog.querySelectorAll<HTMLButtonElement>('[data-choice]').forEach(button=>button.onclick=()=>finish(button.dataset.choice as CloseChoice));
  dialog.oncancel=event=>{event.preventDefault();finish('cancel');};
  bindBackdropCancel(dialog,()=>finish('cancel'));
  document.body.append(dialog);dialog.showModal();dialog.querySelector<HTMLButtonElement>('[data-choice=cancel]')!.focus();
 });
}
export function createCloseGuard(context:{settle:()=>Promise<void>;dirty:()=>boolean;save:()=>Promise<void>;close:()=>Promise<void>;ask?:(message?:string)=>Promise<CloseChoice>;error:(message:string)=>void}){
 let running=false;
 return async()=>{if(running)return;running=true;try{
  let failure='';try{await context.settle();}catch(error){failure=String(error);}
  if(failure||context.dirty()){
   const choice=await(context.ask??askUnsavedClose)(failure?`保存できていない変更があります。${failure}`:undefined);
   if(choice==='cancel')return;
   if(choice==='save'){await context.save();if(context.dirty())throw new Error('未保存の変更が残っています。内容を確認してください。');}
  }
  await context.close();
 }catch(error){context.error(String(error));}finally{running=false;}};
}
