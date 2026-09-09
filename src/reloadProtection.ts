/** 制御できる再読み込みは抑止。画面消失はバックエンド側でも監視する。 */
export function isReloadKey(event:Pick<KeyboardEvent,'key'|'ctrlKey'|'metaKey'>){
 return event.key==='F5'||((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='r');
}
export function installReloadProtection(onBlocked:()=>void){
 window.addEventListener('keydown',event=>{if(isReloadKey(event)){event.preventDefault();event.stopImmediatePropagation();onBlocked();}},true);
}
