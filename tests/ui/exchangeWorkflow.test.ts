import {afterEach,expect,it,vi} from 'vitest';
const api=vi.hoisted(()=>({invoke:vi.fn(),listen:vi.fn(async()=>()=>{})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:api.invoke}));
vi.mock('@tauri-apps/api/event',()=>({listen:api.listen}));
vi.mock('../../src/soundEffects',()=>({playCue:vi.fn(async()=>{})}));
vi.mock('../../src/nativeAudio',()=>({installNativeAudio:vi.fn(),registerNativeAudio:vi.fn()}));
const lines=(text:string)=>text.split('\n').map((text,line)=>({line,text,raw:text,enabled:true,kind:0}));
async function settle(){for(let i=0;i<80;i++)await Promise.resolve();}
afterEach(()=>{vi.clearAllTimers();vi.useRealTimers();vi.restoreAllMocks();});

it('複数原稿の先頭へWebZIPを取り込むと選択と録音対応先を先頭へ保持する',async()=>{
 vi.resetModules();vi.useFakeTimers();document.body.innerHTML='<div id="app"></div>';
 Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});
 Object.defineProperty(HTMLDialogElement.prototype,'showModal',{value:function(this:HTMLDialogElement){this.open=true;},configurable:true});
 const source=(id:string,text:string)=>({id,name:`${id}.md`,kind:'markdown',confirmed_text:text,draft_text:text,lines:lines(text),draft_lines:lines(text)});
 const segment=(id:string,source_id:string,text:string)=>({id,source_id,source_line:0,text,source_text:text,takes:[],selected:null,kana:'',confirmed:false,mappings:[],corrections:[],comparisons:[],trim_start:0,trim_end:0,gap:0,skip:false});
 const project:any={id:'exchange-project',title:'交換',generation:1,script_revision:1,script_sources:[source('a','旧A'),source('b','Bの本文')],segments:[segment('sa','a','旧A'),segment('sb','b','Bの本文')],archived_segments:[],pending_source_deletions:[]};
 api.invoke.mockReset();api.invoke.mockImplementation(async(_command:string,{action,payload}:any)=>{
  if(action==='state')return {project:structuredClone(project),root:'exchange-project',busy:false,job:'',message:'',peak:0,elapsed:0};
  if(action==='markdown_preview')return {lines:lines(payload.text)};
  if(action==='exchange_inspect')return {token:'web-token',manifest:{source:'修正A',sourceFileName:'Web別名.md',lines:[{order:1,text:'修正A',selected:'web-take',takes:[{id:'web-take'}]}]}};
  if(action==='script_commit'){
   expect(payload.sources.find((s:any)=>s.id==='a').text).toBe('修正A');
   expect(payload.sources.find((s:any)=>s.id==='b').text).toBe('Bの本文');
   for(const draft of payload.sources){const source=project.script_sources.find((s:any)=>s.id===draft.id);Object.assign(source,{confirmed_text:draft.text,draft_text:draft.text,lines:draft.lines,draft_lines:draft.lines});}
   Object.assign(project.segments[0],{text:'修正A',source_text:'修正A'});project.generation++;project.script_revision++;
   return {generation:project.generation,script_revision:project.script_revision};
  }
  if(action==='exchange_merge')return {added:1,duplicates:0};
  return action==='devices'?[]:{};
 });
 await import('../../src/main');await settle();
 expect(document.querySelector('.script-source.selected')?.getAttribute('data-source-id')).toBe('a');
 document.querySelector<HTMLButtonElement>('[data-action=exchange_import]')!.click();await settle();
 document.querySelector<HTMLButtonElement>('dialog [data-open]')!.click();await settle();
 expect(document.querySelector('.script-source.selected')?.getAttribute('data-source-id')).toBe('a');
 expect(document.querySelector<HTMLTextAreaElement>('#markdown-source')!.value).toBe('修正A');
 document.querySelector<HTMLButtonElement>('[data-action=script_approve_all]')!.click();await settle();
 expect(document.querySelector<HTMLButtonElement>('[data-action=script_commit]')!.disabled).toBe(false);
 document.querySelector<HTMLButtonElement>('[data-action=script_commit]')!.click();await settle();
 const mapping=document.querySelector<HTMLSelectElement>('.exchange-map select');expect(mapping).not.toBeNull();
 expect([...mapping!.options].map(option=>option.value)).toEqual(['','sa']);
 expect(mapping!.value).toBe('sa');
 document.querySelector<HTMLButtonElement>('.exchange-map [data-merge]')!.click();await settle();
 expect(api.invoke).toHaveBeenCalledWith('command',{action:'exchange_merge',payload:{token:'web-token',script_revision:2,mappings:[{order:1,segment_id:'sa'}]}});
},20000);
