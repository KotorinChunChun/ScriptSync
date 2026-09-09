import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";

const api=vi.hoisted(()=>({invoke:vi.fn(),listen:vi.fn(async(..._args:any[])=>()=>{})}));
vi.mock("@tauri-apps/api/core",()=>({invoke:api.invoke}));
vi.mock("@tauri-apps/api/event",()=>({listen:api.listen}));
let snapshot:any;
async function settle(){for(let i=0;i<40;i++)await Promise.resolve();}
async function tab(name:string){const b=[...document.querySelectorAll<HTMLButtonElement>("[data-tab]")].find(x=>x.textContent?.includes(name));expect(b).toBeTruthy();b!.click();await settle();}
function action(name:string){const b=[...document.querySelectorAll<HTMLButtonElement>("[data-action]")].find(x=>x.textContent===name||x.textContent?.includes(name));expect(b).toBeTruthy();b!.click();}
beforeEach(()=>{api.listen.mockClear();vi.spyOn(HTMLMediaElement.prototype,"pause").mockImplementation(()=>{});vi.resetModules();vi.useFakeTimers();document.body.innerHTML='<div id="app"></div>';Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{},configurable:true});snapshot={project:{title:"試験",generation:1,source_markdown:"# 部\n本文",script_lines:[{line:1,raw:"# 部",text:"部",enabled:true,kind:1,hierarchy:[1,0,0,0]},{line:2,raw:"本文",text:"本文",enabled:true,kind:0,hierarchy:[1,0,0,0]}],segments:[{id:"s1",text:"市場へ行きます。",source_text:"市場へ行きます。",kind:0,hierarchy:[1,0,0,0],revision:1,takes:[{id:"t1",status:"ready",duration:1.4,file:"takes/a.wav",hash:"hash",warning:""}],selected:"t1",kana:"いちばへいきます。",confirmed:true,mappings:[],corrections:[],comparisons:[{id:"r1",order:["v1","v2","v3"],revealed:false,variants:["A","B","C"].map((label,i)=>({id:`v${i+1}`,label,status:"succeeded",file:`${label}.wav`,hash:label,error:"",text:label,transcript:"",rating:null}))}],trim_start:0,trim_end:0,gap:0,skip:false}]},root:"test",busy:false,job:"",message:"保存済み",peak:.1,elapsed:0};api.invoke.mockReset();api.invoke.mockImplementation(async(_cmd:string,{action,payload}:any)=>{if(action==="state")return structuredClone(snapshot);if(action==="devices")return [{endpoint_id:"mic1",friendly_name:"試験マイク",is_default:true}];if(action==="waveform")return {peaks:Array.from({length:800},(_,i)=>(i%50)/50)};if(action==="markdown_preview")return {lines:snapshot.project.script_lines};if(action==="markdown_import")return {text:"# 部\n本文",lines:snapshot.project.script_lines};return null;});});
afterEach(()=>{vi.clearAllTimers();vi.useRealTimers();});
describe("再設計した作業フロー",()=>{
 it("起動画面は新規・開く・5工程だけを表示する",async()=>{snapshot.project=null;await import("../../src/main");await settle();expect(document.body.textContent).toContain("新規プロジェクト");expect(document.body.textContent).toContain("結合・出力");expect(document.querySelector("aside")).toBeNull();});
 it("台本が初期工程である",async()=>{await import("../../src/main");await settle();expect(document.querySelector("[data-tab=script]")?.getAttribute("aria-selected")).toBe("true");});
 it("台本タブにMarkdown原稿を表示する",async()=>{await import("../../src/main");await settle();expect(document.querySelector<HTMLTextAreaElement>("#markdown-source")?.value).toContain("# 部");});
 it("原稿行をオンオフできる",async()=>{await import("../../src/main");await settle();const row=document.querySelector<HTMLButtonElement>("[data-script-line]")!;row.click();expect(document.querySelector(".source-line.disabled")).toBeTruthy();});
 it("Markdown確認はRPCへ送る",async()=>{await import("../../src/main");await settle();action("行を確認");await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="markdown_preview")).toBe(true);});
 it("Markdown確定は行情報を送る",async()=>{await import("../../src/main");await settle();action("台本を確定");await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="markdown_commit")).toBe(true);});
 it("左ペインはフィルターと台本ツリーを表示する",async()=>{await import("../../src/main");await settle();expect(document.querySelector("#filter")).toBeTruthy();expect(document.querySelector("[data-segment=s1]")?.textContent).toContain("市場");});
 it("録音工程は元原稿と録音用原稿を並べる",async()=>{await import("../../src/main");await settle();await tab("録音");expect(document.querySelector("#text")).toBeTruthy();expect(document.querySelector(".take-column")).toBeTruthy();});
 it("録音開始は既定Endpointを送る",async()=>{await import("../../src/main");await settle();await tab("録音");action("録音開始");await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="record")).toBe(true);});
 it("録音前は停止保存を無効にする",async()=>{await import("../../src/main");await settle();await tab("録音");expect([...document.querySelectorAll<HTMLButtonElement>("button")].find(x=>x.textContent?.includes("停止して保存"))?.disabled).toBe(true);});
 it("先頭では前へを無効にする",async()=>{await import("../../src/main");await settle();await tab("録音");expect([...document.querySelectorAll<HTMLButtonElement>("button")].find(x=>x.textContent?.includes("前へ"))?.disabled).toBe(true);});
 it("テイクは試聴・採用・削除を表示する",async()=>{await import("../../src/main");await settle();await tab("録音");expect(document.querySelector(".take-actions")?.textContent).toContain("削除");});
 it("テイク削除は中央確認を表示する",async()=>{await import("../../src/main");await settle();await tab("録音");action("削除");expect(document.querySelector("[role=dialog]")).toBeTruthy();});
 it("削除確認のキャンセルはRPCを呼ばない",async()=>{await import("../../src/main");await settle();await tab("録音");action("削除");document.querySelector<HTMLButtonElement>("#delete-cancel")?.click();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="delete_take")).toBe(false);});
 it("削除確認の確定だけが削除RPCを呼ぶ",async()=>{await import("../../src/main");await settle();await tab("録音");action("削除");document.querySelector<HTMLButtonElement>("#delete-confirm")?.click();await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="delete_take")).toBe(true);});
 it("テイクメモはblurで保存する",async()=>{await import("../../src/main");await settle();await tab("録音");const memo=document.querySelector<HTMLTextAreaElement>("[data-take-memo]")!;memo.value="語尾";memo.dispatchEvent(new FocusEvent("blur"));await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="take_note")).toBe(true);});
 it("採用後は次の発話へ進む",async()=>{snapshot.project.segments.push({...structuredClone(snapshot.project.segments[0]),id:"s2",text:"次の原稿",takes:[],selected:null});await import("../../src/main");await settle();await tab("録音");action("採用");await settle();expect(document.querySelector("#text")?.textContent??document.querySelector<HTMLInputElement>("#text")?.value).toContain("次の原稿");});
 it("読み確認工程に正解読み欄がある",async()=>{await import("../../src/main");await settle();await tab("読み確認");expect(document.querySelector("#kana")).toBeTruthy();});
 it("比較工程は候補名を非開示で表示する",async()=>{await import("../../src/main");await settle();await tab("比較");expect([...document.querySelectorAll("h4")].map(x=>x.textContent)).toEqual(["音声 1","音声 2","音声 3"]);});
 it("比較工程で候補名開示を送る",async()=>{await import("../../src/main");await settle();await tab("比較");action("候補名を開示");await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="reveal")).toBe(true);});
 it("結合・出力工程へ移動できる",async()=>{await import("../../src/main");await settle();await tab("結合・出力");expect(document.body.textContent).toContain("分割単位");});
 it("未収録フィルターは収録済み行を除く",async()=>{await import("../../src/main");await settle();const f=document.querySelector<HTMLSelectElement>("#filter")!;f.value="unrecorded";f.dispatchEvent(new Event("change"));expect(document.querySelectorAll("[data-segment]")).toHaveLength(0);});
 it("原稿編集はblurで保存する",async()=>{await import("../../src/main");await settle();await tab("録音");const text=document.querySelector<HTMLTextAreaElement>("#text")!;text.value="変更";text.dispatchEvent(new FocusEvent("blur"));await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="edit")).toBe(true);});
 it("新規はRPCを呼ぶ",async()=>{snapshot.project=null;await import("../../src/main");await settle();action("新規プロジェクト");await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="new")).toBe(true);});
 it("開くはRPCを呼ぶ",async()=>{snapshot.project=null;await import("../../src/main");await settle();action("開く");await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="open")).toBe(true);});
 it("状態取得を多重実行しない",async()=>{await import("../../src/main");await settle();api.invoke.mockClear();await vi.advanceTimersByTimeAsync(2100);expect(api.invoke.mock.calls.filter(x=>x[1]?.action==="state").length).toBeLessThanOrEqual(3);});
 it("フォルダー選択中に操作をロックする",async()=>{snapshot.project=null;await import("../../src/main");await settle();let done!:()=>void;api.invoke.mockImplementation(async(_c:string,{action}:any)=>action==="new"?await new Promise<void>(r=>done=r):structuredClone(snapshot));action("新規プロジェクト");await settle();expect(document.querySelector("[role=dialog]")).toBeTruthy();done();});
});


describe("収録の状態遷移",()=>{
 it("全件の録音・保存・採用を繰り返して最後に読み確認へ移る",async()=>{
  snapshot.project.segments=[0,1,2].map(i=>({...structuredClone(snapshot.project.segments[0]),id:`s${i}`,text:`原稿${i}`,takes:[],selected:null}));
  let recording="";
  const original=api.invoke.getMockImplementation()!;
  api.invoke.mockImplementation(async(command:string,args:any)=>{
   const {action,payload}=args;
   if(action==="record"){recording=payload.segment;snapshot.busy=true;snapshot.job="recording";return null;}
   if(action==="stop"){const s=snapshot.project.segments.find((x:any)=>x.id===recording);s.takes.push({id:`t-${recording}`,status:"ready",duration:1,file:"a.wav",hash:"a"});snapshot.busy=false;snapshot.job="";snapshot.project.generation++;return null;}
   if(action==="select"){snapshot.project.segments.find((x:any)=>x.id===payload.segment).selected=payload.take;snapshot.project.generation++;return null;}
   return original(command,args);
  });
  await import("../../src/main");await settle();await tab("録音");
  for(let i=0;i<3;i++){
   expect(document.querySelector<HTMLButtonElement>('[data-action=record]')!.disabled).toBe(false);
   action("録音開始");await settle();
   expect(document.querySelector<HTMLButtonElement>('[data-action=stop]')!.disabled).toBe(false);
   action("停止して保存");await settle();
   expect(document.querySelector<HTMLButtonElement>('[data-action=select]')!.disabled).toBe(false);
   action("採用");await settle();
  }
  expect(snapshot.project.segments.every((s:any)=>s.selected)).toBe(true);
  expect(document.querySelector('[data-tab=reading]')!.getAttribute('aria-selected')).toBe('true');
  await tab("結合・出力");expect(document.querySelector('#export-output')).toBeTruthy();
 });
 it("明示開始したF4は最新テイクを採用して次を録音し終了時に監視解除する",async()=>{
  snapshot.project.segments.push({...structuredClone(snapshot.project.segments[0]),id:'s2',text:'次',takes:[],selected:null});
  snapshot.project.segments[0].takes.push({...snapshot.project.segments[0].takes[0],id:'latest'});
  await import('../../src/main');await settle();await tab('録音');
  document.querySelector<HTMLButtonElement>('#continuous-start')!.click();await settle();
  expect(api.invoke).toHaveBeenCalledWith('continuous_hotkeys',{enabled:true});
  const handler=api.listen.mock.calls.find(x=>x[0]==='scriptsync-hotkey')![1];
  handler({payload:'F4'});await settle();await settle();
  expect(api.invoke).toHaveBeenCalledWith('command',{action:'select',payload:{segment:'s1',take:'latest'}});
  expect(api.invoke).toHaveBeenCalledWith('command',{action:'record',payload:{segment:'s2',endpoint:'mic1'}});
  document.querySelector<HTMLButtonElement>('#continuous-stop')!.click();await settle();
  expect(api.invoke).toHaveBeenCalledWith('continuous_hotkeys',{enabled:false});
  api.invoke.mockClear();handler({payload:'F1'});await settle();expect(api.invoke).not.toHaveBeenCalled();
 });
 it("保存が失敗しても待機状態の操作をロックし続けない",async()=>{
  const original=api.invoke.getMockImplementation()!;api.invoke.mockImplementation(async(command:string,args:any)=>{if(args.action==='select')throw new Error('保存失敗');return original(command,args);});
  await import('../../src/main');await settle();await tab('録音');action('採用');await settle();
  expect(document.querySelector<HTMLButtonElement>('[data-action=record]')!.disabled).toBe(false);
  expect(document.querySelector('#app')!.hasAttribute('aria-busy')).toBe(false);
  expect(document.querySelector('#notice')!.textContent).toContain('保存失敗');
 });
});

it('録音操作は原稿より前、連続操作は発話ナビゲーションに配置する',async()=>{
 await import('../../src/main');await settle();await tab('録音');
 const column=document.querySelector('.recording-script')!;
 expect(column.firstElementChild?.classList.contains('record-controls')).toBe(true);
 expect(document.querySelector('.utterance-heading #continuous-start')).toBeTruthy();
 expect(document.querySelector('.record-controls #continuous-start')).toBeNull();
 expect(document.querySelector('#recording-wave')).toBeTruthy();
 snapshot.recording_peaks=[.01,.3,.9];snapshot.elapsed=1234;
 await vi.advanceTimersByTimeAsync(700);await settle();
 expect(document.querySelector('#recording-elapsed')?.textContent).toBe('1.2 秒');
 expect(document.querySelector('#recording-wave path:last-child')?.getAttribute('d')).toContain('v');
});
it('F1は取消完了を待って再録音しF2保存F3直前再生を行う',async()=>{
 vi.spyOn(HTMLMediaElement.prototype,'play').mockResolvedValue();
 const original=api.invoke.getMockImplementation()!;
 api.invoke.mockImplementation(async(command:string,args:any)=>{
  if(args.action==='record'){snapshot.busy=true;snapshot.job='recording';return null;}
  if(args.action==='cancel_record'){snapshot.job='saving';return null;}
  if(args.action==='stop'){snapshot.busy=false;snapshot.job='';snapshot.project.segments[0].takes.push({...snapshot.project.segments[0].takes[0],id:'new',file:'new.wav',hash:'new'});snapshot.project.generation++;return null;}
  if(args.action==='audio')return 'data:audio/wav;base64,';
  return original(command,args);
 });
 await import('../../src/main');await settle();await tab('録音');
 document.querySelector<HTMLButtonElement>('#continuous-start')!.click();await settle();
 expect(api.invoke.mock.calls.filter(x=>x[1]?.action==='record')).toHaveLength(1);
 for(const name of ['record','cancel_record','stop'])expect(document.querySelector<HTMLButtonElement>(`[data-action=${name}]`)!.disabled).toBe(true);
 const handler=api.listen.mock.calls.find(x=>x[0]==='scriptsync-hotkey')![1];
 handler({payload:'F1'});await settle();handler({payload:'F1'});await settle();
 expect(api.invoke.mock.calls.filter(x=>x[1]?.action==='cancel_record')).toHaveLength(1);
 expect(api.invoke.mock.calls.filter(x=>x[1]?.action==='record')).toHaveLength(1);
 snapshot.busy=false;snapshot.job='';await vi.advanceTimersByTimeAsync(700);await settle();
 expect(api.invoke.mock.calls.filter(x=>x[1]?.action==='record')).toHaveLength(2);
 handler({payload:'F2'});await settle();
 expect(api.invoke).toHaveBeenCalledWith('command',{action:'stop',payload:{}});
 handler({payload:'F3'});await settle();
 expect(api.invoke).toHaveBeenCalledWith('command',{action:'audio',payload:{file:'new.wav',hash:'new'}});
 document.querySelector<HTMLButtonElement>('#continuous-stop')!.click();await settle();
 expect(document.querySelector<HTMLButtonElement>('[data-action=record]')!.disabled).toBe(false);
});
