import {beforeEach,expect,it,vi} from 'vitest';
import {bindLegacySources} from '../../src/legacySources';

let root:HTMLElement;
beforeEach(()=>{document.body.innerHTML='<div id="root"></div>';root=document.getElementById('root')!;});
const source={id:'legacy',name:'既存台本',confirmed_text:'甲\n乙',draft_text:'甲\n乙'};
const segments=[{id:'s1',source_id:'legacy',source_text:'甲',text:'甲'},{id:'s2',source_id:'legacy',source_text:'乙',text:'乙'}];
function context(rpc:any){return {source,segments,rpc,flush:vi.fn(async()=>{}),base:()=>({base_revision:3,base_generation:7}),refresh:vi.fn(async()=>{}),notice:vi.fn()};}

it('取消では対応登録を送らない',async()=>{const rpc=vi.fn(async(action:string)=>action==='markdown_import'?{files:[{name:'a.md',text:'甲\n乙',lines:[{line:0,text:'甲',enabled:true},{line:1,text:'乙',enabled:true}]}]}:null),c=context(rpc);bindLegacySources(root,c);root.querySelector('button')!.click();await Promise.resolve();await Promise.resolve();document.querySelector<HTMLButtonElement>('[data-cancel]')!.click();expect(rpc).not.toHaveBeenCalledWith('script_rebind_legacy',expect.anything());});
it('確認後に世代付きの対応を登録し、競合エラーを表示する',async()=>{const rpc=vi.fn(async(action:string)=>{if(action==='markdown_import')return {files:[{name:'a.md',text:'甲\n乙',lines:[{line:0,text:'甲',enabled:true},{line:1,text:'乙',enabled:true}]}]};if(action==='script_rebind_legacy')throw new Error('generation conflict');return null;}),c=context(rpc);bindLegacySources(root,c);root.querySelector('button')!.click();await Promise.resolve();await Promise.resolve();document.querySelector<HTMLButtonElement>('[data-confirm]')!.click();await Promise.resolve();await Promise.resolve();expect(rpc).toHaveBeenCalledWith('script_rebind_legacy',expect.objectContaining({legacy_source_id:'legacy',base_revision:3,base_generation:7}));expect(document.querySelector('[data-error]')?.textContent).toContain('generation conflict');});
