import {afterEach,expect,it,vi} from 'vitest';
const api=vi.hoisted(()=>({invoke:vi.fn(),listen:vi.fn(async()=>()=>{})}));
vi.mock('@tauri-apps/api/core',()=>({invoke:api.invoke}));
vi.mock('@tauri-apps/api/event',()=>({listen:api.listen}));
vi.mock('../../src/soundEffects',()=>({playCue:vi.fn(async()=>{})}));
vi.mock('../../src/nativeAudio',()=>({installNativeAudio:vi.fn(),registerNativeAudio:vi.fn()}));
const settle=async()=>{for(let i=0;i<100;i++)await Promise.resolve();};
afterEach(()=>{vi.clearAllTimers();vi.useRealTimers();vi.restoreAllMocks();});
it('確定後は台本に残り、再編集で出力を無効化し、Codexは別操作で原稿へ反映する',async()=>{
 vi.resetModules();vi.useFakeTimers();vi.spyOn(HTMLMediaElement.prototype,'pause').mockImplementation(()=>{});document.body.innerHTML='<div id="app"></div>';
 Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{},configurable:true});
 const lines=[{line:0,text:'原稿',raw:'原稿',enabled:true,kind:0}];
 const project:any={id:'p',title:'試験',generation:1,script_revision:1,script_sources:[{id:'a',name:'a.md',confirmed_text:'原稿',draft_text:'原稿',lines,draft_lines:lines}],segments:[{id:'s',source_id:'a',source_line:0,text:'原稿',source_text:'原稿',takes:[{id:'t',file:'a.wav',hash:'h',status:'ready',duration:1,warning:'',peak:.2}],selected:'t',kana:'',confirmed:false,mappings:[],corrections:[],comparisons:[],trim_start:0,trim_end:0,gap:0,skip:false,transcriptions:[{id:'asr',take_id:'t',file:'a.wav',audio_hash:'h',status:'succeeded',raw:'発言の誤字',kana:'はつげん'}]}],archived_segments:[],pending_source_deletions:[]};
 let resolveImprove:(result:{text:string})=>void=()=>{};
 api.invoke.mockReset();api.invoke.mockImplementation(async(_command:string,{action,payload}:any)=>{
  if(action==='state')return {project:structuredClone(project),root:'p',busy:false,job:'',message:'',peak:0,elapsed:0,script_export_ready_source_ids:['a']};
  if(action==='markdown_preview')return {lines};
  if(action==='text_improve')return new Promise(resolve=>{resolveImprove=resolve;});
  if(action==='recording_draft'){project.segments[0].recording_draft=payload.text;project.generation++;}
  if(action==='script_commit'){project.generation++;project.script_revision++;return {generation:project.generation,script_revision:project.script_revision};}
  return action==='devices'?[]:{};
 });
 await import('../../src/main');await settle();
 const button=(action:string)=>document.querySelector<HTMLButtonElement>(`[data-action=${action}]`)!;
 expect([...document.querySelectorAll('.script-source-actions>[data-action]')].slice(0,5).map(b=>b.getAttribute('data-action'))).toEqual(['script_empty','script_import','exchange_import','script_rename','script_delete']);
 expect(document.querySelector('#script-send-target')).toBeNull();
 expect(document.querySelectorAll('[data-script-info]')).toHaveLength(4);
 expect(button('exchange_export').disabled).toBe(false);
 button('script_commit').click();await settle();expect(document.querySelector('.script-workspace')).not.toBeNull();
 const input=document.querySelector<HTMLTextAreaElement>('#markdown-source')!;input.value='変更';input.dispatchEvent(new Event('input'));
 expect(button('exchange_export').disabled).toBe(true);expect(button('script_send').disabled).toBe(true);
 // Cancel draft through existing restore, then move to recording.
 button('script_restore').click();await settle();document.querySelector<HTMLButtonElement>('[data-tab=recording]')!.click();await settle();
 expect(document.querySelector('.recording-script>.record-controls')).not.toBeNull();
 expect(document.querySelector('.recording-original #text')).not.toBeNull();expect(document.querySelector('.recording-original [data-action=split]')).not.toBeNull();
 const startKey=new KeyboardEvent('keydown',{key:'F5',bubbles:true,cancelable:true});document.dispatchEvent(startKey);await settle();expect(startKey.defaultPrevented).toBe(true);expect(api.invoke).toHaveBeenCalledWith('continuous_hotkeys',{enabled:true});document.dispatchEvent(new KeyboardEvent('keydown',{key:'F6',bubbles:true,cancelable:true}));await settle();expect(api.invoke).toHaveBeenCalledWith('continuous_hotkeys',{enabled:false});
 expect(document.querySelector('[data-action=recording_asr]')).toBeNull();expect(document.querySelector('.take-trim-fields')?.textContent).toContain('発言の誤字');expect(document.querySelector('.recording-original [data-action=improve_text]')).not.toBeNull();
 expect(api.invoke.mock.calls.some(call=>(call[1] as any)?.action==='text_improve')).toBe(false);
 button('improve_text').click();await settle();expect(api.invoke).toHaveBeenCalledWith('command',{action:'text_improve',payload:{text:'原稿'}});resolveImprove({text:'発言の修正'});await settle();
 expect(project.segments[0].recording_draft).toBe('発言の修正');expect(project.segments[0].transcriptions[0].raw).toBe('発言の誤字');
 button('improve_text').click();await settle();const original=document.querySelector<HTMLTextAreaElement>('#text')!;original.value='ユーザーの編集中';original.dispatchEvent(new Event('input'));
 resolveImprove({text:'上書き禁止'});await settle();expect(original.value).toBe('ユーザーの編集中');
 project.segments[0].transcriptions=[];project.generation++;await vi.advanceTimersByTimeAsync(750);await settle();
 button('recording_asr').click();await settle();
 expect(api.invoke).toHaveBeenCalledWith('command',{action:'recording_asr',payload:{segment:'s',take:'t'}});
},30000);
