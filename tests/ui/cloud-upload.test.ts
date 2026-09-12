import {beforeEach,expect,it,vi} from 'vitest';
import {openCloudUpload} from '../../src/cloudUploadView';
const settle=async()=>{for(let i=0;i<12;i++)await Promise.resolve();};
beforeEach(()=>{document.body.innerHTML='';});
function fill(){document.querySelector<HTMLInputElement>('[data-hn]')!.value='fixture';document.querySelector<HTMLInputElement>('[data-key]')!.value='123456';}
it('Web送信は入力を検査し確定世代と原稿を送信する',async()=>{
 const rpc=vi.fn(async()=>({message:'送信完了'}));const confirm=vi.fn(async()=>true);
 await openCloudUpload({rpc,confirm,notice:vi.fn(),projectId:'project',generation:3,scriptRevision:2},'source');
 document.querySelector<HTMLButtonElement>('[data-send]')!.click();await settle();expect(rpc).not.toHaveBeenCalled();
 fill();document.querySelector<HTMLButtonElement>('[data-send]')!.click();await settle();
 expect(rpc).toHaveBeenCalledWith('cloud_upload',expect.objectContaining({provider:'web',project_id:'project',source_id:'source',base_generation:3,base_revision:2,hn:'fixture',key:'123456'}));
 expect(document.querySelector('[data-status]')!.textContent).toBe('送信完了');
 expect(document.body.textContent).not.toMatch(/OneDrive|Box/);
});
it('送信確認の取消ではネットワーク呼出しを行わない',async()=>{
 const rpc=vi.fn();await openCloudUpload({rpc,confirm:async()=>false,notice:vi.fn()},'s');fill();document.querySelector<HTMLButtonElement>('[data-send]')!.click();await settle();expect(rpc).not.toHaveBeenCalled();expect(document.querySelector<HTMLButtonElement>('[data-send]')!.disabled).toBe(false);
});
it('送信失敗を表示して再試行可能にする',async()=>{
 const notice=vi.fn();await openCloudUpload({rpc:vi.fn(async()=>{throw Error('再確定してください')}),confirm:async()=>true,notice},'s');fill();document.querySelector<HTMLButtonElement>('[data-send]')!.click();await settle();expect(notice).toHaveBeenCalledWith('Error: 再確定してください',true);expect(document.querySelector<HTMLButtonElement>('[data-send]')!.disabled).toBe(false);
});
it('遅い送信中は二重送信と閉じるを防ぐ',async()=>{
 let finish!:(v:unknown)=>void;const rpc=vi.fn(()=>new Promise(resolve=>finish=resolve));await openCloudUpload({rpc:rpc as any,confirm:async()=>true,notice:vi.fn()},'s');fill();const send=document.querySelector<HTMLButtonElement>('[data-send]')!;send.click();await settle();send.click();document.querySelector<HTMLButtonElement>('[data-close]')!.click();expect(rpc).toHaveBeenCalledTimes(1);expect(document.querySelector('[role=dialog]')).not.toBeNull();finish({message:'完了'});await settle();expect(send.disabled).toBe(false);
});
it('Tabを閉じ込めEscapeで背面と起動元フォーカスを復元する',async()=>{
 document.body.innerHTML='<button id="launcher">送信</button>';const launcher=document.querySelector<HTMLButtonElement>('#launcher')!;launcher.focus();await openCloudUpload({rpc:vi.fn(),notice:vi.fn()},'s');const first=document.querySelector<HTMLButtonElement>('[data-close]')!,last=document.querySelector<HTMLButtonElement>('[data-cancel]')!;
 first.focus();first.dispatchEvent(new KeyboardEvent('keydown',{key:'Tab',shiftKey:true,bubbles:true,cancelable:true}));expect(document.activeElement).toBe(last);last.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));expect(document.querySelector('[role=dialog]')).toBeNull();expect(document.activeElement).toBe(launcher);expect(launcher.inert).toBeFalsy();
});
it('廃止したproviderを拒否する',async()=>{await expect(openCloudUpload({rpc:vi.fn(),notice:vi.fn()},'s','box' as any)).rejects.toThrow();});
