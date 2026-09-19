import {beforeEach,describe,it,expect,vi} from 'vitest';
import {openTakeJoin,bindTakeJoin,type TakeJoinContext} from '../../src/takeJoin';
const takes=[1,2].map(n=>({id:`t${n}`,file:`${n}.wav`,hash:`hash${n}`,status:'ready',duration:n,peak:.5,warning:''}));
function context(overrides:Partial<TakeJoinContext>={}):TakeJoinContext{return {projectId:'p',segment:{id:'s',takes},disabled:false,rpc:vi.fn(async()=>({file:'joined.wav',hash:'joined'})) as TakeJoinContext['rpc'],play:vi.fn(async()=>{}),stop:vi.fn(),prepare:vi.fn(async()=>true),onSaved:vi.fn(async()=>{}),notice:vi.fn(),...overrides};}
const button=(name:string)=>document.querySelector<HTMLButtonElement>(`[data-join-${name}]`)!;
describe('前後テイク結合',()=>{
 beforeEach(()=>{document.body.innerHTML='<div class="take-column"><div class="section-head"></div></div>';HTMLDialogElement.prototype.showModal=function(){this.open=true;};HTMLDialogElement.prototype.close=function(){this.open=false;};});
 it('2件未満と録音中は結合を開始しない',()=>{bindTakeJoin(context({segment:{id:'s',takes:[takes[0]]}}));expect(document.querySelector<HTMLButtonElement>('[data-take-join]')!.disabled).toBe(true);});
 it('前後選択・入替・試聴後の保存と採用しない通知',async()=>{
  const ctx=context();openTakeJoin(ctx);expect(button('save').disabled).toBe(true);button('swap').click();button('preview').click();await vi.waitFor(()=>expect(ctx.play).toHaveBeenCalledWith('joined.wav','joined'));
  expect(ctx.rpc).toHaveBeenCalledWith('preview_take_join',{project_id:'p',segment:'s',before:'t2',after:'t1',before_hash:'hash2',after_hash:'hash1'});expect(button('save').disabled).toBe(false);button('save').click();await vi.waitFor(()=>expect(ctx.onSaved).toHaveBeenCalledOnce());expect(document.querySelector('dialog')).toBeNull();expect(ctx.rpc).toHaveBeenLastCalledWith('join_takes',expect.objectContaining({before:'t2',after:'t1'}));
 });
 it('同じテイクを拒否し、変更後は再試聴する',async()=>{const ctx=context();openTakeJoin(ctx);button('preview').click();await vi.waitFor(()=>expect(button('save').disabled).toBe(false));button('swap').click();expect(button('save').disabled).toBe(true);const after=document.querySelector<HTMLSelectElement>('[data-join-after]')!;after.value='t2';after.dispatchEvent(new Event('change'));expect(button('preview').disabled).toBe(true);});
 it('試聴処理中は二重操作・終了を抑止し、失敗後に再試行できる',async()=>{let reject!:(e:Error)=>void;const ctx=context({rpc:vi.fn(()=>new Promise((_,fail)=>{reject=fail;})) as TakeJoinContext['rpc']});openTakeJoin(ctx);button('preview').click();expect(button('preview').disabled).toBe(true);button('preview').click();const dialog=document.querySelector('dialog')!;dialog.dispatchEvent(new Event('cancel',{cancelable:true}));expect(dialog.isConnected).toBe(true);expect(ctx.rpc).toHaveBeenCalledOnce();reject(new Error('試聴エラー'));await vi.waitFor(()=>expect(button('preview').disabled).toBe(false));expect(button('save').disabled).toBe(true);expect(dialog.textContent).toContain('試聴エラー');button('cancel').click();expect(dialog.isConnected).toBe(false);});
});
