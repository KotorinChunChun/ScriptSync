vi.mock("../../src/soundEffects",()=>({playCue:vi.fn(async()=>{})}));
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
 it("ドロップしたMarkdownを下書きに読み込み、確定は送らない",async()=>{
  await import("../../src/main");await settle();
  const original=api.invoke.getMockImplementation()!;
  api.invoke.mockImplementation(async(c:string,args:any)=>args.action==="markdown_drop"?{text:"結合した本文",lines:[],names:["01.md","02.md"]}:original(c,args));
  const callback=api.listen.mock.calls.filter(c=>c[0]==="tauri://drag-drop").at(-1)![1];
  callback({payload:{paths:["C:/02.md","C:/01.md"]}});await settle();
  expect(document.querySelector<HTMLTextAreaElement>("#markdown-source")!.value).toBe("結合した本文");
  expect(document.body.textContent).toContain("01.md → 02.md");
  expect(api.invoke.mock.calls.some(c=>c[1]?.action==="markdown_commit")).toBe(false);
 });
 it("ドロップ読込失敗時は入力中の原稿を保持する",async()=>{
  await import("../../src/main");await settle();
  const input=document.querySelector<HTMLTextAreaElement>("#markdown-source")!;input.value="編集中";input.dispatchEvent(new Event("input"));
  const original=api.invoke.getMockImplementation()!;
  api.invoke.mockImplementation(async(c:string,args:any)=>{if(args.action==="markdown_drop")throw new Error("読込失敗");return original(c,args);});
  const callback=api.listen.mock.calls.filter(c=>c[0]==="tauri://drag-drop").at(-1)![1];
  callback({payload:{paths:["C:/missing.md"]}});await settle();
  expect(document.querySelector<HTMLTextAreaElement>("#markdown-source")!.value).toBe("編集中");
  expect(document.body.textContent).toContain("読込失敗");
 });
 it("起動画面は新規・開く・5工程だけを表示する",async()=>{snapshot.project=null;await import("../../src/main");await settle();expect(document.body.textContent).toContain("新規プロジェクト");expect(document.body.textContent).toContain("結合・出力");expect(document.querySelector("aside")).toBeNull();});
 it("台本が初期工程である",async()=>{await import("../../src/main");await settle();expect(document.querySelector("[data-tab=script]")?.getAttribute("aria-selected")).toBe("true");});
 it("工程バーは作業領域の外にあり、出力工程では左ペインを表示しない",async()=>{await import("../../src/main");await settle();expect(document.querySelector(".stage-bar")?.parentElement?.id).toBe("app");await tab("結合・出力");expect(document.querySelector(".navigation-aside")).toBeNull();});
 it("台本タブにMarkdown原稿を表示する",async()=>{await import("../../src/main");await settle();expect(document.querySelector<HTMLTextAreaElement>("#markdown-source")?.value).toContain("# 部");});
 it("原稿行をオンオフできる",async()=>{await import("../../src/main");await settle();const row=document.querySelector<HTMLButtonElement>("[data-script-line]")!;row.click();await settle();expect(document.querySelector(".source-line.disabled")).toBeTruthy();});
 it("Markdown確認はRPCへ送る",async()=>{await import("../../src/main");await settle();action("行を確認");await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="markdown_preview")).toBe(true);});
 it("Markdown確定は行情報を送る",async()=>{await import("../../src/main");await settle();action("台本を確定");await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==="markdown_commit")).toBe(true);});
it("台本工程の左ペインはフィルターと元Markdown構成を表示する",async()=>{await import("../../src/main");await settle();expect(document.querySelector("#filter")).toBeTruthy();expect(document.querySelector("aside")?.textContent).toContain("部");});
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
it("未収録フィルターは収録済み行を除く",async()=>{await import("../../src/main");await settle();await tab("録音");const f=document.querySelector<HTMLSelectElement>("#filter")!;f.value="unrecorded";f.dispatchEvent(new Event("change"));expect(document.querySelectorAll("[data-segment]")).toHaveLength(0);});
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
 expect(document.querySelector('[data-action=record]')).toBeNull();for(const name of ['cancel_record','stop'])expect(document.querySelector<HTMLButtonElement>(`[data-action=${name}]`)!.disabled).toBe(true);
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

it('画面再接続で録音中を検出しても保存完了後に操作を復帰する',async()=>{
 const original=api.invoke.getMockImplementation()!;snapshot.busy=true;snapshot.job='recording';api.invoke.mockImplementation(async(command:string,args:any)=>args.action==='ui_attach'?{recovering:true}:original(command,args));
 await import('../../src/main');await settle();expect(document.querySelector('[data-tab=recording]')?.getAttribute('aria-selected')).toBe('true');
 expect(document.querySelector<HTMLButtonElement>('[data-action=stop]')!.disabled).toBe(false);
 snapshot.busy=false;snapshot.job='';snapshot.project.generation++;await vi.advanceTimersByTimeAsync(700);await settle();
 expect(document.querySelector<HTMLButtonElement>('[data-action=record]')!.disabled).toBe(false);expect(document.querySelector('#app')!.hasAttribute('aria-busy')).toBe(false);
});
it('保存中のF1は追加録音を予約し保存済みテイクを取り消さない',async()=>{
 const original=api.invoke.getMockImplementation()!;
 api.invoke.mockImplementation(async(command:string,args:any)=>{if(args.action==='record'){snapshot.busy=true;snapshot.job='recording';return null;}if(args.action==='stop'){snapshot.job='saving';return null;}return original(command,args);});
 await import('../../src/main');await settle();await tab('録音');document.querySelector<HTMLButtonElement>('#continuous-start')!.click();await settle();
 const handler=api.listen.mock.calls.find(x=>x[0]==='scriptsync-hotkey')![1];handler({payload:'F2'});await settle();handler({payload:'F1'});handler({payload:'F1'});await settle();
 expect(api.invoke.mock.calls.filter(x=>x[1]?.action==='cancel_record')).toHaveLength(0);
 snapshot.project.segments[0].takes.push({...snapshot.project.segments[0].takes[0],id:'saved-new'});snapshot.busy=false;snapshot.job='';snapshot.project.generation++;
 await vi.advanceTimersByTimeAsync(700);await settle();
 expect(api.invoke.mock.calls.filter(x=>x[1]?.action==='record')).toHaveLength(2);expect(snapshot.project.segments[0].takes.map((t:any)=>t.id)).toEqual(['t1','saved-new']);
 const {playCue}=await import('../../src/soundEffects');vi.mocked(playCue).mockClear();handler({payload:'F3'});await settle();expect(playCue).toHaveBeenCalledWith('invalid');
});
it('未保存トリミングをF4で採用せず、移動前に保存を選択できる',async()=>{
 await import('../../src/main');await settle();await tab('録音');document.querySelector<HTMLButtonElement>('#continuous-start')!.click();await settle();
 const wave=document.querySelector<HTMLElement>('.take-wave-wrap')!,handle=wave.querySelector<HTMLButtonElement>('.take-trim-handle.start')!;
 vi.spyOn(wave,'getBoundingClientRect').mockReturnValue({left:0,width:100} as DOMRect);
 const pointer=(x:number)=>Object.assign(new Event('pointermove',{bubbles:true}),{clientX:x,pointerId:1}) as PointerEvent;
 handle.onpointerdown!(pointer(0));wave.onpointermove!(pointer(25));wave.onpointerup!(pointer(25));
 const handler=api.listen.mock.calls.find(x=>x[0]==='scriptsync-hotkey')![1];handler({payload:'F4'});await settle();expect(api.invoke.mock.calls.some(x=>x[1]?.action==='select')).toBe(false);
 await tab('結合・出力');expect(document.querySelector('.take-trim-modal')).toBeTruthy();document.querySelector<HTMLButtonElement>('[data-choice=save]')!.click();await settle();
 expect(api.invoke.mock.calls.some(x=>x[1]?.action==='trim_take')).toBe(true);expect(document.querySelector('[data-tab=export]')?.getAttribute('aria-selected')).toBe('true');
});

it('停止延長は既定0秒で指定値を停止RPCへ渡す',async()=>{
 await import('../../src/main');await settle();await tab('録音');
 const field=document.querySelector<HTMLInputElement>('#stop-extension')!;
 expect(field.value).toBe('0');field.value='2';field.dispatchEvent(new Event('change'));
 snapshot.busy=true;snapshot.job='recording';await vi.advanceTimersByTimeAsync(800);await settle();
 action('停止して保存');await settle();
 expect(api.invoke).toHaveBeenCalledWith('command',{action:'stop',payload:{delay_seconds:2}});
});
it('無効化した確定原稿は即保存し録音から除外する',async()=>{
 const original=api.invoke.getMockImplementation()!;
 api.invoke.mockImplementation(async(c:string,a:any)=>{if(a.action==='markdown_commit'){snapshot.project.segments=[];snapshot.project.generation++;return null;}return original(c,a);});
 await import('../../src/main');await settle();
 document.querySelector<HTMLButtonElement>('[data-script-line]')!.click();await settle();
 expect(api.invoke.mock.calls.some(x=>x[1]?.action==='markdown_commit')).toBe(true);
 await tab('録音');expect(document.querySelector('[data-action=record]')).toBeNull();
});
it('無効な原稿を採用後の移動から飛ばす',async()=>{
 snapshot.project.segments.push({...structuredClone(snapshot.project.segments[0]),id:'off',skip:true,text:'無効'}, {...structuredClone(snapshot.project.segments[0]),id:'active',text:'有効'});
 await import('../../src/main');await settle();await tab('録音');action('採用');await settle();
 expect(document.querySelector<HTMLTextAreaElement>('#text')?.value).toBe('有効');
 expect(document.querySelector('[data-segment="off"]')).toBeNull();
});
it('通常録音のF1は開始と取消を切り替えF2保存F3再生F4最新採用を行う',async()=>{
 const original=api.invoke.getMockImplementation()!;
 snapshot.project.segments.push({...structuredClone(snapshot.project.segments[0]),id:'next',takes:[],selected:null});
 api.invoke.mockImplementation(async(c:string,a:any)=>{
  if(a.action==='record'){snapshot.busy=true;snapshot.job='recording';return null;}
  if(a.action==='cancel_record'||a.action==='stop'){snapshot.busy=false;snapshot.job='';return null;}
  if(a.action==='audio')return 'test.wav';
  return original(c,a);
 });
 vi.spyOn(HTMLMediaElement.prototype,'play').mockResolvedValue();
 await import('../../src/main');await settle();await tab('録音');
 const key=async(key:string)=>{document.dispatchEvent(new KeyboardEvent('keydown',{key,bubbles:true,cancelable:true}));await settle();};
 await key('F1');expect(snapshot.job).toBe('recording');await key('F1');expect(snapshot.busy).toBe(false);
 expect(snapshot.project.segments[0].takes).toHaveLength(1);
 await key('F1');await key('F2');expect(snapshot.busy).toBe(false);
 await key('F3');expect(api.invoke.mock.calls.some(x=>x[1]?.action==='audio')).toBe(true);
 await key('F4');expect(api.invoke).toHaveBeenCalledWith('command',{action:'select',payload:{segment:'s1',take:'t1'}});
 expect(api.invoke.mock.calls.some(x=>x[0]==='continuous_hotkeys')).toBe(false);
 expect(api.invoke.mock.calls.filter(x=>x[1]?.action==='record')).toHaveLength(2);
});
it('テイク範囲試聴は指定した終了時間で停止する',async()=>{
 const original=api.invoke.getMockImplementation()!;
 api.invoke.mockImplementation(async(c:string,a:any)=>{if(a.action==='preview_take_trim')return {file:'clip.wav',hash:'clip'};if(a.action==='audio')return 'clip.wav';return original(c,a);});
 vi.spyOn(HTMLMediaElement.prototype,'play').mockResolvedValue();
 await import('../../src/main');await settle();await tab('録音');
 const end=document.querySelector<HTMLInputElement>('[data-trim-end]')!;end.value='0.50';end.dispatchEvent(new Event('change'));
 document.querySelector<HTMLButtonElement>('[data-trim-preview]')!.click();await settle();
 const audio=document.querySelector<HTMLAudioElement>('#player')!;audio.currentTime=0.7;audio.dispatchEvent(new Event('timeupdate'));
 expect(audio.currentTime).toBe(0.5);expect(audio.pause).toHaveBeenCalled();
 expect(api.invoke).toHaveBeenCalledWith('command',{action:'preview_take_trim',payload:{segment:'s1',take:'t1',start:0,end:0.5}});
});

it('欠落警告があるテイクは確認前に採用しない',async()=>{
 snapshot.project.segments[0].takes[0].warning='要確認：WASAPI不連続 1回';
 await import('../../src/main');await settle();await tab('録音');action('採用');await settle();
 expect(document.querySelector('[role=dialog]')?.textContent).toContain('WASAPI不連続');
 expect(api.invoke.mock.calls.some(x=>x[1]?.action==='select')).toBe(false);
 document.querySelector<HTMLButtonElement>('[data-quality-adopt]')!.click();await settle();
 expect(api.invoke.mock.calls.some(x=>x[1]?.action==='select')).toBe(true);
});


it('評価は未評価と10点刻み11段階で即時保存しメモを保持する',async()=>{
 snapshot.project.segments[0].take_notes={t1:{memo:'残すメモ',favorite:false,score:null}};
 await import('../../src/main');await settle();await tab('録音');
 const field=document.querySelector<HTMLSelectElement>('[data-take-score]')!;expect(field.options).toHaveLength(12);expect(field.value).toBe('');
 field.value='80';field.dispatchEvent(new Event('change'));await settle();
 expect(api.invoke).toHaveBeenCalledWith('command',{action:'take_score',payload:{segment:'s1',take:'t1',score:80}});
 expect(document.querySelector<HTMLTextAreaElement>('[data-take-memo]')!.value).toBe('残すメモ');
 field.value='';field.dispatchEvent(new Event('change'));await settle();expect(api.invoke).toHaveBeenCalledWith('command',{action:'take_score',payload:{segment:'s1',take:'t1',score:null}});
});
it('試聴とF3は再生・停止で表示を切り替え自然終了で戻る',async()=>{
 const original=api.invoke.getMockImplementation()!;
 api.invoke.mockImplementation(async(c:string,a:any)=>a.action==='preview_take_trim'?{file:'clip.wav',hash:'clip'}:a.action==='audio'?'blob:test':original(c,a));
 let paused=true;vi.spyOn(HTMLMediaElement.prototype,'paused','get').mockImplementation(()=>paused);
 vi.spyOn(HTMLMediaElement.prototype,'play').mockImplementation(async function(this:HTMLMediaElement){paused=false;this.dispatchEvent(new Event('play'));});
 vi.spyOn(HTMLMediaElement.prototype,'pause').mockImplementation(function(this:HTMLMediaElement){paused=true;this.dispatchEvent(new Event('pause'));});
 await import('../../src/main');await settle();await tab('録音');
 const b=document.querySelector<HTMLButtonElement>('[data-trim-preview]')!;b.click();await settle();expect(b.textContent).toBe('停止(F3)');
 document.dispatchEvent(new KeyboardEvent('keydown',{key:'F3',bubbles:true}));await settle();expect(b.textContent).toBe('試聴(F3)');
 b.click();await settle();expect(b.textContent).toBe('停止(F3)');paused=true;document.querySelector('audio')!.dispatchEvent(new Event('ended'));expect(b.textContent).toBe('試聴(F3)');
});

it('品質確認は採用が既定でEnterで決定し過剰な文言を表示しない',async()=>{
 snapshot.project.segments[0].takes[0].warning='要確認：WASAPI不連続 37回\n欠落した発話は復元できません。試聴して再録音を検討してください';
 await import('../../src/main');await settle();await tab('録音');action('採用');await settle();
 const button=document.querySelector<HTMLButtonElement>('[data-quality-adopt]')!;
 expect(button.textContent).toBe('採用');expect(button.classList.contains('primary')).toBe(true);expect(document.activeElement).toBe(button);expect(document.body.textContent).not.toContain('欠落した発話は復元できません');
 button.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));await settle();expect(api.invoke.mock.calls.filter(c=>c[1]?.action==='select')).toHaveLength(1);
});
