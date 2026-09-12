import {afterEach,expect,it,vi} from 'vitest';
const api=vi.hoisted(()=>({invoke:vi.fn(),listen:vi.fn(async()=>()=>{})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:api.invoke}));
vi.mock('@tauri-apps/api/event',()=>({listen:api.listen}));
vi.mock('../../src/soundEffects',()=>({playCue:vi.fn(async()=>{})}));
afterEach(()=>{vi.clearAllTimers();vi.useRealTimers();});
it('opens one dropped project through the same open action and removes welcome explanations',async()=>{
 vi.resetModules();vi.useFakeTimers();document.body.innerHTML='<div id="app"></div>';
 Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});
 api.invoke.mockImplementation(async(_command,{action})=>action==='state'?{project:null,root:null,busy:false,job:'',message:'',peak:0,elapsed:0}:action==='devices'?[]:{});
 await import('../../src/main');for(let i=0;i<30;i++)await Promise.resolve();
 expect(document.querySelectorAll('.flow-graphic li')).toHaveLength(5);
 expect(document.querySelectorAll('.welcome-options p')).toHaveLength(0);
 const drop=api.listen.mock.calls.find(([event])=>event==='tauri://drag-drop')![1] as any;
 await drop({payload:{paths:['C:/試験/私の台本.SCRIPTSYNC']}});for(let i=0;i<30;i++)await Promise.resolve();
 expect(api.invoke).toHaveBeenCalledWith('command',{action:'open',payload:{path:'C:/試験/私の台本.SCRIPTSYNC'}});
 api.invoke.mockClear();await drop({payload:{paths:['wrong.md']}});
 expect(api.invoke.mock.calls.some(([,call])=>call.action==='open')).toBe(false);
});
