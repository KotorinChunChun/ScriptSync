import qrcode from 'qrcode-generator';
import {escapeHtml as esc} from './html';
import './settingsView.css';

export type MachineSettings = {
  capture_endpoint:string; render_endpoint:string; asr_provider:string; asr_model:string; codex_path:string;
  codex_prompt:string;
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
  const read=():MachineSettings=>{const next={...snapshot!.settings};for(const key of Object.keys(next) as (keyof MachineSettings)[]){const input=overlay.querySelector<HTMLInputElement|HTMLSelectElement|HTMLTextAreaElement>(`[name="${key}"]`);if(input)next[key]=input.value;}return next;};
  const notify=(text:string)=>{message=text;const target=overlay.querySelector('[data-message]');if(target)target.textContent=text;};
  const updateStatus=()=>{const target=overlay.querySelector('[data-setup-status]');if(target&&snapshot)target.textContent=snapshot.setup.message||(snapshot.ready?'選択したローカルモデルは導入済みです':'選択した認識モデルの導入が必要です');const button=overlay.querySelector<HTMLButtonElement>('[data-setup]');if(button&&snapshot)button.disabled=snapshot.setup.running||saving;};
  const refreshStatus=async()=>{try{const next=await context.rpc<Snapshot>('asr_setup_status');if(!overlay.isConnected)return;snapshot={...next,settings:snapshot?.settings??next.settings};updateStatus();if(next.setup.running)poll=setTimeout(()=>void refreshStatus(),1200);}catch(error){notify(String(error));}};
  const save=async()=>{if(saving||!snapshot)return false;saving=true;try{snapshot=await context.rpc<Snapshot>('settings_save',{settings:read()});await context.onSaved?.(snapshot.settings);notify('このPCの設定を保存しました');return true;}catch(error){notify(String(error));return false;}finally{saving=false;}};
  const draw=()=>{
    if(!snapshot){overlay.innerHTML='<section class="settings-dialog"><h2>設定</h2><p>設定を読み込んでいます…</p><button data-close>閉じる</button></section>';overlay.querySelector<HTMLButtonElement>('[data-close]')!.onclick=close;return;}
    const s=snapshot.settings,qr=qrcode(0,'M');qr.addData(snapshot.web_url);qr.make();
    overlay.innerHTML=`<section class="settings-dialog"><header><h2>設定</h2></header><p data-message role="status">${esc(message||snapshot.load_error||'このPCに保存します。プロジェクトの保存状態には影響しません。')}</p><div class="settings-scroll">
      <fieldset><legend>音声デバイス</legend><label>録音デバイス<select name="capture_endpoint">${options(devices.capture,s.capture_endpoint)}</select></label><label>再生デバイス<select name="render_endpoint">${options(devices.render,s.render_endpoint)}</select></label><p>「既定」はWindowsで変更したデバイスに追従します。選択した機器がない場合は、選び直してください。録音・認識中は設定を変更できません。</p><button data-refresh-devices>デバイス一覧を更新</button></fieldset>
      <fieldset><legend>文字起こし</legend><p>Whisperで文字起こしした後、必要に応じて「Codex CLIで文章を改善」を別操作で実行します。</p><label>認識モデル<select name="asr_model">${[['tiny','tiny（軽量）'],['base','base'],['small','small（標準）'],['medium','medium'],['large-v3','large-v3（高精度・大容量）']].map(([value,label])=>`<option value="${value}" ${value===s.asr_model?'selected':''}>${label}</option>`).join('')}</select></label><p>初回はPython未導入でも、ボタン1つでこのユーザー専用のPython・ライブラリ・モデルを取得します。インターネット接続と空き容量が必要です。通常のローカル認識では音声を外部送信しません。</p><p data-setup-status role="status"></p><button data-setup>文字起こし環境を導入／再試行</button><p>導入先：<code>${esc(snapshot.runtime_root)}</code></p><button data-link="whisper">Whisperライブラリの案内</button><section class="settings-codex" aria-label="Codex CLI"><h3>Codex CLI</h3><label>Codex実行ファイル（空欄はPATHから検索）<input name="codex_path" value="${esc(s.codex_path)}" placeholder="codex.exe"></label><p>Codex CLIの導入とログインが別途必要です。「Codex CLIで文章を改善」を押したときだけ、現在の原稿テキストと下記プロンプトをCodexへ送信します。音声は送信しません。補正結果は原稿へ反映されるため、内容を確認してください。</p><label>文章改善プロンプト<textarea name="codex_prompt" rows="8">${esc(s.codex_prompt)}</textarea></label><button data-link="codex">Codex CLIの導入案内</button></section><p>OpenAI音声認識APIは参考情報です。本アプリからは実行しません。</p><button data-link="api">OpenAIファイル文字起こしの案内</button></fieldset>
      <fieldset><legend>ScriptSyncWeb</legend><div class="settings-web"><div><p class="settings-url">${esc(snapshot.web_url)}</p><button data-link="web">ScriptSyncWebを開く</button><p>スマートフォンでは右のQRコードから開けます。</p></div><div class="settings-qr" aria-label="ScriptSyncWebのQRコード">${qr.createSvgTag({cellSize:4,margin:4,scalable:true})}</div></div></fieldset>

      </div><footer><button data-save class="primary button-large">設定を保存</button><button data-cancel class="button-large">閉じる</button></footer></section>`;
    overlay.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=close;
    overlay.querySelector<HTMLButtonElement>('[data-save]')!.onclick=()=>void save().then(saved=>{if(saved)close();});

    overlay.querySelector<HTMLButtonElement>('[data-refresh-devices]')!.onclick=async()=>{try{snapshot!.settings=read();devices=await context.rpc('settings_devices');draw();}catch(error){notify(String(error));}};
    overlay.querySelectorAll<HTMLButtonElement>('[data-link]').forEach(button=>button.onclick=()=>void context.rpc('open_settings_link',{kind:button.dataset.link}).catch(error=>notify(String(error))));
    overlay.querySelector<HTMLButtonElement>('[data-setup]')!.onclick=async()=>{if(!await save())return;try{await context.rpc('asr_setup');await refreshStatus();}catch(error){notify(String(error));}};
    updateStatus();
  };
  overlay.onkeydown=event=>{event.stopPropagation();if(event.key==='Escape'){event.preventDefault();close();}if(event.key==='Tab'){const focus=[...overlay.querySelectorAll<HTMLElement>('button:not(:disabled),input,select,textarea,summary')].filter(el=>el.getClientRects().length>0);const index=focus.indexOf(document.activeElement as HTMLElement);if(event.shiftKey&&index<=0){event.preventDefault();focus.at(-1)?.focus();}else if(!event.shiftKey&&index===focus.length-1){event.preventDefault();focus[0]?.focus();}}};
  draw();
  try{snapshot=await context.rpc<Snapshot>('machine_settings');try{devices=await context.rpc('settings_devices');}catch(error){message=`デバイス一覧を取得できませんでした：${String(error)}`;}if(!overlay.isConnected)return;draw();overlay.querySelector<HTMLSelectElement>('[name="capture_endpoint"]')?.focus();if(snapshot.setup.running)void refreshStatus();}catch(error){notify(String(error));context.notice(String(error),true);close();}
}
