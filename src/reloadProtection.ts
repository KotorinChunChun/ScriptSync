/** 制御できる再読み込みは抑止。画面消失はバックエンド側でも監視する。 */
export function isReloadKey(event:Pick<KeyboardEvent,'key'|'ctrlKey'|'metaKey'>){
 return event.key==='F5'||((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='r');
}
export function installReloadProtection(onBlocked:()=>void, allowRecordingF5:()=>boolean=()=>false){
 window.addEventListener('keydown',event=>{if(event.key==='F5'&&!event.ctrlKey&&!event.metaKey&&allowRecordingF5()){event.preventDefault();return;}if(isReloadKey(event)){event.preventDefault();event.stopImmediatePropagation();onBlocked();}},true);
}
