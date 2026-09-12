import { escapeHtml as esc } from './html';
export type CloudContext={rpc:<T>(action:string,payload?:unknown)=>Promise<T>;notice:(message:string,error?:boolean)=>void;confirm?:(message:string)=>Promise<boolean>|boolean;projectId?:string;openSettings?:()=>void|Promise<void>};
type Provider='web'|'onedrive'|'box';
const labels:Record<Provider,string>={web:'ScriptSyncWeb',onedrive:'会社のOneDrive',box:'Box'};
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
/** 台本確定とは別の操作。送信するのは最後に確定した選択原稿だけ。 */
export async function openCloudUpload(context:CloudContext,sourceId:string,provider:Provider){
 const previousFocus=document.activeElement instanceof HTMLElement?document.activeElement:null;
 const overlay=document.createElement('div');overlay.className='settings-overlay';overlay.setAttribute('role','dialog');overlay.setAttribute('aria-modal','true');overlay.setAttribute('aria-label',`${labels[provider]}に台本を送信`);document.body.append(overlay);
 let timer:ReturnType<typeof setTimeout>|undefined,working=false,authPending=false,authenticated=provider==='web',closed=false;
 const status=(text:string)=>{const node=overlay.querySelector('[data-status]');if(node)node.textContent=text;};
 const update=()=>{const send=overlay.querySelector<HTMLButtonElement>('[data-send]');if(send)send.disabled=working||authPending||!authenticated;const auth=overlay.querySelector<HTMLButtonElement>('[data-auth]');if(auth)auth.disabled=working||authPending;};
 const close=()=>{if(working)return;closed=true;clearTimeout(timer);void context.rpc('cloud_auth_cancel',{provider}).catch(()=>{});overlay.remove();if(previousFocus?.isConnected)previousFocus.focus();};
 overlay.innerHTML=`<section class="settings-dialog"><header><h2>${esc(labels[provider])}に台本を送信</h2><button data-close>閉じる</button></header><div class="settings-scroll"><p>最後に確定した選択原稿をZIPで送信します。未確定の編集と既存の録音は含めません。</p>${provider==='web'?'<label>HN<input data-hn autocomplete="off"></label><label>6桁のワンタイムキー<input data-key type="password" inputmode="numeric" maxlength="6" autocomplete="off"></label><p>同じHNとキーをスマートフォンのWeb画面に入力すると取得できます。暗号化ZIPの保存期間は24時間です。キーはこのPCへ保存しません。</p>':`<p>設定画面のアプリ登録と保存先を使用します。OneDriveの保存先には既存フォルダーのパス、BoxにはフォルダーIDが必要です。</p>${provider==='box'?'<label>Box client secret<input data-secret type="password" autocomplete="off"></label><p>管理者が登録したOAuthアプリのsecretです。認証の間だけメモリーに保持し、設定ファイルには保存しません。共有アプリのsecretをアプリ本体に同梱しないでください。</p>':''}<button data-auth>ユーザー認証を開始</button><div data-auth-info></div>`}<p data-status role="status"></p></div><footer><button data-settings>詳細設定</button><button data-send class="primary">台本ZIPを送信</button><button data-cancel>キャンセル</button></footer></section>`;
 overlay.querySelector<HTMLButtonElement>('[data-close]')!.onclick=close;overlay.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=close;
 overlay.querySelector<HTMLButtonElement>('[data-settings]')!.onclick=()=>{if(working)return;close();void context.openSettings?.();};
 bindDialogKeys(overlay,close);
 const poll=async()=>{try{const result=await context.rpc<{pending?:boolean;ready?:boolean;interval?:number}>('cloud_auth_poll',{provider});if(closed)return;if(result.ready){authPending=false;authenticated=true;status('認証できました。送信ボタンを押すと原稿を送ります。');update();}else{timer=setTimeout(()=>void poll(),Math.max(2,result.interval??5)*1000);}}catch(error){if(closed)return;authPending=false;status(String(error));update();}};
 overlay.querySelector<HTMLButtonElement>('[data-auth]')?.addEventListener('click',async()=>{
  if(working||authPending)return;authPending=true;update();status('認証を開始しています…');
  const secret=overlay.querySelector<HTMLInputElement>('[data-secret]')?.value??'';
  try{const result=await context.rpc<{url:string;user_code?:string;interval?:number}>('cloud_auth_start',{provider,secret});const secretInput=overlay.querySelector<HTMLInputElement>('[data-secret]');if(secretInput)secretInput.value='';if(closed){void context.rpc('cloud_auth_cancel',{provider}).catch(()=>{});return;}
   const info=overlay.querySelector('[data-auth-info]')!;info.innerHTML=`${result.user_code?`<p>Microsoftの画面で入力するコード：<strong>${esc(result.user_code)}</strong></p>`:''}<button data-open-auth>認証画面を開く</button>`;
   info.querySelector<HTMLButtonElement>('[data-open-auth]')!.onclick=()=>void context.rpc('cloud_open_auth',{url:result.url}).catch(error=>status(String(error)));
   await context.rpc('cloud_open_auth',{url:result.url});if(closed)return;status('ブラウザーで認証してください。完了後にこの画面へ戻ります。');timer=setTimeout(()=>void poll(),Math.max(2,result.interval??5)*1000);
  }catch(error){if(closed)return;authPending=false;status(String(error));update();}
 });
 overlay.querySelector<HTMLButtonElement>('[data-send]')!.onclick=async()=>{
  if(working||authPending||!authenticated)return;
  const hn=overlay.querySelector<HTMLInputElement>('[data-hn]')?.value??'',key=overlay.querySelector<HTMLInputElement>('[data-key]')?.value??'';
  if(provider==='web'&&(!hn||!/^\d{6}$/.test(key))){status('HNと6桁のワンタイムキーを入力してください。');return;}
  working=true;update();
  try{const message=`最後に確定した選択原稿を${labels[provider]}へ送信しますか？`;if(!await(context.confirm?context.confirm(message):window.confirm(message)))return;
   status('ZIPを作成して送信しています…');const result=await context.rpc<{message:string}>('cloud_upload',{provider,project_id:context.projectId,source_id:sourceId,exported_at:new Date().toISOString(),hn,key});status(result.message);context.notice(result.message);
  }catch(error){status(String(error));context.notice(String(error),true);}finally{working=false;update();}
 };
 update();(overlay.querySelector<HTMLElement>('input')??overlay.querySelector<HTMLElement>('[data-close]'))?.focus();
 if(provider!=='web'){try{const result=await context.rpc<Record<string,boolean>>('cloud_status');if(closed)return;authenticated=!!result[provider];status(authenticated?'認証済みです。送信できます。':'送信前にユーザー認証してください。');update();}catch(error){status(String(error));}}
}
export function openCloudSettings(context:CloudContext){
 const previousFocus=document.activeElement instanceof HTMLElement?document.activeElement:null;
 const overlay=document.createElement('div');overlay.className='settings-overlay';overlay.setAttribute('role','dialog');overlay.setAttribute('aria-modal','true');overlay.setAttribute('aria-label','クラウド連携の導入案内');
 overlay.innerHTML=`<section class="settings-dialog"><header><h2>クラウド連携の導入案内</h2><button data-close>閉じる</button></header><div class="settings-scroll"><h3>ScriptSyncWeb</h3><p>台本画面から送信先にWebを選び、HNと6桁キーを指定します。スマートフォンでは同じHNとキーでZIPを取得します。</p><h3>会社のOneDrive</h3><p>会社のEntra管理者にアプリ登録を依頼してください。委任アクセス権 Files.ReadWrite とパブリッククライアントフローを有効にします。テナントIDとクライアントIDを設定し、既存の保存先フォルダーのパス（例：ScriptSync）を指定します。会社の条件付きアクセスでデバイスコード認証が禁止されている場合は管理者の確認が必要です。</p><h3>Box</h3><p>Box開発者コンソールでStandard OAuth 2.0アプリを登録し、リダイレクトURIを <code>http://127.0.0.1:8767/callback</code> にします。クライアントIDと保存先フォルダーIDを設定します。secretは送信画面で認証時に入力し、ディスクには保存しません。会社の承認が必要な場合があります。EntraへのログインだけではBoxのAPI権限は得られません。</p><p>認証トークンはメモリーにだけ保持します。アプリを終了すると再認証が必要です。登録が未完了の状態では送信できません。</p><button data-logout>クラウドの認証を解除</button><p data-status role="status"></p></div></section>`;
 const close=()=>{overlay.remove();if(previousFocus?.isConnected)previousFocus.focus();};overlay.querySelector<HTMLButtonElement>('[data-close]')!.onclick=close;bindDialogKeys(overlay,close);overlay.querySelector<HTMLButtonElement>('[data-logout]')!.onclick=async()=>{try{await context.rpc('cloud_logout');overlay.querySelector('[data-status]')!.textContent='このアプリの認証を解除しました。';}catch(error){overlay.querySelector('[data-status]')!.textContent=String(error);}};document.body.append(overlay);overlay.querySelector<HTMLElement>('[data-close]')?.focus();
}
