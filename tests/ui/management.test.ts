import {afterEach,expect,it,vi} from 'vitest';
import {installManagement} from '../../src/management';
afterEach(()=>document.body.replaceChildren());
async function fixture(blocked=false,accepted=false){
 const handlers=new Map<string,(event:{payload:unknown})=>void>();
 const invoke=vi.fn(async()=>accepted) as any,freeze=vi.fn();
 await installManagement({listen:async(name,handler)=>{handlers.set(name,handler);return()=>{};},invoke,freeze,blocked:()=>blocked});
 return {invoke,freeze,request:()=>handlers.get('management-stop-requested')!({payload:'request-1'}),cancel:()=>handlers.get('management-stop-cancelled')!({payload:null})};
}
const settle=async()=>{for(let i=0;i<5;i++)await Promise.resolve();};
it('未保存入力や処理があれば停止を拒否して画面を操作可能なまま保つ',async()=>{
 const f=await fixture(true);f.request();await settle();expect(f.invoke).toHaveBeenCalledWith('management_stop_reply',{id:'request-1',allowed:false});expect(f.freeze).not.toHaveBeenCalled();
});
it('設定・編集・確認ダイアログが開いていれば停止を拒否する',async()=>{
 document.body.innerHTML='<div role="dialog"><textarea>未保存</textarea></div>';
 const f=await fixture();f.request();await settle();expect(f.invoke.mock.calls[0][1].allowed).toBe(false);
});
it('終了受理前に入力を止め、重複停止要求を処理しない',async()=>{
 const f=await fixture(false,true);f.request();f.request();await settle();expect(f.freeze).toHaveBeenCalledExactlyOnceWith(true);expect(f.invoke).toHaveBeenCalledTimes(1);
});
it('バックエンドの拒否や応答失敗では入力を復旧する',async()=>{
 const f=await fixture();f.request();await settle();expect(f.freeze.mock.calls.map(c=>c[0])).toEqual([true,false]);
 f.invoke.mockRejectedValueOnce(Error('応答なし'));f.request();await settle();expect(f.freeze.mock.calls.map(c=>c[0])).toEqual([true,false,true,false]);
});
it('通信期限切れの撤回通知で画面を復旧する',async()=>{
 const f=await fixture(false,true);f.request();await settle();f.cancel();expect(f.freeze).toHaveBeenLastCalledWith(false);
});
it('停止を準備していない撤回通知ではダイアログの入力制限に触れない',async()=>{
 const f=await fixture();f.cancel();expect(f.freeze).not.toHaveBeenCalled();
});
