/** 操作失敗を画面中央へ一時表示する。フォーカスは奪わない。 */
let timer:ReturnType<typeof setTimeout>|undefined;
export function showTransientError(message:string){
 clearTimeout(timer);document.getElementById('transient-error')?.remove();
 const popup=document.createElement('div');popup.id='transient-error';popup.setAttribute('role','alert');popup.textContent=message;
 Object.assign(popup.style,{position:'fixed',left:'50%',top:'50%',transform:'translate(-50%,-50%)',zIndex:'3000',maxWidth:'min(560px,90vw)',padding:'20px 26px',border:'2px solid #b64a35',borderRadius:'10px',background:'#fff7f4',color:'#812a1b',whiteSpace:'pre-wrap',boxShadow:'0 8px 35px #0004',pointerEvents:'none'});
 document.body.append(popup);timer=setTimeout(()=>popup.remove(),3000);
}
