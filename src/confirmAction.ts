import {escapeHtml} from './html';
export function confirmAction(message:string):Promise<boolean>{
 return new Promise(resolve=>{
  const dialog=document.createElement('dialog');dialog.className='action-confirm';dialog.setAttribute('role','dialog');
  dialog.innerHTML=`<p>${escapeHtml(message)}</p><div class="toolbar"><button data-cancel>キャンセル</button><button data-confirm class="primary">実行</button></div>`;
  let finished=false;const finish=(value:boolean)=>{if(finished)return;finished=true;dialog.remove();resolve(value);};
  dialog.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=()=>finish(false);
  dialog.querySelector<HTMLButtonElement>('[data-confirm]')!.onclick=()=>finish(true);
  dialog.oncancel=e=>{e.preventDefault();finish(false);};
  dialog.onkeydown=e=>{if(e.key==='Escape'){e.preventDefault();e.stopPropagation();finish(false);}else if(e.key==='Enter'&&document.activeElement!==dialog.querySelector('[data-cancel]')){e.preventDefault();e.stopPropagation();finish(true);}};
  document.body.append(dialog);dialog.showModal();dialog.querySelector<HTMLButtonElement>('[data-confirm]')!.focus();
 });
}
