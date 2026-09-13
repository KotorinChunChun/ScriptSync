import {afterEach,describe,expect,it,vi} from 'vitest';
import {openSettings, type MachineSettings} from '../../src/settingsView';
import {appInfo} from '../../src/appInfo';
const settings:MachineSettings={capture_endpoint:'missing',render_endpoint:'',asr_provider:'local',asr_model:'small',codex_path:'',codex_model:'gpt-5.6-luna',codex_prompt:'誤字だけ修正してください。'};
const snapshot=()=>({settings:{...settings},web_url:'https://dev-k.sakura.ne.jp/tools/scriptsyncweb/',runtime_root:'C:/user/asr',ready:false,setup:{running:false,phase:'',message:''}});
const flush=()=>new Promise(resolve=>setTimeout(resolve,0));
afterEach(()=>{document.body.innerHTML='';vi.restoreAllMocks();});
describe('端末設定',()=>{
 it('アプリ情報に配布版と更新日を表示する',async()=>{const rpc=vi.fn(async(action:string)=>action==='settings_devices'?{capture:[],render:[]}:snapshot());await openSettings({rpc:rpc as never,notice:vi.fn()});const info=document.querySelector('.settings-app-info')!;expect(info.textContent).toContain(appInfo.name);expect(info.querySelector('[data-app-version]')?.textContent).toBe(appInfo.version);expect(info.querySelector('time')?.getAttribute('datetime')).toBe('2026-09-13');});
 it('消失したデバイスを既定へ黙って切り替えずキャンセルで保存しない',async()=>{const rpc=vi.fn(async(action:string)=>action==='settings_devices'?{capture:[],render:[]}:snapshot());await openSettings({rpc:rpc as never,notice:vi.fn()});const select=document.querySelector<HTMLSelectElement>('[name="capture_endpoint"]')!;expect(select.value).toBe('missing');expect(select.textContent).toContain('見つかりません');expect(document.querySelector('.settings-qr svg')).not.toBeNull();document.querySelector<HTMLButtonElement>('[data-cancel]')!.click();expect(rpc.mock.calls.some(call=>call[0]==='settings_save')).toBe(false);expect(document.getElementById('settings-manager')).toBeNull();});
 it('保存成功後のみ端末の変更を通知し保存失敗では反映しない',async()=>{const onSaved=vi.fn(),rpc=vi.fn(async(action:string)=>{if(action==='settings_devices')return {capture:[],render:[]};if(action==='settings_save')throw Error('保存できません');return snapshot();});await openSettings({rpc:rpc as never,notice:vi.fn(),onSaved});document.querySelector<HTMLSelectElement>('[name="asr_model"]')!.value='base';document.querySelector<HTMLButtonElement>('[data-save]')!.click();await flush();expect(onSaved).not.toHaveBeenCalled();expect(document.querySelector('[data-message]')!.textContent).toContain('保存できません');expect(document.querySelector<HTMLSelectElement>('[name="asr_model"]')!.value).toBe('base');});
 it('導入前にモデルを保存し認識環境の準備を要求する',async()=>{const rpc=vi.fn(async(action:string)=>action==='settings_devices'?{capture:[],render:[]}:snapshot());await openSettings({rpc:rpc as never,notice:vi.fn()});document.querySelector<HTMLButtonElement>('[data-setup]')!.click();await flush();const actions=rpc.mock.calls.map(call=>call[0]);expect(actions.indexOf('settings_save')).toBeLessThan(actions.indexOf('asr_setup'));expect(actions).toContain('asr_setup_status');expect(document.getElementById('settings-manager')).not.toBeNull();});
});

it('プロンプトを保存した成功時に閉じる。上部の閉じると旧方式はない',async()=>{
 const rpc=vi.fn(async(action:string,payload?:any)=>action==='settings_devices'?{capture:[],render:[]}:action==='settings_save'?{...snapshot(),settings:payload.settings}:snapshot());
 await openSettings({rpc:rpc as never,notice:vi.fn()});
 expect(document.querySelector('header [data-close]')).toBeNull();
 expect(document.querySelector('[name="asr_provider"]')).toBeNull();
 expect(document.querySelector('[name="codex_prompt"]')?.closest('details')).toBeNull();
 expect(document.querySelector('.settings-codex')?.textContent).toContain('現在の台本の修正案');
 expect(document.querySelector('[name="entra_client_id"]')).toBeNull();
 document.querySelector<HTMLTextAreaElement>('[name="codex_prompt"]')!.value='数値を保持。\n誤字のみ。';
 document.querySelector<HTMLButtonElement>('[data-save]')!.click();await flush();
 expect(rpc.mock.calls.find(c=>c[0]==='settings_save')?.[1].settings.codex_prompt).toBe('数値を保持。\n誤字のみ。');
 expect(document.getElementById('settings-manager')).toBeNull();
});
it('未保存のモデルとプロンプトで合成テストし、設定や原稿を変更しない',async()=>{
 const rpc=vi.fn(async(action:string,payload?:any)=>action==='settings_devices'?{capture:[],render:[]}:action==='text_improve_test'?{input:'合成入力',text:'合成結果',model:payload.settings.codex_model}:snapshot());
 await openSettings({rpc:rpc as never,notice:vi.fn()});
 expect(document.querySelector<HTMLSelectElement>('[name="codex_model"]')!.value).toBe('gpt-5.6-luna');
 document.querySelector<HTMLSelectElement>('[name="codex_model"]')!.value='gpt-5.6-terra';
 document.querySelector<HTMLTextAreaElement>('[name="codex_prompt"]')!.value='編集中の指示';
 document.querySelector<HTMLButtonElement>('[data-codex-test]')!.click();await flush();
 const payload=rpc.mock.calls.find(c=>c[0]==='text_improve_test')?.[1];
 expect(payload.settings.codex_model).toBe('gpt-5.6-terra');expect(payload.settings.codex_prompt).toBe('編集中の指示');
 expect(rpc.mock.calls.some(c=>c[0]==='settings_save'||c[0]==='recording_draft')).toBe(false);
 expect(document.querySelector('[data-codex-test-result]')?.textContent).toContain('合成結果');
 expect(document.getElementById('settings-manager')).not.toBeNull();
});
