vi.mock('../../src/soundEffects',()=>({playCue:vi.fn(async()=>{})}));
import {afterEach,expect,it,vi} from 'vitest';
const api=vi.hoisted(()=>({invoke:vi.fn(),listen:vi.fn(async()=>()=>{})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:api.invoke}));
vi.mock('@tauri-apps/api/event',()=>({listen:api.listen}));

it('空のsourceでも録音行を挿入して直ちに録音できる',async()=>{
 vi.resetModules();vi.useFakeTimers();document.body.innerHTML='<div id="app"></div>';Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});
 const snapshot:any={project:{id:'project-1',title:'empty',generation:9,script_revision:4,script_sources:[{id:'source-1',name:'empty.md',kind:'markdown',confirmed_text:'',draft_text:'',lines:[],draft_lines:[]}],segments:[]},root:'empty',busy:false,job:'',message:'',peak:0,elapsed:0};
 api.invoke.mockReset();api.listen.mockClear();api.invoke.mockImplementation(async(_command:string,{action,payload}:any)=>{
  if(action==='state')return structuredClone(snapshot);
  if(action==='ui_attach')return {};if(action==='devices')return [{endpoint_id:'mic',friendly_name:'mic',is_default:true}];
  if(action==='script_insert_empty'){expect(payload).toMatchObject({project_id:'project-1',source_id:'source-1',base_revision:4,base_generation:9});snapshot.project.segments=[{id:'empty-segment',source_id:'source-1',text:'',takes:[],selected:null,kana:'',confirmed:false,mappings:[],corrections:[],comparisons:[],trim_start:0,trim_end:0,gap:0,skip:false}];snapshot.project.generation++;return {generation:snapshot.project.generation,script_revision:4};}
  return null;
 });
 await import('../../src/main');for(let i=0;i<30;i++)await Promise.resolve();
 [...document.querySelectorAll<HTMLButtonElement>('[data-tab]')].find(button=>button.textContent?.includes('録音'))!.click();for(let i=0;i<30;i++)await Promise.resolve();
 const insert=[...document.querySelectorAll<HTMLButtonElement>('button')].find(button=>button.textContent==='空の台本行を挿入');expect(insert).toBeTruthy();insert!.click();for(let i=0;i<40;i++)await Promise.resolve();
 expect(api.invoke.mock.calls.some(call=>call[1]?.action==='script_insert_empty')).toBe(true);
 expect(document.querySelector<HTMLTextAreaElement>('#text, .recording-original textarea[readonly]')?.value).toBe('');
 expect(document.querySelector<HTMLButtonElement>('[data-action=record]')).toBeTruthy();
});
afterEach(()=>{vi.clearAllTimers();vi.useRealTimers();});
