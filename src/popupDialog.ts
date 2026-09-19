/** ポップアップの背景クリックを、その画面の取消処理へ統一して渡す。 */
import './popupDialog.css';
export function bindBackdropCancel(overlay:HTMLElement,close:()=>void){
 overlay.addEventListener('click',event=>{if(event.target===overlay){event.preventDefault();close();}});
}

const cancelSelector='[data-cancel],[data-choice="cancel"],[data-join-cancel],[data-trim-cancel],[data-quality-cancel],#delete-cancel,#purge-current-cancel,#take-move-cancel,[data-close]';
document.addEventListener('click',event=>{
 const target=event.target;
 if(!(target instanceof HTMLElement)||!target.matches('.settings-overlay,.delete-modal,.take-trim-modal,.archive-manager'))return;
 const cancel=target.querySelector<HTMLButtonElement>(cancelSelector);
 if(cancel&&!cancel.disabled){event.preventDefault();cancel.click();}
});
