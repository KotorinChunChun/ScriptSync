import qrcode from 'qrcode-generator';
import {escapeHtml as esc} from './html';
import './settingsView.css';
import {showTransientError} from './transientError';
import {appInfo} from './appInfo';
import {confirmAction} from './confirmAction';
import {bindBackdropCancel} from './popupDialog';
import {serviceToken,setServiceToken} from './services';

export type MachineSettings = {
  capture_endpoint:string; render_endpoint:string; asr_provider:string; asr_model:string; codex_path:string;
  codex_prompt:string; codex_model:string; word_replacements?:{from:string;to:string}[];
  service_url?:string;ai_provider?:string;tts_model?:string;tts_voice?:string;
};
type Device={endpoint_id:string;friendly_name:string;is_default:boolean};
type Snapshot={settings:MachineSettings;web_url:string;runtime_root:string;ready:boolean;installed?:boolean;setup:{running:boolean;phase:string;message:string};browser?:boolean;load_error?:string};
type Context={rpc:<T>(action:string,payload?:unknown)=>Promise<T>;notice:(text:string,error?:boolean)=>void;onSaved?:(settings:MachineSettings)=>void|Promise<void>};
let settingsDirty:()=>boolean=()=>false;
let settingsSave:(()=>Promise<void>)|undefined;
export function hasUnsavedSettings(){return settingsDirty();}
export async function saveOpenSettings(){await settingsSave?.();}

export async function openSettings(context:Context) {
  if(document.getElementById('settings-manager'))return;
  const overlay=document.createElement('div');overlay.id='settings-manager';overlay.className='settings-overlay';
  overlay.setAttribute('role','dialog');overlay.setAttribute('aria-modal','true');overlay.setAttribute('aria-label','設定');
  const previous=document.activeElement as HTMLElement|null;
  const siblings=[...document.body.children] as HTMLElement[],inert=siblings.map(el=>el.inert);
  siblings.forEach(el=>el.inert=true);document.body.append(overlay);
  let snapshot:Snapshot|undefined,devices:{capture:Device[];render:Device[]}={capture:[],render:[]},message='',saving=false,testing=false;
  let poll:ReturnType<typeof setTimeout>|undefined;
  let savedSettings:MachineSettings|undefined;
  const close=()=>{if(saving)return;clearTimeout(poll);overlay.remove();settingsDirty=()=>false;settingsSave=undefined;siblings.forEach((el,i)=>el.inert=inert[i]);previous?.focus();};
  bindBackdropCancel(overlay,close);
  const options=(rows:Device[],selected:string)=>`<option value="">既定のデバイス</option>${selected&&!rows.some(row=>row.endpoint_id===selected)?`<option value="${esc(selected)}" selected>選択したデバイスが見つかりません</option>`:''}${rows.map(row=>`<option value="${esc(row.endpoint_id)}" ${row.endpoint_id===selected?'selected':''}>${esc(row.friendly_name)}${row.is_default?'（現在の既定）':''}</option>`).join('')}`;
  const read=():MachineSettings=>{const next={service_url:'http://localhost:4318',ai_provider:'codex',tts_model:'default',tts_voice:'default',...snapshot!.settings};const tokenInput=overlay.querySelector<HTMLInputElement>('[data-service-token]');if(tokenInput)setServiceToken(tokenInput.value);for(const key of Object.keys(next) as (keyof MachineSettings)[]){const input=overlay.querySelector<HTMLInputElement|HTMLSelectElement|HTMLTextAreaElement>(`[name="${key}"]`);if(input&&key!=='word_replacements')next[key]=input.value;}next.word_replacements=[...overlay.querySelectorAll<HTMLElement>('[data-word-row]')].map(row=>({from:row.querySelector<HTMLInputElement>('[data-word-from]')!.value,to:row.querySelector<HTMLInputElement>('[data-word-to]')!.value}));return next;};
  const notify=(text:string)=>{message=text;const target=overlay.querySelector('[data-message]');if(target)target.textContent=text;};
  const updateStatus=()=>{const target=overlay.querySelector('[data-setup-status]');if(target&&snapshot)target.textContent=snapshot.setup.message||(snapshot.ready?'選択したローカルモデルは導入済みです':'選択した認識モデルの導入が必要です');const button=overlay.querySelector<HTMLButtonElement>('[data-setup]');if(button&&snapshot){button.disabled=snapshot.setup.running||saving||testing;button.hidden=snapshot.ready&&overlay.querySelector<HTMLSelectElement>('[name="asr_model"]')?.value===snapshot.settings.asr_model;}const remove=overlay.querySelector<HTMLButtonElement>('[data-uninstall]');if(remove&&snapshot){remove.hidden=!(snapshot.installed??snapshot.ready);remove.disabled=snapshot.setup.running||saving||testing;}const check=overlay.querySelector<HTMLButtonElement>('[data-check]');if(check&&snapshot)check.disabled=snapshot.setup.running||saving||testing;};
  const refreshStatus=async()=>{try{const next=await context.rpc<Snapshot>('asr_setup_status');if(!overlay.isConnected)return;snapshot={...next,settings:snapshot?.settings??next.settings};updateStatus();if(next.setup.running)poll=setTimeout(()=>void refreshStatus(),1200);}catch(error){notify(String(error));}};
  const save=async()=>{if(saving||testing||!snapshot)return false;saving=true;try{snapshot=await context.rpc<Snapshot>('settings_save',{settings:read()});savedSettings={...snapshot.settings,word_replacements:snapshot.settings.word_replacements??[]};await context.onSaved?.(snapshot.settings);notify('このPCの設定を保存しました');return true;}catch(error){notify(String(error));return false;}finally{saving=false;}};
  settingsDirty=()=>overlay.isConnected&&!!savedSettings&&JSON.stringify(read())!==JSON.stringify(savedSettings);
  settingsSave=async()=>{if(!await save())throw Error(message||'設定を保存できませんでした');};
  const draw=()=>{
    if(!snapshot){overlay.innerHTML='<section class="settings-dialog"><h2>設定</h2><p>設定を読み込んでいます…</p><footer class="popup-actions"><button data-close>閉じる</button></footer></section>';overlay.querySelector<HTMLButtonElement>('[data-close]')!.onclick=close;return;}
    const s=snapshot.settings,qr=qrcode(0,'M');qr.addData(snapshot.web_url);qr.make();
    overlay.innerHTML=`<section class="settings-dialog"><header><h2>設定</h2></header><p data-message role="status">${esc(message||snapshot.load_error||'このPCに保存します。プロジェクトの保存状態には影響しません。')}</p><div class="settings-scroll"><fieldset class="settings-app-info"><legend>アプリ情報</legend><p>${esc(appInfo.name)}</p><p>バージョン：<span data-app-version>${esc(appInfo.version)}</span></p><p>更新日：<time datetime="${appInfo.updatedAt}">${appInfo.updatedAt}</time></p></fieldset>
      <fieldset><legend>音声デバイス</legend><label>録音デバイス<select name="capture_endpoint">${options(devices.capture,s.capture_endpoint)}</select></label><label>再生デバイス<select name="render_endpoint">${options(devices.render,s.render_endpoint)}</select></label><p>「既定」はWindowsで変更したデバイスに追従します。選択した機器がない場合は、選び直してください。録音・認識中は設定を変更できません。</p><button data-refresh-devices>デバイス一覧を更新</button></fieldset>
      <fieldset><legend>文字起こし</legend><p>Whisperで文字起こしした後、必要に応じて「AIで文章改善」を別操作で実行します。</p><label>認識モデル<select name="asr_model">${[['tiny','tiny（軽量）'],['base','base'],['small','small（標準）'],['medium','medium'],['large-v3','large-v3（高精度・大容量）']].filter(([value])=>!snapshot!.browser||['tiny','base','small'].includes(value)).map(([value,label])=>`<option value="${value}" ${value===s.asr_model?'selected':''}>${label}</option>`).join('')}</select></label><p>${snapshot.browser?'ブラウザー内Whisperを使用します。初回はモデルを取得し、認識は端末内で行います。Web版ではtiny/base/smallを選択してください。':'初回はPython未導入でも、ボタン1つでこのユーザー専用のPython・ライブラリ・モデルを取得します。'}インターネット接続と空き容量が必要です。通常のローカル認識では音声を外部送信しません。</p><p data-setup-status role="status"></p><button data-check>Whisperの導入状態を確認</button><button data-setup>文字起こし環境を導入／再試行</button><button data-uninstall>Whisperをアンインストール</button><p>導入先：<code>${esc(snapshot.runtime_root)}</code></p><button data-link="whisper">Whisperライブラリの案内</button><section class="settings-codex" aria-label="AI・音声合成サービス"><h3>AI・音声合成サービス</h3><p>利用者側の接続ゲートウェイを指定します。Codex app-server・OpenAI API等の接続先と秘密はゲートウェイ側で管理します。音声合成は別のHTTPサーバーを使用します。</p><label>接続URL（既定localhost）<input name="service_url" value="${esc(s.service_url??'http://localhost:4318')}" placeholder="http://localhost:4318"></label><label>AI接続方式<select name="ai_provider">${[['codex','Codex app-server'],['openai','OpenAI Responses API'],['chat-completions','Chat Completions互換API']].map(([v,l])=>`<option value="${v}" ${(s.ai_provider??'codex')===v?'selected':''}>${l}</option>`).join('')}</select></label>${'__TAURI_INTERNALS__'in window?'<p>接続トークンは環境変数SCRIPTSYNC_GATEWAY_TOKEN、またはC:/AI/env/ScriptSync/gateway-token.txtへ設定します。</p>':`<label>接続トークン（このページを開いている間だけ保持）<input type="password" data-service-token autocomplete="off" value="${esc(serviceToken())}"></label>`}<label>AIモデル<input name="codex_model" value="${esc(s.codex_model)}"></label><label>音声合成モデル<input name="tts_model" value="${esc(s.tts_model??'default')}"></label><label>音声名<input name="tts_voice" value="${esc(s.tts_voice??'default')}"></label><label>文章改善プロンプト<textarea name="codex_prompt" rows="8">${esc(s.codex_prompt)}</textarea></label><p>「AIで文章改善」を押した時だけ、現在の台本の修正案・確定台本を送信します。結果は承認後に反映します。localhostは閲覧端末自身を指します。</p><button data-codex-test>AI接続の動作テスト</button><div data-codex-test-result role="status"></div><button data-link="codex">Codex app-serverの案内</button></section><p>OpenAI音声認識APIは参考情報です。本アプリからは実行しません。</p><button data-link="api">OpenAIファイル文字起こしの案内</button></fieldset>
      <fieldset><legend>単語の静的変換辞書</legend><p>AIへ送信する前に置換します。英数字は単語単位、日本語は一致部分を置換します。長い置換元を優先し、置換結果をさらに置換しません。</p><div data-word-list class="settings-word-list">${(s.word_replacements??[]).map(row=>`<div data-word-row class="settings-word-row"><label>置換元<input data-word-from value="${esc(row.from)}"></label><label>置換先<input data-word-to value="${esc(row.to)}"></label><button data-word-remove>削除</button></div>`).join('')}</div><button data-word-add>単語を追加</button></fieldset>
      <fieldset><legend>ScriptSyncWeb</legend><div class="settings-web"><div><p class="settings-url">${esc(snapshot.web_url)}</p><button data-link="web">ScriptSyncWebを開く</button><p>スマートフォンでは右のQRコードから開けます。</p></div><div class="settings-qr" aria-label="ScriptSyncWebのQRコード">${qr.createSvgTag({cellSize:4,margin:4,scalable:true})}</div></div></fieldset>

      </div><footer class="popup-actions"><button data-cancel class="button-large">閉じる</button><button data-save class="primary button-large">設定を保存</button></footer></section>`;
    overlay.querySelector<HTMLButtonElement>('[data-word-add]')!.onclick=()=>{snapshot!.settings=read();snapshot!.settings.word_replacements!.push({from:'',to:''});draw();overlay.querySelectorAll<HTMLInputElement>('[data-word-from]').item(snapshot!.settings.word_replacements!.length-1)?.focus();};
    overlay.querySelectorAll<HTMLButtonElement>('[data-word-remove]').forEach(button=>button.onclick=()=>button.closest('[data-word-row]')!.remove());
    overlay.querySelector<HTMLButtonElement>('[data-cancel]')!.onclick=close;
    overlay.querySelector<HTMLButtonElement>('[data-save]')!.onclick=()=>void save().then(saved=>{if(saved)close();});

    overlay.querySelector<HTMLButtonElement>('[data-refresh-devices]')!.onclick=async()=>{try{snapshot!.settings=read();devices=await context.rpc('settings_devices');draw();}catch(error){notify(String(error));}};
    overlay.querySelectorAll<HTMLButtonElement>('[data-link]').forEach(button=>button.onclick=()=>void context.rpc('open_settings_link',{kind:button.dataset.link}).catch(error=>notify(String(error))));
    overlay.querySelector<HTMLButtonElement>('[data-setup]')!.onclick=async()=>{if(!await save())return;try{await context.rpc('asr_setup');await refreshStatus();}catch(error){notify(String(error));}};
    overlay.querySelector<HTMLButtonElement>('[data-check]')!.onclick=async()=>{if(!await save())return;try{await context.rpc('asr_check');await refreshStatus();}catch(error){showTransientError(String(error));}};
    overlay.querySelector<HTMLButtonElement>('[data-uninstall]')!.onclick=async()=>{if(saving||testing||snapshot!.setup.running)return;if(!await confirmAction('Whisper専用の実行環境・ライブラリ・モデルを削除します。台本・録音は保持されます。アンインストールしますか？'))return;try{snapshot=await context.rpc<Snapshot>('asr_uninstall');updateStatus();}catch(error){showTransientError(String(error));}};
    overlay.querySelector<HTMLButtonElement>('[data-codex-test]')!.onclick=async()=>{
      if(testing||saving)return;testing=true;
      const button=overlay.querySelector<HTMLButtonElement>('[data-codex-test]')!,result=overlay.querySelector<HTMLElement>('[data-codex-test-result]')!;
      const guarded=[...overlay.querySelectorAll<HTMLButtonElement>('[data-save],[data-setup],[data-check],[data-uninstall],[data-refresh-devices]')],disabled=guarded.map(b=>b.disabled);guarded.forEach(b=>b.disabled=true);
      button.disabled=true;result.textContent='動作テスト中…';
      try{const response=await context.rpc<{input:string;text:string;model:string}>('text_improve_test',{settings:read()});if(!overlay.isConnected)return;result.innerHTML=`<p>入力：${esc(response.input)}</p><p>改善結果：${esc(response.text)}</p><p>モデル：${esc(response.model)}（軽）</p>`;}
      catch(error){if(overlay.isConnected){result.textContent='動作テストに失敗しました。';showTransientError(String(error));}}
      finally{testing=false;button.disabled=false;guarded.forEach((b,i)=>b.disabled=disabled[i]);}
    };
    overlay.querySelector<HTMLSelectElement>('[name="asr_model"]')!.onchange=updateStatus;
    updateStatus();
  };
  overlay.onkeydown=event=>{event.stopPropagation();if(event.key==='Escape'){event.preventDefault();close();}if(event.key==='Tab'){const focus=[...overlay.querySelectorAll<HTMLElement>('button:not(:disabled),input,select,textarea,summary')].filter(el=>el.getClientRects().length>0);const index=focus.indexOf(document.activeElement as HTMLElement);if(event.shiftKey&&index<=0){event.preventDefault();focus.at(-1)?.focus();}else if(!event.shiftKey&&index===focus.length-1){event.preventDefault();focus[0]?.focus();}}};
  draw();
  try{snapshot=await context.rpc<Snapshot>('machine_settings');snapshot.settings={service_url:'http://localhost:4318',ai_provider:'codex',tts_model:'default',tts_voice:'default',...snapshot.settings};savedSettings={...snapshot.settings,word_replacements:snapshot.settings.word_replacements??[]};try{devices=await context.rpc('settings_devices');}catch(error){message=`デバイス一覧を取得できませんでした：${String(error)}`;}if(!overlay.isConnected)return;draw();overlay.querySelector<HTMLSelectElement>('[name="capture_endpoint"]')?.focus();if(snapshot.setup.running)void refreshStatus();}catch(error){notify(String(error));context.notice(String(error),true);close();}
}
