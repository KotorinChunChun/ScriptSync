import {beforeEach,expect,it,vi} from 'vitest';
import {openCloudUpload,openCloudSettings} from '../../src/cloudUploadView';
const settle=async()=>{for(let i=0;i<12;i++)await Promise.resolve();};
beforeEach(()=>{document.body.innerHTML='';});
it('Web送信はHNと6桁キーを確認し確定原稿の識別子だけを送信する',async()=>{
 const rpc=vi.fn(async()=>({message:'送信完了'}));const confirm=vi.fn(async()=>true);
 await openCloudUpload({rpc,confirm,notice:vi.fn(),projectId:'project'},'source','web');
 document.querySelector<HTMLButtonElement>('[data-send]')!.click();await settle();expect(rpc).not.toHaveBeenCalled();
 document.querySelector<HTMLInputElement>('[data-hn]')!.value='fixture';document.querySelector<HTMLInputElement>('[data-key]')!.value='123456';
 document.querySelector<HTMLButtonElement>('[data-send]')!.click();await settle();
 expect(confirm).toHaveBeenCalledOnce();expect(rpc).toHaveBeenCalledWith('cloud_upload',expect.objectContaining({provider:'web',project_id:'project',source_id:'source',hn:'fixture',key:'123456'}));expect(document.querySelector('[data-status]')!.textContent).toBe('送信完了');
});
it('送信確認を取り消すと送信しない',async()=>{
 const rpc=vi.fn(async()=>({onedrive:true}));await openCloudUpload({rpc,confirm:async()=>false,notice:vi.fn(),projectId:'p'},'s','onedrive');
 document.querySelector<HTMLButtonElement>('[data-send]')!.click();await settle();expect(rpc.mock.calls.filter(([action])=>action==='cloud_upload')).toHaveLength(0);
});
it('認証情報がない場合送信を無効にし登録エラーを表示する',async()=>{
 const rpc=vi.fn(async(action:string)=>{if(action==='cloud_status')return {onedrive:false};throw new Error('クライアントIDが必要');});
 await openCloudUpload({rpc,notice:vi.fn(),projectId:'p'},'s','onedrive');expect(document.querySelector<HTMLButtonElement>('[data-send]')!.disabled).toBe(true);
 document.querySelector<HTMLButtonElement>('[data-auth]')!.click();await settle();expect(document.querySelector('[data-status]')!.textContent).toContain('クライアントIDが必要');expect(document.querySelector<HTMLButtonElement>('[data-send]')!.disabled).toBe(true);
});
it('送信失敗を成功と表示せず閉じる操作で認証待ちを解除する',async()=>{
 const notice=vi.fn();const rpc=vi.fn(async(action:string)=>{if(action==='cloud_status')return {box:true};if(action==='cloud_upload')throw new Error('送信失敗');return {};});
 await openCloudUpload({rpc,confirm:async()=>true,notice,projectId:'p'},'s','box');document.querySelector<HTMLButtonElement>('[data-send]')!.click();await settle();expect(notice).toHaveBeenCalledWith('Error: 送信失敗',true);document.querySelector<HTMLButtonElement>('[data-close]')!.click();expect(document.querySelector('[role=dialog]')).toBeNull();expect(rpc).toHaveBeenCalledWith('cloud_auth_cancel',{provider:'box'});
});
it('設定案内はBoxの登録URIとsecretを保存しない方針を表示する',()=>{
 openCloudSettings({rpc:vi.fn(),notice:vi.fn()});expect(document.body.textContent).toContain('http://127.0.0.1:8767/callback');expect(document.body.textContent).toContain('ディスクには保存しません');
});
it('Box認証中に閉じた場合は遅れて完了した認証開始も解除する',async()=>{
 let complete!:(value:unknown)=>void;const rpc=vi.fn((action:string)=>{if(action==='cloud_status')return Promise.resolve({box:false});if(action==='cloud_auth_start')return new Promise(resolve=>complete=resolve);return Promise.resolve({});});
 await openCloudUpload({rpc:rpc as any,notice:vi.fn(),projectId:'p'},'s','box');document.querySelector<HTMLInputElement>('[data-secret]')!.value='fixture-secret';document.querySelector<HTMLButtonElement>('[data-auth]')!.click();document.querySelector<HTMLButtonElement>('[data-close]')!.click();
 complete({url:'https://account.box.com/api/oauth2/authorize',interval:2});await settle();expect(rpc.mock.calls.filter(([action])=>action==='cloud_auth_cancel')).toHaveLength(2);expect(rpc.mock.calls.filter(([action])=>action==='cloud_open_auth')).toHaveLength(0);
});
it('設定上のクラウド案内は初期フォーカスとTabを閉じ込めEscapeで起動元へ戻す',()=>{
 document.body.innerHTML='<section role="dialog" id="settings"><button id="launcher">案内</button><button id="behind">保存</button></section>';
 const launcher=document.querySelector<HTMLButtonElement>('#launcher')!;launcher.focus();const behindKey=vi.fn();document.querySelector('#settings')!.addEventListener('keydown',behindKey);
 openCloudSettings({rpc:vi.fn(),notice:vi.fn()});const overlay=document.querySelector<HTMLElement>('.settings-overlay')!;
 const first=overlay.querySelector<HTMLButtonElement>('[data-close]')!,last=overlay.querySelector<HTMLButtonElement>('[data-logout]')!;expect(document.activeElement).toBe(first);
 first.dispatchEvent(new KeyboardEvent('keydown',{key:'Tab',shiftKey:true,bubbles:true,cancelable:true}));expect(document.activeElement).toBe(last);
 last.dispatchEvent(new KeyboardEvent('keydown',{key:'Tab',bubbles:true,cancelable:true}));expect(document.activeElement).toBe(first);
 const escape=new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true});first.dispatchEvent(escape);expect(escape.defaultPrevented).toBe(true);expect(overlay.isConnected).toBe(false);expect(document.activeElement).toBe(launcher);expect(behindKey).not.toHaveBeenCalled();
});
it('入力のないOneDrive送信も初期フォーカスを持ちTabを循環しEscapeで戻る',async()=>{
 document.body.innerHTML='<button id="launcher">送信</button>';const launcher=document.querySelector<HTMLButtonElement>('#launcher')!;launcher.focus();
 const rpc=vi.fn(async()=>({onedrive:false}));await openCloudUpload({rpc,notice:vi.fn(),projectId:'p'},'s','onedrive');
 const first=document.querySelector<HTMLButtonElement>('[data-close]')!,last=document.querySelector<HTMLButtonElement>('[data-cancel]')!;expect(document.activeElement).toBe(first);
 first.dispatchEvent(new KeyboardEvent('keydown',{key:'Tab',shiftKey:true,bubbles:true,cancelable:true}));expect(document.activeElement).toBe(last);
 last.dispatchEvent(new KeyboardEvent('keydown',{key:'Tab',bubbles:true,cancelable:true}));expect(document.activeElement).toBe(first);
 first.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));expect(document.querySelector('.settings-overlay')).toBeNull();expect(document.activeElement).toBe(launcher);
});
