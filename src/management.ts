/** 管理停止は、確認ダイアログを出さず編集がない場合だけ受理する。 */
export interface ManagementBridge {
 listen:(event:string,handler:(event:{payload:unknown})=>void)=>Promise<()=>void>;
 invoke:<T>(command:string,payload?:Record<string,unknown>)=>Promise<T>;
 blocked:()=>boolean;
 freeze:(value:boolean)=>void;
}
export function hasOpenDialog(){return !!document.querySelector('.settings-overlay,.delete-modal,.take-trim-modal,.archive-manager,[role="dialog"],dialog[open]');}
export async function installManagement(bridge:ManagementBridge){
 let pending=false;
 await bridge.listen('management-stop-requested',event=>{
  if(typeof event.payload!=='string'||pending)return;
  const allowed=!bridge.blocked()&&!hasOpenDialog();
  if(allowed){pending=true;bridge.freeze(true);}
  void bridge.invoke<boolean>('management_stop_reply',{id:event.payload,allowed}).then(accepted=>{
   if(!accepted&&pending){pending=false;bridge.freeze(false);}
  }).catch(()=>{if(pending){pending=false;bridge.freeze(false);}});
 });
 await bridge.listen('management-stop-cancelled',()=>{if(pending){pending=false;bridge.freeze(false);}});
}
