vi.mock('../../src/soundEffects',()=>({playCue:vi.fn(async()=>{})}));
import {afterEach,expect,it,vi} from 'vitest';

const api=vi.hoisted(()=>({invoke:vi.fn(),listen:vi.fn(async()=>()=>{})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:api.invoke}));
vi.mock('@tauri-apps/api/event',()=>({listen:api.listen}));

const lines=(text:string)=>text.split('\n').map((raw,line)=>({line,raw,text:raw,enabled:true,kind:0}));
const snapshot:any={project:{title:'source-ui',generation:1,script_revision:4,script_sources:[
 {id:'a',name:'a.md',kind:'markdown',confirmed_text:'旧A',draft_text:'旧A',lines:lines('旧A'),draft_lines:lines('旧A')},
 {id:'b',name:'b.md',kind:'markdown',confirmed_text:'旧B',draft_text:'旧B',lines:lines('旧B'),draft_lines:lines('旧B')},
],segments:[{id:'sa',source_id:'a',source_line:0,text:'旧A',takes:[],selected:null,kana:'よみA',confirmed:true,mappings:[],corrections:[],comparisons:[],trim_start:0,trim_end:0,gap:0,skip:false},{id:'sb',source_id:'b',source_line:0,text:'旧B',takes:[],selected:null,kana:'よみB',confirmed:true,mappings:[],corrections:[],comparisons:[],trim_start:0,trim_end:0,gap:0,skip:false}]},root:'source',busy:false,job:'',message:'',peak:0,elapsed:0};
async function settle(){for(let i=0;i<30;i++)await Promise.resolve();}
function click(label:string){const button=[...document.querySelectorAll<HTMLButtonElement>('button')].find(value=>value.textContent?.includes(label));expect(button).toBeTruthy();button!.click();}
afterEach(()=>{vi.clearAllTimers();vi.useRealTimers();});

it('source mode keeps per-file drafts, approvals, ordering, and committed baselines',async()=>{
 vi.resetModules();vi.useFakeTimers();document.body.innerHTML='<div id="app"></div>';Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});
 let imports=0;
 api.invoke.mockReset();api.listen.mockClear();api.invoke.mockImplementation(async(_command:string,{action,payload}:any)=>{
  if(action==='state')return structuredClone(snapshot);
  if(action==='markdown_preview')return {lines:lines(payload.text)};
  if(action==='markdown_import')return imports++===0?{files:[{name:'c.md',text:'C1'},{name:'d.md',text:'D1'}]}:null;
  if(action==='script_draft'){expect(payload.base_generation).toBe(snapshot.project.generation);expect(payload.base_revision).toBe(snapshot.project.script_revision);for(const draft of payload.sources){let source=snapshot.project.script_sources.find((item:any)=>item.id===draft.id);if(!source){source={id:draft.id,name:draft.name,kind:draft.kind,confirmed_text:'',draft_text:'',lines:[],draft_lines:[]};snapshot.project.script_sources.push(source);}source.draft_text=draft.text;source.draft_lines=draft.lines;source.approved_text=draft.approved_text;}snapshot.project.generation++;return {generation:snapshot.project.generation,script_revision:snapshot.project.script_revision};}
  if(action==='script_commit'){expect(payload.base_generation).toBe(snapshot.project.generation);expect(payload.base_revision).toBe(snapshot.project.script_revision);expect(payload.sources.every((source:any)=>source.approved_text===source.text)).toBe(true);expect(snapshot.project.script_sources.find((source:any)=>source.name==='d.md')?.draft_text).toBe('D3');for(const draft of payload.sources){const source=snapshot.project.script_sources.find((item:any)=>item.id===draft.id)!;source.confirmed_text=draft.text;source.draft_text=draft.text;source.lines=draft.lines;source.draft_lines=draft.lines;}snapshot.project.script_revision++;snapshot.project.generation++;return {generation:snapshot.project.generation,script_revision:snapshot.project.script_revision};}
  return action==='ui_attach'?{}:action==='devices'?[]:null;
 });
 await import('../../src/main');await settle();
 expect(document.querySelectorAll('.script-columns>section')).toHaveLength(4);
 expect(document.querySelectorAll('.script-source')).toHaveLength(2);
 expect([...document.querySelectorAll('.script-columns h3')].map(node=>node.firstChild?.textContent)).toEqual(['原稿を登録する','原稿を閲覧／編集する','差分を確認する','読み上げ対象を選択する']);
 const before=document.querySelector<HTMLElement>('.script-columns')!.style.gridTemplateColumns;
 document.querySelector<HTMLButtonElement>('[data-collapse-pane="0"]')!.click();
 expect(document.querySelector('.script-files.collapsed .script-source span')?.textContent).toBe('確認済み');
 document.querySelector<HTMLButtonElement>('[data-collapse-pane="1"]')!.click();
 expect(document.querySelectorAll('.collapsed')).toHaveLength(2);
 document.querySelector<HTMLButtonElement>('[data-collapse-pane="0"]')!.click();
 document.querySelector<HTMLButtonElement>('[data-collapse-pane="1"]')!.click();
 expect(document.querySelector<HTMLElement>('.script-columns')!.style.gridTemplateColumns).toBe(before);
 document.querySelector<HTMLButtonElement>('[data-action=script_import]')!.click();await settle();await vi.advanceTimersByTimeAsync(400);await settle();
 expect(document.querySelectorAll('.script-source')).toHaveLength(4);
 expect(document.querySelector('#markdown-source')?.textContent??document.querySelector<HTMLTextAreaElement>('#markdown-source')?.value).toBe('D1');
 const temporary=document.querySelector<HTMLTextAreaElement>('#markdown-source')!;temporary.value='取り消す編集';temporary.dispatchEvent(new Event('input'));await vi.advanceTimersByTimeAsync(400);await settle();
 document.querySelector<HTMLButtonElement>('[data-action=script_restore]')!.click();await vi.advanceTimersByTimeAsync(400);await settle();
 expect(document.querySelector<HTMLTextAreaElement>('#markdown-source')!.value).toBe('D1');
 const input=document.querySelector<HTMLTextAreaElement>('#markdown-source')!;input.value='D2';input.dispatchEvent(new Event('input'));await vi.advanceTimersByTimeAsync(150);await settle();
 expect(document.querySelectorAll('.diff-row button').length).toBeGreaterThan(0);
 document.querySelector<HTMLButtonElement>('[data-diff-mode=full]')!.click();expect(document.querySelector('.full-diff-table')).toBeTruthy();
 click('すべて承認');await vi.advanceTimersByTimeAsync(150);await settle();
 expect(document.querySelector<HTMLButtonElement>('[data-action=script_commit]')!.disabled).toBe(true);
 expect(document.querySelector('[data-action=script_approve_all]')?.getAttribute('aria-pressed')).toBe('true');
 expect(document.querySelector('.script-review-toolbar .script-review-all')).toBeTruthy();
 click('すべて解除');await settle();
 expect(document.querySelector('[data-action=script_approve_all]')?.getAttribute('aria-pressed')).toBe('false');expect(document.querySelector<HTMLButtonElement>('[data-action=script_commit]')!.disabled).toBe(true);
 click('すべて承認');await vi.advanceTimersByTimeAsync(400);await settle();
 await vi.advanceTimersByTimeAsync(700);await settle();
 expect(document.querySelector<HTMLTextAreaElement>('#markdown-source')!.value).toBe('D2');
 document.querySelector<HTMLButtonElement>('[data-action=script_empty]')!.click();await settle();document.querySelector<HTMLButtonElement>('[data-action=script_empty]')!.click();await settle();
 expect([...document.querySelectorAll('.script-source b')].map(node=>node.textContent)).toContain('空の原稿 (2).md');
 click('c.md');await settle();click('すべて承認');await vi.advanceTimersByTimeAsync(400);await settle();
 click('d.md');await settle();expect(snapshot.project.script_sources.find((source:any)=>source.id==='b').confirmed_text).toBe('旧B');
 const latest=document.querySelector<HTMLTextAreaElement>('#markdown-source')!;latest.value='D3';latest.dispatchEvent(new Event('input'));await vi.advanceTimersByTimeAsync(200);await settle();click('すべて承認');await settle();
 click('台本確定');await settle();
 expect(api.invoke.mock.calls.some(call=>call[1]?.action==='script_commit')).toBe(true);
});
