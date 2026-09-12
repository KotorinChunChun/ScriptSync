import {afterEach,expect,it,vi} from 'vitest';
const api=vi.hoisted(()=>({invoke:vi.fn(),listen:vi.fn(async()=>()=>{})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:api.invoke}));vi.mock('@tauri-apps/api/event',()=>({listen:api.listen}));vi.mock('../../src/soundEffects',()=>({playCue:vi.fn(async()=>{})}));
afterEach(()=>{vi.clearAllTimers();vi.useRealTimers();vi.restoreAllMocks();});
it('左上のプロジェクト名をEnterで確定し、Escapeでは変更しない',async()=>{
 vi.spyOn(HTMLMediaElement.prototype,'pause').mockImplementation(()=>{});vi.resetModules();vi.useFakeTimers();document.body.innerHTML='<div id="app"></div>';Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});
 const snapshot:any={project:{id:'project',title:'旧名',generation:1,script_revision:1,script_sources:[],segments:[]},root:'old',busy:false,job:'',message:'',peak:0,elapsed:0};
 api.invoke.mockImplementation(async(_,{action,payload})=>{if(action==='state')return structuredClone(snapshot);if(action==='devices')return [];if(action==='project_rename'){expect(payload).toEqual({project_id:'project',base_generation:1,name:'新しい名前'});snapshot.project.title=payload.name;snapshot.project.generation++;snapshot.root='new';}return {};});
 const settle=async()=>{for(let i=0;i<40;i++)await Promise.resolve();};await import('../../src/main');await settle();
 document.querySelector<HTMLButtonElement>('#project-name-button')!.click();let input=document.querySelector<HTMLInputElement>('#project-name')!;
 input.value='取消';input.dispatchEvent(new Event('input'));input.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));
 expect(document.querySelector('#project-name-button')?.textContent).toBe('旧名');
 document.querySelector<HTMLButtonElement>('#project-name-button')!.click();input=document.querySelector<HTMLInputElement>('#project-name')!;input.value='新しい名前';input.dispatchEvent(new Event('input'));input.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));await settle();
 expect(document.querySelector('#project-name-button')?.textContent).toBe('新しい名前');
 expect(api.invoke.mock.calls.filter(([,call])=>call.action==='project_rename')).toHaveLength(1);
});
