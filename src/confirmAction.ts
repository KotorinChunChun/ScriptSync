import {escapeHtml} from './html';
export function confirmAction(message:string):Promise<boolean>{
 return new Promise(resolve=>{
  const dialog=document.createElement('dialog');dialog.className='action-confirm';
  dialog.innerHTML=`<p>${escapeHtml(message)}</p><div class="toolbar"><button data-cancel>キャンセル</button><button data-confirm class="primary">実行</button></div>`;
  const finish=(value:boolean)=>{dialog.remove();resolve(value);};
  dialog.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=()=>finish(false);
  dialog.querySelector<HTMLButtonElement>('[data-confirm]')!.onclick=()=>finish(true);
  dialog.oncancel=e=>{e.preventDefault();finish(false);};
  document.body.append(dialog);dialog.showModal();dialog.querySelector<HTMLButtonElement>('[data-cancel]')!.focus();
 });
}
