import { escapeHtml as esc } from "./html";
export type ExportSegment={id:string;text:string;source_text?:string;hierarchy?:number[];kind?:number};
export type ExportContext={
 project:{id?:string;segments:ExportSegment[];export_settings?:unknown};selected:string;select:(id:string)=>void;
 rpc:<T>(action:string,payload?:unknown)=>Promise<T>;play:(file:string,hash:string)=>Promise<void>;
 notice:(message:string,error?:boolean)=>void;player?:HTMLAudioElement|null;
 confirm?:(message:string)=>boolean|Promise<boolean>;
};
type GapKey="normal"|"part"|"chapter"|"section"|"head"|"tail";
type Settings={unit:string;naming:string;format:string;subtitle_format:"vtt"|"none";normalize:boolean;boundaries:string[];global_gaps:Record<GapKey,number>;blocks:Record<string,{name?:string;gaps?:Partial<Record<GapKey,{mode:string;value?:number}>>}>};
export type ExportDurationSummary={audio_seconds:number;wait_seconds:Record<GapKey,number>;wait_total_seconds:number;total_seconds:number;unselected_count:number};
type PreviewFile={mode:string;file:string;hash:string;frames:number};
type Preview={cached?:boolean;files:PreviewFile[];timeline?:{block:number;offset?:number;offset_seconds?:number;duration?:number;duration_seconds?:number}[]};
const gaps:[GapKey,string][]=[["part","部の区切り"],["chapter","章の区切り"],["section","節の区切り"],["normal","通常の発話間"],["head","ファイル先頭"],["tail","ファイル末尾"]];
const defaults=():Settings=>({unit:"all",naming:"jp",format:"flac",subtitle_format:"vtt",normalize:true,boundaries:[],global_gaps:{normal:.3,part:1.2,chapter:.8,section:.5,head:.1,tail:.3},blocks:{}});
let preparation="";
let settings=defaults(), selectedBlock=0, preview:Preview|undefined, projectKey="", lastOutputId="", playbackBlock=-1;
let boundPlayer:HTMLAudioElement|null=null, previewMode="combined", previewStart=0,previewPlaying=false;
let history:Settings[]=[], savedSettings=JSON.stringify(settings), confirming=false, session=0;
let durationSummary:ExportDurationSummary|undefined, durationSummaryKey="", durationSummaryPending="", durationSummaryError="";
export function hasUnsavedExportDraft(){return !!projectKey&&JSON.stringify(settings)!==savedSettings;}
export async function saveExportDraft(context:ExportContext){if(!hasUnsavedExportDraft())return;const snapshot=structuredClone(settings);await context.rpc('export_settings',{settings:snapshot});savedSettings=JSON.stringify(snapshot);context.project.export_settings=snapshot;}
let undoKeyHandler:((event:KeyboardEvent)=>void)|undefined;
/** プロジェクトを閉じると未出力の編集と履歴を破棄する。同じIDを開き直す場合も必ず呼ぶ。 */
export function resetExportDraft(){session++;projectKey="";settings=defaults();history=[];savedSettings=JSON.stringify(settings);preparation="";confirming=false;selectedBlock=0;preview=undefined;lastOutputId="";previewPlaying=false;playbackBlock=-1;durationSummary=undefined;durationSummaryKey="";durationSummaryPending="";durationSummaryError="";if(undoKeyHandler)document.removeEventListener("keydown",undoKeyHandler);undoKeyHandler=undefined;}
document.addEventListener("play",event=>{if((event.target as HTMLAudioElement).id==="player"){const stop=document.querySelector<HTMLButtonElement>("#export-stop");if(stop)stop.disabled=false;}},true);
function loaded(value:unknown):Settings{const base=defaults();if(!value||typeof value!=="object"||Array.isArray(value))return base;const v=structuredClone(value) as Partial<Settings>;return {...base,...v,subtitle_format:v.subtitle_format==="none"?"none":"vtt",boundaries:Array.isArray(v.boundaries)?v.boundaries.filter(x=>typeof x==="string"):[],global_gaps:{...base.global_gaps,...(v.global_gaps??{})},blocks:typeof v.blocks==="object"&&v.blocks? v.blocks: {}};}
function starts(project:ExportContext["project"]){const all=project.segments; if(!all.length)return[];if(settings.unit==="custom")return [0,...settings.boundaries.map(id=>all.findIndex(s=>s.id===id)).filter(i=>i>0)].sort((a,b)=>a-b);if(settings.unit==="utterance")return all.map((_,i)=>i);if(settings.unit==="all")return[0];const kind:{[key:string]:number}={part:1,chapter:2,section:3},wanted=kind[settings.unit];return [0,...all.map((s,i)=>i&&((!!s.kind&&s.kind<=wanted)||Array.from({length:wanted},(_,level)=>level).some(level=>s.hierarchy?.[level]!==all[i-1].hierarchy?.[level]))?i:-1).filter(i=>i>0)];}
function blockAt(project:ExportContext["project"],index:number){const points=starts(project);const start=points[index]??0;return {start,end:points[index+1]??project.segments.length};}
export function cleanFilenameText(value:string){const clean=Array.from(value.trim().replace(/[<>:"/\\|?*\p{Cc}]/gu,"")).slice(0,20).join("").replace(/[ .]+$/u,"");return clean||"script";}
function name(project:ExportContext["project"],block:number){const {start}=blockAt(project,block),s=project.segments[start];return cleanFilenameText(settings.blocks[s?.id]?.name?.trim()||s?.source_text?.trim()||s?.text?.trim()||"script");}
function prefix(project:ExportContext["project"],block:number){const ordinal=`${String(block+1).padStart(4,"0")}_`,h=project.segments[blockAt(project,block).start]?.hierarchy??[];if(settings.naming==="none")return ordinal;if(settings.naming==="number"){const levels=h.filter(Boolean).map(value=>String(value).padStart(2,"0"));return levels.length?ordinal+levels.join("_")+"_":ordinal;}const labels=["部","章","節","項"],levels=h.map((value,index)=>value?index===3?`${String(value).padStart(2,"0")}項`:`第${value}${labels[index]}`:"").filter(Boolean);return levels.length?ordinal+levels.join("_")+"_":ordinal;}
function settingsPayload(){return structuredClone(settings);}
function summaryKey(project:ExportContext["project"]){return JSON.stringify({settings,segments:project.segments.map(segment=>{const detail=segment as ExportSegment&{skip?:boolean;selected?:string|null;takes?:{id:string;duration:number;trim_start?:number;trim_end?:number}[]};return {id:segment.id,hierarchy:segment.hierarchy,kind:segment.kind,skip:detail.skip,selected:detail.selected,takes:detail.takes};})});}
function seconds(value:number){return `${value.toFixed(2)} 秒`;}
function summaryHtml(summary:ExportDurationSummary|undefined,error:string){if(error)return `<section class="export-duration warning" role="status">時間を計算できません：${esc(error)}</section>`;if(!summary)return `<section class="export-duration muted" role="status">出力時間を計算中です…</section>`;const waits=(["head","normal","part","chapter","section","tail"] as GapKey[]).map(key=>`<span>${({head:"先頭",normal:"通常",part:"部",chapter:"章",section:"節",tail:"末尾"} as Record<GapKey,string>)[key]} ${seconds(summary.wait_seconds[key])}</span>`).join(" ／ ");return `<section class="export-duration" aria-label="出力時間の見込み"><strong>出力時間の見込み ${seconds(summary.total_seconds)}</strong><span>採用音声 ${seconds(summary.audio_seconds)}</span><span>待ち時間合計 ${seconds(summary.wait_total_seconds)}</span><span class="export-waits">${waits}</span>${summary.unselected_count?`<span class="warning">未採用発話 ${summary.unselected_count} 件（見込みには含めません）</span>`:"<span>未採用発話 0 件</span>"}</section>`;}
export function exportView(context:ExportContext):string{const key=context.project.id??context.project.segments.map(s=>s.id).join(",");if(key!==projectKey){session++;projectKey=key;settings=loaded(context.project.export_settings);history=[];savedSettings=JSON.stringify(settings);confirming=false;selectedBlock=0;preview=undefined;lastOutputId="";previewPlaying=false;durationSummary=undefined;durationSummaryKey="";durationSummaryPending="";durationSummaryError="";}const points=starts(context.project);selectedBlock=Math.min(selectedBlock,Math.max(0,points.length-1));const block=blockAt(context.project,selectedBlock), first=context.project.segments[block.start];const extension=settings.format==="wav"?".wav":".flac", keyForSummary=summaryKey(context.project);const currentSummary=durationSummaryKey===keyForSummary?durationSummary:undefined,currentError=durationSummaryKey===keyForSummary?durationSummaryError:"";return `<section class="export-view" aria-label="結合・出力" aria-busy="${!!preparation}"><div class="export-preparation" role="status" ${preparation?"":"hidden"}>${esc(preparation)}</div>
<div class="export-heading"><div><h2>音声ファイルの区切りを整える</h2><p class="muted">境界、ファイル名、間隔を確認してから非破壊で出力します。</p><p class="muted">未採用の発話は警告を表示し、出力からスキップします。</p></div><div><span class="export-count">${points.length} ファイル</span><button id="export-undo" title="元に戻す (Ctrl+Z)" ${history.length?"":"disabled"}>元に戻す</button><span role="status" class="muted export-draft-status">${JSON.stringify(settings)===savedSettings?"":"未出力の変更があります。出力成功時に保存します。"}</span></div></div>
<div class="export-options"><strong>分割単位</strong>${[["all","全体で1ファイル"],["part","部ごと（H1）"],["chapter","章ごと（H2）"],["section","節ごと（H3）"],["utterance","発話ごと"]].map(([key,label])=>`<button data-unit="${key}" aria-pressed="${settings.unit===key}">${label}</button>`).join("")}<button data-unit="custom" aria-pressed="${settings.unit==="custom"}">手動</button><span class="export-naming"><strong>命名規則</strong>${[["none","なし"],["jp","日本語"],["number","2桁数字"]].map(([key,label])=>`<button data-naming="${key}" aria-pressed="${settings.naming===key}">${label}</button>`).join("")}</span></div>
<div class="export-workspace"><section class="export-list"><div class="export-list-head"><span>境界　読み上げ原稿</span><span>クリックでファイルを選択</span></div><div class="export-rows">${context.project.segments.map((s,index)=>{const blockIndex=points.indexOf(index),active=blockIndex>=0?blockIndex:points.findIndex((start,i)=>start<=index&&(points[i+1]??context.project.segments.length)>index);const isStart=blockIndex>=0;return `<div class="export-row ${active%2?"blue":"green"} ${active===selectedBlock?"selected":""} ${active===playbackBlock?"playing":""}" data-block="${active}" data-segment="${esc(s.id)}" tabindex="0" role="button"><button class="export-boundary ${isStart&&index?"existing":""}" data-boundary="${esc(s.id)}" title="${isStart&&index?"境界を解除":"ここで分割"}" ${index?"":"disabled"}>${isStart&&index?"🔗":"✂"}</button><span class="export-number">${String(index+1).padStart(4,"0")}</span><span class="export-text" style="padding-inline-start:${Math.max(0,(s.hierarchy?.filter(Boolean).length??1)-1)}em">${esc(s.text)}</span>${s.kind?`<span class="export-kind">${["","部","章","節","項"][s.kind]}</span>`:""}${isStart?`<span class="export-file">${String(active+1).padStart(4,"0")}</span>`:""}</div>`;}).join("")}</div></section>
<aside class="export-detail"><h3>ファイル ${String(selectedBlock+1).padStart(4,"0")}</h3><p class="export-range">原稿 ${block.start+1} — ${block.end}</p><label>固定の連番・階層表記<output id="export-prefix">${esc(prefix(context.project,selectedBlock))}</output></label><label>原稿から作るファイル名<div class="export-filename"><input id="export-name" value="${esc(name(context.project,selectedBlock))}"><span>${extension}</span></div></label><p class="export-preview">${esc(prefix(context.project,selectedBlock)+name(context.project,selectedBlock)+extension)}</p><h4>このブロックに挿入する無音時間（秒）</h4>${gaps.map(([key,label])=>{const state=settings.blocks[first?.id]?.gaps?.[key]??{mode:"default"};return `<div class="export-gap"><span>${label}</span><select data-gap-mode="${key}"><option value="default" ${state.mode!=="manual"?"selected":""}>既定</option><option value="manual" ${state.mode==="manual"?"selected":""}>手入力</option></select><input data-gap-value="${key}" type="number" min="0" max="60" step="0.1" value="${state.mode==="manual"?(state.value??settings.global_gaps[key]):settings.global_gaps[key]}" ${state.mode==="manual"?"":"disabled"}></div>`;}).join("")}<p class="muted">境界を変えても原稿の順番と録音は変更しません。</p></aside></div>
<section class="export-defaults"><h3>全体の既定値：挿入する無音時間（秒）</h3><div>${gaps.map(([key,label])=>`<label>${label}<input data-global-gap="${key}" type="number" min="0" max="60" step="0.1" value="${settings.global_gaps[key]}"></label>`).join("")}</div></section>
${summaryHtml(currentSummary,currentError)}
<div class="export-actions"><label class="inline"><input id="export-normalize" type="checkbox" ${settings.normalize?"checked":""}> 音量を均一にする</label><label class="inline">字幕ファイル出力形式<select id="export-subtitle-format"><option value="vtt" ${settings.subtitle_format==="vtt"?"selected":""}>VTT</option><option value="none" ${settings.subtitle_format==="none"?"selected":""}>なし</option></select></label><label class="inline">音声形式<select id="export-format"><option value="flac" ${settings.format==="flac"?"selected":""}>FLAC</option><option value="wav" ${settings.format==="wav"?"selected":""}>WAV</option></select></label><span class="export-spacer"></span><button id="export-first">最初から再生</button><button id="export-preview">選択ブロックを試聴</button><button id="export-last">選択ブロックの最後の項から試聴</button><button id="export-stop" ${previewPlaying?"":"disabled"}>停止</button><button id="export-output" class="primary">音声ファイルを出力</button><button id="export-folder">出力フォルダーを開く</button></div></section>`;}
export function bindExportView(context:ExportContext){
 const root=document.querySelector<HTMLElement>(".export-view"),player=context.player??document.querySelector<HTMLAudioElement>("#player");
 if(!root)return;
 const viewSession=session;
 if(preparation||confirming)root.querySelectorAll<HTMLButtonElement|HTMLInputElement|HTMLSelectElement>("button:not(#export-folder),input,select").forEach(el=>el.disabled=true);
 const rerender=()=>{if(viewSession!==session)return;const view=document.querySelector(".export-view");if(view)view.outerHTML=exportView(context);bindExportView(context);};
 const refreshDuration=()=>{
  const key=summaryKey(context.project);if(key===durationSummaryKey||key===durationSummaryPending)return;
  durationSummaryPending=key;durationSummaryError="";
  void context.rpc<ExportDurationSummary>("duration_summary",settingsPayload()).then(result=>{
   if(viewSession!==session||durationSummaryPending!==key)return;
   if(!result||!Number.isFinite(result.audio_seconds)||!Number.isFinite(result.wait_total_seconds)||!Number.isFinite(result.total_seconds)||!Number.isInteger(result.unselected_count)||!gaps.every(([gap])=>Number.isFinite(result.wait_seconds?.[gap]))){throw new Error("時間集計の応答が不正です");}
   durationSummary=result;durationSummaryKey=key;durationSummaryPending="";rerender();
  }).catch(error=>{if(viewSession!==session||durationSummaryPending!==key)return;durationSummary=undefined;durationSummaryKey=key;durationSummaryPending="";durationSummaryError=String(error);rerender();});
 };
 refreshDuration();
 const changed=(change:()=>void)=>{
  if(preparation||confirming)return;
  const previous=settingsPayload();change();
  if(JSON.stringify(previous)!==JSON.stringify(settings)){history.push(previous);if(history.length>100)history.shift();preview=undefined;}
  rerender();
 };
 const undo=()=>{if(preparation||confirming||!history.length)return;settings=history.pop()!;preview=undefined;rerender();};
 root.querySelector<HTMLButtonElement>("#export-undo")!.onclick=undo;
 if(undoKeyHandler)document.removeEventListener("keydown",undoKeyHandler);
 undoKeyHandler=event=>{
  if(!root.isConnected||event.defaultPrevented||document.querySelector("dialog[open],[role=dialog]"))return;
  if(event.ctrlKey&&!event.shiftKey&&!event.altKey&&event.key.toLowerCase()==="z"&&!(event.target instanceof HTMLElement&&event.target.closest("input,textarea,[contenteditable]:not([contenteditable=false])"))){
   event.preventDefault();event.stopPropagation();undo();
  }
 };
 document.addEventListener("keydown",undoKeyHandler);
 const effectiveIds=()=>starts(context.project).slice(1).map(index=>context.project.segments[index].id);
 const play=async(file:PreviewFile)=>{previewMode=file.mode;await context.play(file.file,file.hash);if(viewSession!==session)return;previewPlaying=true;const stop=document.querySelector<HTMLButtonElement>("#export-stop");if(stop)stop.disabled=false;};
 const updateTimeline=()=>{
  if(viewSession!==session||!player||!preview?.timeline)return;
  const time=player.currentTime;
  const item=preview.timeline.find(entry=>{const offset=entry.offset_seconds??entry.offset??0,duration=entry.duration_seconds??entry.duration??0;return time>=offset&&time<offset+duration;});
  const next=previewMode==="combined"?(item?.block??-1):previewStart;
  if(next!==playbackBlock){playbackBlock=next;if(next>=0){selectedBlock=next;rerender();document.querySelector<HTMLElement>(`.export-row[data-block="${next}"]`)?.scrollIntoView?.({block:"nearest"});}document.querySelectorAll(".export-row").forEach(row=>row.classList.toggle("playing",Number((row as HTMLElement).dataset.block)===next));}
 };
 // 最新のプロジェクト・設定を参照する。再描画のたびにリスナーを増やさない。
 if(player){
  if(boundPlayer&&boundPlayer!==player){boundPlayer.ontimeupdate=null;boundPlayer.onended=null;}
  boundPlayer=player;player.ontimeupdate=updateTimeline;
  player.onended=()=>{if(viewSession!==session)return;previewPlaying=false;const stop=document.querySelector<HTMLButtonElement>("#export-stop");if(stop)stop.disabled=true;playbackBlock=-1;document.querySelectorAll(".export-row").forEach(row=>row.classList.remove("playing"));};
 }
 root.querySelectorAll<HTMLElement>("[data-block]").forEach(row=>{
  const activate=()=>{selectedBlock=Number(row.dataset.block);context.select(row.dataset.segment??"");rerender();};
  row.onclick=event=>{if((event.target as HTMLElement).closest("[data-boundary]"))return;activate();};
  row.onkeydown=event=>{if(event.target!==row)return;if(event.key==="Enter"||event.key===" "){event.preventDefault();activate();}};
 });
 root.querySelectorAll<HTMLButtonElement>("[data-boundary]").forEach(button=>button.onclick=event=>{
  event.stopPropagation();changed(()=>{const id=button.dataset.boundary!;const boundaries=effectiveIds();settings.unit="custom";settings.boundaries=boundaries.includes(id)?boundaries.filter(value=>value!==id):[...boundaries,id];});
 });
 root.querySelectorAll<HTMLButtonElement>("[data-unit]").forEach(button=>button.onclick=async()=>{
  const unit=button.dataset.unit!;if(preparation||confirming||settings.unit===unit)return;
  if(settings.unit==="custom"&&unit!=="custom"){
   confirming=true;rerender();
   try{
    const message="手動で設定した分割境界を、自動分割の境界に置き換えますか？\n元に戻す (Ctrl+Z) で変更前に戻せます。";
    const accepted=await (context.confirm?context.confirm(message):window.confirm(message));
    if(viewSession!==session)return;
    confirming=false;if(!accepted){rerender();return;}
   }catch(error){if(viewSession!==session)return;confirming=false;context.notice(String(error),true);rerender();return;}
  }
  changed(()=>{const boundaries=effectiveIds();settings.unit=unit;settings.boundaries=unit==="custom"?boundaries:[];});
 });
 root.querySelectorAll<HTMLButtonElement>("[data-naming]").forEach(button=>button.onclick=()=>changed(()=>{settings.naming=button.dataset.naming!;}));
 root.querySelector<HTMLInputElement>("#export-name")!.onchange=event=>changed(()=>{
  const first=blockAt(context.project,selectedBlock).start,id=context.project.segments[first]?.id;
  if(id){settings.blocks[id]??={};settings.blocks[id].name=cleanFilenameText((event.target as HTMLInputElement).value);}
 });
 root.querySelectorAll<HTMLInputElement>("[data-global-gap]").forEach(input=>input.onchange=()=>changed(()=>{settings.global_gaps[input.dataset.globalGap as GapKey]=Number(input.value);}));
 root.querySelectorAll<HTMLSelectElement>("[data-gap-mode]").forEach(select=>select.onchange=()=>changed(()=>{
  const first=context.project.segments[blockAt(context.project,selectedBlock).start];if(!first)return;
  settings.blocks[first.id]??={};const key=select.dataset.gapMode as GapKey;const old=settings.blocks[first.id].gaps?.[key];settings.blocks[first.id].gaps??={};settings.blocks[first.id].gaps![key]={...old,mode:select.value,value:old?.value??settings.global_gaps[key]};
 }));
 root.querySelectorAll<HTMLInputElement>("[data-gap-value]").forEach(input=>input.onchange=()=>changed(()=>{
  const first=context.project.segments[blockAt(context.project,selectedBlock).start];if(!first)return;
  const key=input.dataset.gapValue as GapKey;settings.blocks[first.id]??={};settings.blocks[first.id].gaps??={};settings.blocks[first.id].gaps![key]={...settings.blocks[first.id].gaps![key],mode:"manual",value:Number(input.value)};
 }));
 root.querySelector<HTMLInputElement>("#export-normalize")!.onchange=event=>changed(()=>{settings.normalize=(event.target as HTMLInputElement).checked;});
 root.querySelector<HTMLSelectElement>("#export-format")!.onchange=event=>changed(()=>{settings.format=(event.target as HTMLSelectElement).value;});
 root.querySelector<HTMLSelectElement>("#export-subtitle-format")!.onchange=event=>changed(()=>{settings.subtitle_format=(event.target as HTMLSelectElement).value==="none"?"none":"vtt";});
 const startPreview=async(start:number,last=false,scope="block")=>{
  if(preparation||confirming)return;preparation="試聴用の音声データを生成中です…";previewPlaying=false;player?.pause();rerender();context.notice(preparation);
  try{
   previewStart=start;const result=await context.rpc<Preview>("preview_blocks",{...settingsPayload(),selected_block:start,from_last_item:last,preview_scope:scope});
   if(viewSession!==session)return;preview=result;const combined=preview.files.find(file=>file.mode==="combined");rerender();
   if(combined){await play(combined);if(viewSession===session)context.notice(preview.cached?"キャッシュした音声を再生中":"再生中");}
  }catch(error){if(viewSession===session)context.notice(String(error),true);}finally{if(viewSession===session){preparation="";rerender();}}
 };
 root.querySelector<HTMLButtonElement>("#export-first")!.onclick=()=>void startPreview(0,false,"all");
 root.querySelector<HTMLButtonElement>("#export-preview")!.onclick=()=>void startPreview(selectedBlock);
 root.querySelector<HTMLButtonElement>("#export-last")!.onclick=()=>void startPreview(selectedBlock,true,"following");
 root.querySelector<HTMLButtonElement>("#export-stop")!.onclick=()=>{previewPlaying=false;if(player){player.pause();player.currentTime=0;}playbackBlock=-1;rerender();};
 root.querySelector<HTMLButtonElement>("#export-output")!.onclick=async()=>{
  if(preparation||confirming)return;const snapshot=settingsPayload();const withSubtitles=snapshot.subtitle_format!=="none";
  preparation=withSubtitles?"音声とVTT字幕を生成中です…":"音声を生成中です…";rerender();context.notice(preparation);
  try{
   // バックエンドは出力成功時だけ、このスナップショットをプロジェクトへ確定する。
   const output=await context.rpc<{id?:string;files?:unknown[]}>("export_blocks",snapshot);
   if(viewSession!==session)return;lastOutputId=output.id??"";savedSettings=JSON.stringify(snapshot);context.project.export_settings=structuredClone(snapshot);
   const skipped=durationSummaryKey===summaryKey(context.project)?(durationSummary?.unselected_count??0):0;
   const suffix=skipped?` ／ 未採用${skipped}件をスキップ`:"";
   context.notice((withSubtitles?`${output.files?.length??0} 組の音声・VTT字幕を書き出しました`:`${output.files?.length??0} 個の音声ファイルを書き出しました`)+suffix);
  }catch(error){if(viewSession===session)context.notice(String(error),true);}finally{if(viewSession===session){preparation="";rerender();}}
 };
 root.querySelector<HTMLButtonElement>("#export-folder")?.addEventListener("click",()=>void context.rpc("open_export_folder",{directory:lastOutputId?`exports/${lastOutputId}`:"exports"}).catch(error=>context.notice(String(error),true)));
}
