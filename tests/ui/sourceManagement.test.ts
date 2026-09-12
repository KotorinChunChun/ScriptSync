import {afterEach,expect,it,vi} from 'vitest';
import {importedSource,makeDraft,sourceMapping,sourcePayload,updateParsedLines} from '../../src/scriptSources';
const api=vi.hoisted(()=>({invoke:vi.fn(),listen:vi.fn(async()=>()=>{})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:api.invoke}));
vi.mock('@tauri-apps/api/event',()=>({listen:api.listen}));
vi.mock('../../src/soundEffects',()=>({playCue:vi.fn(async()=>{})}));
const lines=(text:string)=>text.split('\n').map((text,line)=>({line,text,raw:text,enabled:true,kind:0}));
afterEach(()=>{vi.clearAllTimers();vi.useRealTimers();});
it('対応候補は未使用の削除行と過去アーカイブだけで、新規選択は再解析後も維持する',()=>{
 const draft=makeDraft({id:'a',name:'台本.md',confirmed_text:'一致\n削除',draft_text:'一致\n追加',lines:lines('一致\n削除'),draft_lines:lines('一致\n追加')});
 const active=[{id:'same',source_id:'a',source_line:0,text:'一致'},{id:'removed',source_id:'a',source_line:1,text:'削除'}];
 const archived=[{id:'old',source_id:'a',text:'以前'},{id:'foreign',source_id:'b',text:'他原稿'}];
 expect(sourceMapping(draft,draft.lines[0],active,archived).matched?.id).toBe('same');
 expect(sourceMapping(draft,draft.lines[1],active,archived).candidates.map(s=>s.id)).toEqual(['removed','old']);
 draft.lines[1].previous_id='old';draft.lines[1].force_new=false;
 expect(sourcePayload(draft).lines[1].keep).toBe(true);
 draft.lines[1].previous_id=undefined;draft.lines[1].force_new=true;draft.lines[1].previous_line=null;
 updateParsedLines(draft,lines(draft.text));
 expect(sourcePayload(draft).lines[1]).toMatchObject({previous_line:null,force_new:true,keep:false});
});
it('F2の名前入力、同名取り込みの差分、削除予定の取消を保存する',async()=>{
 vi.resetModules();vi.useFakeTimers();document.body.innerHTML='<div id="app"></div>';
 Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});
 const target={id:'a',name:'台本.md',kind:'markdown',confirmed_text:'既存',draft_text:'既存',lines:lines('既存'),draft_lines:lines('既存')};
 const incoming={id:'b',name:'台本_修正.md',kind:'markdown',confirmed_text:'',draft_text:'修正版',lines:[],draft_lines:lines('修正版')};
 const project:any={id:'p',title:'rename',generation:1,script_revision:1,script_sources:[target,incoming],segments:[],archived_segments:[],pending_source_deletions:[]};
 const settle=async()=>{for(let i=0;i<40;i++)await Promise.resolve();};
 api.invoke.mockReset();api.invoke.mockImplementation(async(_command,{action,payload}:any)=>{
  if(action==='state')return {project:structuredClone(project),root:'rename',busy:false,job:'',peak:0,elapsed:0};
  if(action==='markdown_preview')return {lines:lines(payload.text)};
  if(action==='script_draft'){
   project.script_sources=payload.sources.map((s:any)=>({...project.script_sources.find((old:any)=>old.id===s.id),name:s.name,draft_text:s.text,draft_lines:s.lines,approved_text:s.approved_text}));
   project.pending_source_deletions=payload.deleted_source_ids;return {generation:++project.generation,script_revision:project.script_revision};
  }
  return action==='devices'?[]:{};
 });
 await import('../../src/main');await settle();
 document.querySelector<HTMLButtonElement>('[data-source-id=b]')!.click();
 document.dispatchEvent(new KeyboardEvent('keydown',{key:'F2',bubbles:true}));
 const input=document.querySelector<HTMLInputElement>('#script-source-name')!;
 expect(document.activeElement).toBe(input);
 input.value='台本.md';input.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',isComposing:true,bubbles:true}));
 expect(document.querySelector('#script-source-name')).toBeTruthy();
 input.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));await settle();await vi.advanceTimersByTimeAsync(400);await settle();
 expect(project.script_sources.find((s:any)=>s.id==='a')).toMatchObject({confirmed_text:'既存',draft_text:'修正版'});
 expect(project.pending_source_deletions).toEqual(['b']);
 expect(document.querySelectorAll('.script-source')).toHaveLength(1);
 expect(document.querySelector('.diff-text')?.textContent).toBeTruthy();
 const direct=importedSource(makeDraft(target),{name:'台本.md',text:'修正版'},'a');
 expect(project.script_sources.find((s:any)=>s.id==='a').draft_text).toBe(direct.text);
 document.querySelector<HTMLButtonElement>('[data-action=script_delete]')!.click();await vi.advanceTimersByTimeAsync(400);await settle();
 expect(project.pending_source_deletions).toEqual(['b','a']);
 expect(document.querySelectorAll('.script-source')).toHaveLength(0);
 document.querySelector<HTMLButtonElement>('[data-source-restore=a]')!.click();await vi.advanceTimersByTimeAsync(400);await settle();
 expect(project.pending_source_deletions).toEqual(['b']);
});
