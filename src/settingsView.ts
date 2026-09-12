import qrcode from 'qrcode-generator';
import {escapeHtml as esc} from './html';
import './settingsView.css';
import {openCloudSettings} from './cloudUploadView';

export type MachineSettings = {
  capture_endpoint:string; render_endpoint:string; asr_provider:string; asr_model:string; codex_path:string;
  entra_tenant:string; entra_client_id:string; onedrive_folder:string; box_client_id:string; box_folder:string;
};
type Device={endpoint_id:string;friendly_name:string;is_default:boolean};
type Snapshot={settings:MachineSettings;web_url:string;runtime_root:string;ready:boolean;setup:{running:boolean;phase:string;message:string};load_error?:string};
type Context={rpc:<T>(action:string,payload?:unknown)=>Promise<T>;notice:(text:string,error?:boolean)=>void;onSaved?:(settings:MachineSettings)=>void|Promise<void>};

export async function openSettings(context:Context) {
  if(document.getElementById('settings-manager'))return;
  const overlay=document.createElement('div');overlay.id='settings-manager';overlay.className='settings-overlay';
  overlay.setAttribute('role','dialog');overlay.setAttribute('aria-modal','true');overlay.setAttribute('aria-label','設定');
  const previous=document.activeElement as HTMLElement|null;
  const siblings=[...document.body.children] as HTMLElement[],inert=siblings.map(el=>el.inert);
  siblings.forEach(el=>el.inert=true);document.body.append(overlay);
  let snapshot:Snapshot|undefined,devices:{capture:Device[];render:Device[]}={capture:[],render:[]},message='',saving=false;
  let poll:ReturnType<typeof setTimeout>|undefined;
  const close=()=>{if(saving)return;clearTimeout(poll);overlay.remove();siblings.forEach((el,i)=>el.inert=inert[i]);previous?.focus();};
  const options=(rows:Device[],selected:string)=>`<option value="">Windowsの既定のデバイス</option>${selected&&!rows.some(row=>row.endpoint_id===selected)?`<option value="${esc(selected)}" selected>選択したデバイスが見つかりません</option>`:''}${rows.map(row=>`<option value="${esc(row.endpoint_id)}" ${row.endpoint_id===selected?'selected':''}>${esc(row.friendly_name)}${row.is_default?'（現在の既定）':''}</option>`).join('')}`;
  const read=():MachineSettings=>{const next={...snapshot!.settings};for(const key of Object.keys(next) as (keyof MachineSettings)[]){const input=overlay.querySelector<HTMLInputElement|HTMLSelectElement>(`[name="${key}"]`);if(input)next[key]=input.value;}return next;};
  const notify=(text:string)=>{message=text;const target=overlay.querySelector('[data-message]');if(target)target.textContent=text;};
  const updateStatus=()=>{const target=overlay.querySelector('[data-setup-status]');if(target&&snapshot)target.textContent=snapshot.setup.message||(snapshot.ready?'選択したローカルモデルは導入済みです':'選択した認識モデルの導入が必要です');const button=overlay.querySelector<HTMLButtonElement>('[data-setup]');if(button&&snapshot)button.disabled=snapshot.setup.running||saving;};
  const refreshStatus=async()=>{try{const next=await context.rpc<Snapshot>('asr_setup_status');if(!overlay.isConnected)return;snapshot={...next,settings:snapshot?.settings??next.settings};updateStatus();if(next.setup.running)poll=setTimeout(()=>void refreshStatus(),1200);}catch(error){notify(String(error));}};
  const save=async()=>{if(saving||!snapshot)return false;saving=true;try{snapshot=await context.rpc<Snapshot>('settings_save',{settings:read()});await context.onSaved?.(snapshot.settings);notify('このPCの設定を保存しました');return true;}catch(error){notify(String(error));return false;}finally{saving=false;}};
  const draw=()=>{
    if(!snapshot){overlay.innerHTML='<section class="settings-dialog"><h2>設定</h2><p>設定を読み込んでいます…</p><button data-close>閉じる</button></section>';overlay.querySelector<HTMLButtonElement>('[data-close]')!.onclick=close;return;}
    const s=snapshot.settings,qr=qrcode(0,'M');qr.addData(snapshot.web_url);qr.make();
    overlay.innerHTML=`<section class="settings-dialog"><header><h2>設定</h2><button class="button-large" data-close>閉じる</button></header><p data-message role="status">${esc(message||snapshot.load_error||'このPCに保存します。プロジェクトの保存状態には影響しません。')}</p><div class="settings-scroll">
      <fieldset><legend>音声デバイス</legend><label>録音デバイス<select name="capture_endpoint">${options(devices.capture,s.capture_endpoint)}</select></label><label>再生デバイス<select name="render_endpoint">${options(devices.render,s.render_endpoint)}</select></label><p>「既定」はWindowsで変更したデバイスに追従します。選択した機器がない場合は、選び直してください。録音・認識中は設定を変更できません。</p><button data-refresh-devices>デバイス一覧を更新</button></fieldset>
      <fieldset><legend>文字起こし</legend><label>方式<select name="asr_provider"><option value="local" ${s.asr_provider==='local'?'selected':''}>ローカル（faster-whisper）</option><option value="codex" ${s.asr_provider==='codex'?'selected':''}>Codex CLI経由（ローカル認識ツールを実行）</option></select></label><label>認識モデル<select name="asr_model">${[['tiny','tiny（軽量）'],['base','base'],['small','small（標準）'],['medium','medium'],['large-v3','large-v3（高精度・大容量）']].map(([value,label])=>`<option value="${value}" ${value===s.asr_model?'selected':''}>${label}</option>`).join('')}</select></label><p>初回はPython未導入でも、ボタン1つでこのユーザー専用のPython・ライブラリ・モデルを取得します。インターネット接続と空き容量が必要です。通常のローカル認識では音声を外部送信しません。</p><p data-setup-status role="status"></p><button data-setup>文字起こし環境を導入／再試行</button><details><summary>保存先・Codex CLI・追加方式</summary><p>導入先：<code>${esc(snapshot.runtime_root)}</code></p><label>Codex実行ファイル（空欄はPATHから検索）<input name="codex_path" value="${esc(s.codex_path)}" placeholder="C:\…\codex.exe"></label><p>Codex CLIの導入とログインが別途必要です。CLIが同じfaster-whisperを呼び出す方式なので、独立モデルの比較ではありません。認識したテキストがCodexサービスに送信されます。音声そのものはローカルで処理します。CLIが実際に認識ツールを実行しなかった場合は失敗として扱います。</p><button data-link="codex">Codex CLIの導入案内</button><button data-link="whisper">認識ライブラリの案内</button><p>追加候補：OpenAI音声認識API（音声の外部送信・API認証・従量課金が必要）。現在の選択から自動切替しません。</p><button data-link="api">追加方式の導入案内</button></details></fieldset>
      <fieldset><legend>ScriptSyncWeb</legend><div class="settings-web"><div><p class="settings-url">${esc(snapshot.web_url)}</p><button data-link="web">ScriptSyncWebを開く</button><p>スマートフォンでは右のQRコードから開けます。</p></div><div class="settings-qr" aria-label="ScriptSyncWebのQRコード">${qr.createSvgTag({cellSize:4,margin:4,scalable:true})}</div></div></fieldset>
      <fieldset><legend>会社のクラウド連携</legend><p>アプリ登録の識別子と保存先です。認証情報を入力する欄ではありません。送信時にユーザー認証します。</p><label>Entra テナントID<input name="entra_tenant" value="${esc(s.entra_tenant)}"></label><label>Entra アプリのクライアントID<input name="entra_client_id" value="${esc(s.entra_client_id)}"></label><label>OneDrive 保存先フォルダー<input name="onedrive_folder" value="${esc(s.onedrive_folder)}"></label><label>Box クライアントID<input name="box_client_id" value="${esc(s.box_client_id)}"></label><label>Box 保存先フォルダーID<input name="box_folder" value="${esc(s.box_folder)}"></label></fieldset>
      </div><footer><button data-save class="primary button-large">設定を保存</button><button data-cancel>キャンセル</button></footer></section>`;
    overlay.querySelector<HTMLButtonElement>('[data-close]')!.onclick=close;
    overlay.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=close;
    overlay.querySelector<HTMLButtonElement>('[data-save]')!.onclick=()=>void save();
    const cloudGuide=document.createElement('button');cloudGuide.textContent='クラウド連携の導入案内';cloudGuide.onclick=()=>openCloudSettings(context);overlay.querySelector('fieldset:last-child')?.append(cloudGuide);
    overlay.querySelector<HTMLButtonElement>('[data-refresh-devices]')!.onclick=async()=>{try{snapshot!.settings=read();devices=await context.rpc('settings_devices');draw();}catch(error){notify(String(error));}};
    overlay.querySelectorAll<HTMLButtonElement>('[data-link]').forEach(button=>button.onclick=()=>void context.rpc('open_settings_link',{kind:button.dataset.link}).catch(error=>notify(String(error))));
    overlay.querySelector<HTMLButtonElement>('[data-setup]')!.onclick=async()=>{if(!await save())return;try{await context.rpc('asr_setup');await refreshStatus();}catch(error){notify(String(error));}};
    updateStatus();
  };
  overlay.onkeydown=event=>{event.stopPropagation();if(event.key==='Escape'){event.preventDefault();close();}if(event.key==='Tab'){const focus=[...overlay.querySelectorAll<HTMLElement>('button:not(:disabled),input,select,summary')].filter(el=>el.getClientRects().length>0);const index=focus.indexOf(document.activeElement as HTMLElement);if(event.shiftKey&&index<=0){event.preventDefault();focus.at(-1)?.focus();}else if(!event.shiftKey&&index===focus.length-1){event.preventDefault();focus[0]?.focus();}}};
  draw();
  try{snapshot=await context.rpc<Snapshot>('machine_settings');try{devices=await context.rpc('settings_devices');}catch(error){message=`デバイス一覧を取得できませんでした：${String(error)}`;}if(!overlay.isConnected)return;draw();overlay.querySelector<HTMLButtonElement>('[data-close]')?.focus();if(snapshot.setup.running)void refreshStatus();}catch(error){notify(String(error));context.notice(String(error),true);close();}
}
