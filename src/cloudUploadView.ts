export type CloudContext={rpc:<T>(action:string,payload?:unknown)=>Promise<T>;notice:(message:string,error?:boolean)=>void;confirm?:(message:string)=>Promise<boolean>|boolean;projectId?:string;generation?:number;scriptRevision?:number;openSettings?:()=>void|Promise<void>};
/** 背面の設定画面へTab/Escapeを渡さず、無効化後も現在の操作候補で循環する。 */
function bindDialogKeys(overlay:HTMLElement,close:()=>void){
 overlay.onkeydown=event=>{
  event.stopPropagation();
  if(event.key==='Escape'){event.preventDefault();close();return;}
  if(event.key!=='Tab')return;
  const controls=[...overlay.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),a[href],summary,[tabindex]:not([tabindex="-1"])')].filter(el=>{
   for(let node:HTMLElement|null=el;node&&node!==overlay;node=node.parentElement){if(node.hidden||node.hasAttribute('inert')||getComputedStyle(node).display==='none'||getComputedStyle(node).visibility==='hidden')return false;}
   return true;
  });
  const index=controls.indexOf(document.activeElement as HTMLElement);
  if(!controls.length){event.preventDefault();return;}
  if(event.shiftKey&&index<=0){event.preventDefault();controls.at(-1)!.focus();}
  else if(!event.shiftKey&&(index<0||index===controls.length-1)){event.preventDefault();controls[0].focus();}
 };
}
/** 選択原稿の確定世代を、既存WebのHN・キー方式で送る。 */
export async function openCloudUpload(context:CloudContext,sourceId:string,provider:'web'='web'){
 if(provider!=='web')throw new Error('ZIP出力またはWebアップロードを使用してください');
 const previous=document.activeElement as HTMLElement|null;
 const siblings=[...document.body.children] as HTMLElement[],inert=siblings.map(el=>el.inert);
 const overlay=document.createElement('div');overlay.className='settings-overlay';overlay.setAttribute('role','dialog');overlay.setAttribute('aria-modal','true');overlay.setAttribute('aria-label','ScriptSyncWebに台本を送信');
 siblings.forEach(el=>el.inert=true);document.body.append(overlay);
 let working=false;
 const close=()=>{if(working)return;overlay.remove();siblings.forEach((el,i)=>el.inert=inert[i]);if(previous?.isConnected)previous.focus();};
 const status=(text:string)=>{overlay.querySelector('[data-status]')!.textContent=text;};
 const update=()=>overlay.querySelectorAll<HTMLButtonElement|HTMLInputElement>('button,input').forEach(el=>el.disabled=working);
 overlay.innerHTML=`<section class="settings-dialog cloud-upload-dialog"><header><h2>ScriptSyncWebに台本を送信</h2></header><div class="settings-scroll"><p>確定した選択原稿をZIPで送信します。既存の録音は含めません。編集後は再確定が必要です。</p><label>HN<input data-hn autocomplete="off"></label><label>6桁のワンタイムキー<input data-key type="password" inputmode="numeric" maxlength="6" autocomplete="off"></label><p>同じHNとキーをスマートフォンのWeb画面に入力すると取得できます。暗号化ZIPの保存期間は24時間です。キーはこのPCへ保存しません。</p><p data-status role="status"></p></div><footer><button data-web>スクリプト送信ウェブを開く</button><button data-send class="primary">台本ZIPを送信</button><button data-cancel>キャンセル</button></footer></section>`;
 overlay.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=close;
 overlay.querySelector<HTMLButtonElement>('[data-web]')!.onclick=()=>void context.rpc('open_settings_link',{kind:'web'}).catch(error=>{status(String(error));context.notice(String(error),true);});
 bindDialogKeys(overlay,close);
 overlay.querySelector<HTMLButtonElement>('[data-send]')!.onclick=async()=>{
  if(working)return;
  const hn=overlay.querySelector<HTMLInputElement>('[data-hn]')!.value.trim(),key=overlay.querySelector<HTMLInputElement>('[data-key]')!.value;
  if(!hn||!/^\d{6}$/.test(key)){status('HNと6桁のワンタイムキーを入力してください');return;}
  working=true;update();
  try{
   if(!await(context.confirm?context.confirm('確定した選択原稿をScriptSyncWebへ送信しますか？'):window.confirm('確定した選択原稿をScriptSyncWebへ送信しますか？')))return;
   status('ZIPを作成して送信しています…');
   const result=await context.rpc<{message:string}>('cloud_upload',{provider:'web',project_id:context.projectId,source_id:sourceId,base_generation:context.generation,base_revision:context.scriptRevision,exported_at:new Date().toISOString(),hn,key});
   status(result.message);context.notice(result.message);
  }catch(error){status(String(error));context.notice(String(error),true);}finally{working=false;update();}
 };
 overlay.querySelector<HTMLInputElement>('[data-hn]')!.focus();
}
