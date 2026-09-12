import type { Take, Segment, Snapshot } from "./models";
import { escapeHtml as esc } from "./html";
import {waveAmplitude} from "./waveScale";
import {bindTakeTrim,isTakeTrimDirty,discardTakeTrim,guardTakeTrimMove} from "./takeTrim";
import {reviewRows,resolveReview,type Decision} from "./scriptReview";
import {makeDraft, updateParsedLines, sourcePayload, unresolved as unresolvedSource, resolveDraft, reorder, type ScriptSource, type SourceDraft} from "./scriptSources";
import {bindRecordingArchives} from "./recordingArchives";
import {bindLegacySources} from "./legacySources";
import "./scriptWorkspace.css";
import "./takeTrim.css";
import {playCue} from "./soundEffects";
import {installReloadProtection} from "./reloadProtection";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./style.css";
import {readingTestHint} from "./readingTest";
import { exportView as modernExportView, bindExportView } from "./exportView";
import "./exportView.css";
import "./navigation.css";
import {filterFor, matchesSegment, optionsFor, renderSegmentTree, renderSourceTree, renderStageBar, resetNavigation, saveScroll, scrollFor, setCollapsed, setFilter, type Stage} from "./navigation";
const root=document.querySelector<HTMLDivElement>("#app")!;
let state:Snapshot={project:null,root:null,busy:false,job:"",message:"",peak:0,elapsed:0};
let selected="",tab="script",devices:{endpoint_id:string;friendly_name:string;is_default:boolean}[]=[],endpoint="",playingUrl="",playingTake="";
const batchSelected=new Set<string>();const listenRanges=new Map<string,{start:number;end:number}>();
const takeWaveCache=new Map<string,number[]>();
let revisionGeneration=-1, revisionText="";let wave:{duration:number;peaks:number[];suggested_gain:number;rms_db:number;peak_db:number}|null=null;
let playingOffset=0;let recordingSegment="";let startingRecording=false;let recordedBefore=new Set<string>();let uiReady=false;
let focusTake="";
let generation=-1;let busy=false;let dialogAction="";let deleteRequest="";let continuousRecording=false;let pendingRerecord="";
let navigationRenderedStage:Stage="script"; let stopExtension=0;
const drafts=new Map<string,Record<string,string>>();let renderedKey="";
const desktop="__TAURI_INTERNALS__"in window;
const value=(id:string)=>(document.getElementById(id) as HTMLInputElement)?.value??"";
const num=(id:string)=>Number(value(id));
const activeSegments=()=>state.project?.segments.filter(s=>!s.skip)??[];
const current=()=>state.project?.segments.find(s=>s.id===selected&&!s.skip);
async function rpc<T=unknown>(action:string,payload:unknown={}):Promise<T>{if(!desktop)throw new Error("閲覧プレビューです。保存・録音はデスクトップアプリで操作してください。");return invoke<T>("command",{action,payload});}
function notice(message:string,error=false){const el=document.getElementById("notice");if(el){el.textContent=message;el.classList.toggle("error",error);}}
let draftSave:Promise<void>=Promise.resolve();
async function act(action:string,payload:unknown={}){
 if(action!=="stop"&&action!=="cancel_record")await draftSave;
 if(["new","open"].includes(action)&&!await guardTakeTrimMove(()=>{}))return false;
 if(busy)return false;busy=true;
 const dialog=({new:"新しいプロジェクトの保存先を選択しています",open:"ScriptSyncプロジェクトを選択しています",markdown_import:"台本ファイルを選択しています",import_script:"台本ファイルを選択しています",talk_inspect:"Talkプロジェクトを選択しています"}as Record<string,string>)[action]??"";
 if(dialog){dialogAction=dialog;render();}
 try{if(["new","open"].includes(action))await endContinuous();await rpc(action,payload);if(action==="delete_take")discardTakeTrim((payload as {take:string}).take);await refresh(true);return true;}
 catch(e){state.message=String(e);if(["record","select","stop"].includes(action))await endContinuous();return false;}
 finally{busy=false;dialogAction="";render();}
}
function player(){return document.getElementById("player") as HTMLAudioElement;}
function clock(seconds:number){if(!Number.isFinite(seconds)||seconds<0)seconds=0;const whole=Math.floor(seconds);return `${Math.floor(whole/60)}:${String(whole%60).padStart(2,"0")}`;}
installReloadProtection(()=>{void playCue("invalid");});
function syncTakePlayback(){root.querySelectorAll<HTMLButtonElement>('[data-trim-preview]').forEach(b=>{const suffix=b.dataset.shortcut??"";b.textContent=(playingTake===b.dataset.trimPreview&&!player()?.paused?"停止":"試聴")+suffix;});}
function stopTakePlayback(take:string){if(playingTake===take&&!player()?.paused){player().pause();syncTakePlayback();return true;}return false;}
function updateTakeProgress(){if(!playingTake)return;const audio=player(),head=document.getElementById(`take-playhead-${playingTake}`);const duration=current()?.takes.find(t=>t.id===playingTake)?.duration||audio?.duration||0;if(head&&audio)head.style.left=`${duration?Math.min(100,((audio.currentTime||0)+playingOffset)/duration*100):0}%`;}
async function play(file:string,hash:string,take="",offset=0,limit?:number){try{player().pause();player().ontimeupdate=null;player().loop=false;playingTake=take;playingOffset=offset;notice("音声を読み込んでいます…");playingUrl=await rpc<string>("audio",{file,hash});player().src=playingUrl;player().ontimeupdate=()=>{if(limit!==undefined&&player().currentTime>=limit){player().pause();player().currentTime=limit;}updateTakeProgress();};player().onended=()=>{updateTakeProgress();syncTakePlayback();};await player().play();updateTakeProgress();notice("再生中");}catch(e){notice(String(e),true);}}
const statusName=(s:string)=>({ready:"録音済み",recording:"録音中",failed:"失敗",interrupted:"中断",cancelled:"取消",pending:"待機",running:"生成中",succeeded:"生成済み"}[s]??esc(s));
function recordingAsr(segment: Segment, take: Take) {
 const result = (segment.transcriptions ?? []).filter(item =>
  item.take_id === take.id || (item.file === take.file && item.audio_hash === take.hash)
 ).at(-1);
 if (!result) {
  return `<div class="recording-asr"><span>自動認識：待機</span>${button('recording_asr','認識する',`data-take="${take.id}"`)}</div>`;
 }
 const text = result.kana ?? result.raw ?? result.error ?? '';
 const status = result.status === 'stale' ? '音声変更により無効' : statusName(result.status ?? 'pending');
 return `<div class="recording-asr"><span>自動認識：${esc(status)}</span>${text ? `<p>${esc(text)}</p>` : ''}${button('recording_asr','再認識',`data-take="${take.id}"`)}</div>`;
}
function renderRecordingAsrCards(segment:Segment|undefined){if(!segment)return;segment.takes.filter(take=>take.status==='ready').forEach(take=>(document.getElementById(`memo-${take.id}`)?.closest('.take-annotation')??document.getElementById(`take-wave-${take.id}`)?.parentElement)?.insertAdjacentHTML('afterend',recordingAsr(segment,take)));}
const button=(action:string,label:string,attrs="")=>`<button data-action="${action}" ${attrs}>${label}</button>`;
function matches(s:Segment){return matchesSegment(tab as Stage,s);}
function legacyRender(){
 root.removeAttribute("aria-busy");
 if(renderedKey){const fields:Record<string,string>={};root.querySelectorAll<HTMLTextAreaElement>('textarea[id]').forEach(el=>fields[el.id]=el.value);drafts.set(renderedKey,fields);}
 const s=current();const p=state.project;
 root.innerHTML=`<header><div><span class="brand">ScriptSync <small>v0.0.1</small></span><h1>読みあわせ</h1></div><div class="toolbar">${button("new","新規プロジェクト")}${button("open","開く")}</div></header>
 <div class="statusbar"><span id="notice" role="status">${esc(state.message||"台本を録音してつなぐ。読みを確認して整える。")}</span><span id="meter"></span></div>
 <div class="layout"><aside><h2>${esc(p?.title??"プロジェクト")}</h2><p class="muted">${p?`${p.segments.filter(s=>s.selected).length} / ${p.segments.length} 発話を採用`:"作業フォルダに原稿と音声を保存します"}</p>${p?`<label>要確認フィルター<select id="filter">${[["all","すべて"],["unrecorded","未収録"],["unselected","未採用"],["unconfirmed","読み未確定"],["pending","補正保留"],["review","原稿変更後の再確認"]].map(([v,l])=>`<option value="${v}" ${v===filterFor(tab as Stage)?"selected":""}>${l}</option>`).join("")}</select></label>${button("next_pending","次の対象へ")}`:""}<nav aria-label="発話一覧">${p?.segments.map((line,i)=>matches(line)?`<button class="segment ${line.id===selected?"selected":""}" data-segment="${line.id}"><span>${String(i+1).padStart(2,"0")} ${line.skip?"対象外":line.selected?"●":"○"}</span>${esc(line.text)}</button>`:"").join("")??""}</nav>${p?`<details><summary>台本を追加</summary><label>1行を1発話として追加<textarea id="append" rows="5" placeholder="おはようございます。&#10;今日は市場へ行きます。"></textarea></label>${button("append","追加")}${button("import_script","TXT / Markdown取込")}</details>`:""}<p class="path">${esc(state.root??"")}</p></aside>
 <main>${p?`<div class="tabs flow-tabs" role="tablist">${[["recording","台本・録音"],["reading","読み確認・補正"],["evaluation","比較・採点"],["export","結合・出力"]].map(([key,label],index)=>`<button role="tab" aria-selected="${tab===key}" data-tab="${key}"><span>${index+1}</span>${label}</button>`).join("")}</div>${tab==="export"?exportView():s?segmentView(s):`<section class="empty"><h2>最初の発話を追加しましょう</h2><p>左の「台本を追加」から貼り付け、またはテキストファイルを取り込みます。</p></section>`}`:`<section class="empty"><p class="eyebrow">声から、台本を仕上げる</p><h2>一つずつ録って、<br>納得のいく読みへ。</h2><p>自分の声を発話ごとに録り直して、1本の音声に。<br>合成音声との読みあわせも、同じ原稿から始められます。</p>${button("new","新しいプロジェクトを作成",'class="primary"')}<p class="muted">録音・結合はローカルで動作します。<br>合成提案を実行すると原稿がCodexに送信されます。</p></section>`}</main></div>
 <footer><label>試聴 <audio id="player" controls></audio></label><span>録音：Ctrl + R ／ 停止：Esc</span></footer>${dialogAction?`<div id="dialog-modal" class="dialog-modal" role="dialog" aria-modal="true" aria-labelledby="dialog-modal-title"><div><strong id="dialog-modal-title">${esc(dialogAction)}</strong><p>選択画面を閉じるまで、この画面は操作できません。</p></div></div>`:""}`;
 renderedKey=`${selected}:${tab}`;for(const [id,value]of Object.entries(drafts.get(renderedKey)??{})){const field=document.getElementById(id);if(field instanceof HTMLTextAreaElement&&id!=="markdown-source")field.value=value;}
 if(s){enhance(s);editingView(s);if(tab==="reading"||tab==="evaluation")batchView(tab);comparisonTools(s);ruleView(s);}testGuide();fitWorkspace();bind();bindBatch();bindWave();updateMeter();void loadTakeWaves();if(state.busy||busy){root.setAttribute("aria-busy","true");root.querySelectorAll<HTMLButtonElement>("button[data-action]:not([data-action=stop]):not([data-action=cancel_record])").forEach(b=>b.disabled=true);root.querySelectorAll<HTMLInputElement>('input,textarea,select').forEach(field=>field.disabled=true);}
}
function fitWorkspace(){
 const main=root.querySelector('main')!;
 const recording=main.querySelector('.recording-workspace');
 main.classList.toggle('recording-main',!!recording);
 if(recording){
  const script=recording.querySelector('.recording-script')!;
  for(const child of [...main.children])if(child.matches('details'))script.append(child);
 }else{
  const content=document.createElement('div');content.className='workspace-content';
  for(const child of [...main.children])if(!child.matches('.tabs'))content.append(child);
  main.append(content);
 }
}
function segmentView(s:Segment){
 const base=`<div class="section-head"><div class="utterance-heading"><h2>発話 ${state.project!.segments.indexOf(s)+1}</h2>${button("previous_segment","← 前へ",activeSegments().indexOf(s)===0?"disabled":"")}${button("next_segment","後へ →",activeSegments().indexOf(s)===activeSegments().length-1?"disabled":"")}</div>${tab==="recording"?"":`<span class="badge">${s.confirmed?"正解読み確定":"読み未確定"}</span>`}</div>`;
 if(tab==="recording"){const recording=state.busy&&state.job==="recording";return base+`<div class="recording-workspace"><section class="recording-script"><section class="panel record-controls"><h3>録音</h3><div class="toolbar">${recording?button("cancel_record","取り消し(F1)",continuousRecording?"disabled":""):button("record","● 録音開始(F1)",`class="primary" ${continuousRecording?"disabled":""}`)}${button("stop",state.stop_scheduled?"延長収録中…":"■ 停止して保存(F2)",recording&&!continuousRecording&&!state.stop_scheduled?"":"disabled")}<label class="stop-extension">延長：<input id="stop-extension" type="number" min="0" max="60" step="0.1" value="${stopExtension}" aria-label="停止後の延長秒数"> 秒</label><label class="input-level-label">入力レベル <meter id="input-level" min="0" max="100" value="0"></meter><span id="input-level-value">0%</span></label></div><div class="live-recording"><svg id="recording-wave" viewBox="0 0 600 44" preserveAspectRatio="none" role="img" aria-label="録音中の入力波形"></svg><output id="recording-elapsed" aria-label="録音経過時間">0.0 秒</output></div></section><label>原稿<textarea id="text" class="script-input" rows="3">${esc(s.text)}</textarea></label></section><section class="take-column"><div class="section-head"><h3>テイク ${s.takes.length} 件</h3></div><div class="takes">${s.takes.map((t,i)=>`<article class="take ${s.selected===t.id?"chosen":""}"><div class="take-summary"><strong>Take ${i+1} ${s.selected===t.id?"／採用中":""}</strong><span>${t.status==="ready"?`${t.duration.toFixed(2)} 秒`:statusName(t.status)}</span><p class="take-diagnostic">${esc(qualityMessage(t.warning))}</p></div>${t.status==="ready"?`<div class="take-wave-wrap"><svg class="take-wave" id="take-wave-${t.id}" viewBox="0 0 100 24" preserveAspectRatio="none" role="img" aria-label="Take ${i+1}の音声レベル波形"></svg><i id="take-playhead-${t.id}"></i></div><div class="take-actions">${button("play","試聴",`data-file="${esc(t.file)}" data-hash="${t.hash}" data-take="${t.id}"`)}${button("select",`採用して次へ${t.id===s.takes.filter(x=>x.status==="ready").at(-1)?.id?"(F4)":""}`,`class="primary" data-id="${t.id}"`)}${button("delete_take","削除",`data-id="${t.id}"`)}</div><div class="take-annotation"><textarea class="take-memo" id="memo-${t.id}" data-take-memo="${t.id}" rows="1" aria-label="テイクメモ" placeholder="メモ">${esc(s.take_notes?.[t.id]?.memo??"")}</textarea><label class="take-score">評価<select data-take-score="${t.id}" aria-label="テイク評価"><option value="" ${s.take_notes?.[t.id]?.score==null?"selected":""}>未評価</option>${Array.from({length:11},(_,n)=>n*10).map(score=>`<option value="${score}" ${s.take_notes?.[t.id]?.score===score?"selected":""}>${score}点</option>`).join("")}</select></label></div>`:""}</article>`).join("")||`<p class="muted">まだ録音がありません。</p>`}</div></section></div>`;}
 if(tab==="reading")return base+`<p class="script">${esc(s.text)}</p><p class="muted">採用音声を聴いて、実際に発音した読みをひらがなで入力します。認識誤りや言い間違いは修正し、確認してから確定してください。</p>${s.selected?button("selected_audio","採用音声を試聴"):""}<label>正解の読み<textarea id="kana" rows="3">${esc(s.kana)}</textarea></label>${button("reference","この読みを正解として確定",'class="primary"')}<h3>原文の範囲を指定して補正</h3><p class="muted">原文欄で補正したい語を選択すると位置が入ります。同じ語も出現位置ごとに扱います。</p><textarea id="range-text" readonly rows="2">${esc(s.text)}</textarea><div class="fields"><label>開始（0始まり）<input id="start" type="number" min="0" value="0"></label><label>終了（含まない）<input id="end" type="number" min="1" value="${[...s.text].length}"></label><label>正しい読み<input id="replacement" placeholder="いちば"></label></div>${button("correction","補正候補を追加")}<div>${s.corrections.map(c=>`<article class="take"><div>${esc(c.original)} → <strong>${esc(c.replacement)}</strong><p class="muted">位置 ${c.start}–${c.end} ／ ${c.decision==="accepted"?"採用":c.decision==="rejected"?"却下":"保留"}</p></div><div>${button("accept","採用",`data-id="${c.id}"`)}${button("reject","却下",`data-id="${c.id}"`)}${button("hold","保留",`data-id="${c.id}"`)}</div></article>`).join("")}</div><p>合成後は「比較・採点」で各音声を試聴し、実際の読みとの違いを確認できます。</p>`;
 return base+`<p class="script">${esc(s.text)}</p><p>原文・全文かな・採用した局所補正を、共通の声条件で生成します。</p><p class="muted">Codexへ原稿を送信します。VoiSona Talkを起動してAPIを有効にしてください。声は既定を使用します。</p>${button("compare","3種類の比較音声を生成",'class="primary"')}${button("stop","生成をキャンセル")}${s.comparisons.slice().reverse().map(r=>`<section class="panel"><h3>比較 ${esc(r.id.slice(0,8))}</h3>${button("reveal","候補名を開示",`data-run="${r.id}"`)}<div class="comparison">${r.order.map((vid,i)=>{const v=r.variants.find(x=>x.id===vid)!;return `<article><h4>${r.revealed?esc({A:"原文",B:"全文かな",C:"局所補正",P0:"読点変更前",P1:"読点変更後"}[v.label]??v.label):`音声 ${i+1}`}</h4><p>${statusName(v.status)}</p><p class="warning">${esc(v.error)}</p>${v.status==="succeeded"?`${button("play","試聴",`data-file="${esc(v.file)}" data-hash="${v.hash}"`)}<details><summary>実際の読みを確認</summary><label>聴こえた読み<textarea id="transcript-${v.id}">${esc(v.transcript)}</textarea></label>${button("transcript","読みを保存",`data-run="${r.id}" data-id="${v.id}"`)}<p>${v.transcript?diff(s.kana,v.transcript):"音声を聴いて読みを確認してください"}</p></details><div class="ratings">${[["accuracy","読みの正確さ"],["naturalness","自然さ"],["accent","アクセント"],["rhythm","抑揚・間"],["emotion","感情"]].map(([k,l])=>`<label>${l}<select id="${k}-${v.id}"><option value="">未採点</option>${[1,2,3,4,5].map(n=>`<option ${v.rating?.[k]===n?"selected":""}>${n}</option>`).join("")}</select></label>`).join("")}</div><label>所見<textarea id="comment-${v.id}">${esc(v.rating?.comment??"")}</textarea></label>${button("rate","採点を保存",`data-run="${r.id}" data-id="${v.id}"`)}`:""}</article>`;}).join("")}</div></section>`).join("")}`;
}
function diff(a:string,b:string){const normalize=(s:string)=>[...s].filter(c=>!/[\s。、！？,.!?「」]/u.test(c)).map(c=>c>='ァ'&&c<='ヶ'?String.fromCodePoint(c.codePointAt(0)!-0x60):c).join("");return normalize(a)===normalize(b)?"確認したかな列は一致しています。アクセントは試聴で評価してください。":`読みが異なります。正解：${esc(a)} ／ 実音声：${esc(b)}`;}
function asrPanel(s:Segment,file:string,hash:string,target:string){const result=s.transcriptions?.filter(t=>t.audio_hash===hash).at(-1);return `<div class="asr">${button("asr","ローカル認識",`data-file="${esc(file)}" data-hash="${hash}"`)}${result?`<details open><summary>認識結果（要確認）</summary><p>${esc(result.error??result.raw)}</p><p>${esc(result.kana)}</p><p class="warning">${esc(result.warning)}</p>${result.kana?button("adopt_asr","読みの下書きへ",`data-asr="${result.id}" data-target="${target}"`):""}<p class="muted">時刻は認識上の推定区間です。発音の正しさを保証しません。</p>${result.words?.map(w=>button("interval",`${esc(w.text)} ${w.start.toFixed(2)}–${w.end.toFixed(2)}秒`,`data-file="${esc(file)}" data-hash="${hash}" data-start="${w.start}" data-end="${w.end}"`)).join("")??""}</details>`:""}</div>`;}
function enhance(s:Segment){if(tab==="reading"){const take=s.takes.find(t=>t.id===s.selected);if(take)document.getElementById("kana")?.parentElement?.insertAdjacentHTML("beforebegin",asrPanel(s,take.file,take.hash,"kana"));document.querySelector('[data-action="correction"]')?.insertAdjacentHTML("afterend",`${button("mapping","原文と正解読みの対応を登録")}${button("clear_mappings","対応をクリア")}<p class="muted">${s.mappings.map(m=>`${esc([...s.text].slice(m.start,m.end).join(""))}［${m.start}–${m.end}］＝${esc(m.kana)}`).join(" ／ ")}</p>`);}
 if(tab==="recording"){root.querySelectorAll('p').forEach(p=>{if(p.textContent?.includes("最大10分"))p.textContent=p.textContent.replace("最大10分","最大5分");});}
 if(tab==="evaluation"){document.querySelector('[data-action="compare"]')?.insertAdjacentHTML("afterend",button("fixed_compare","AI提案なし・標準の声で比較"));for(const r of s.comparisons){document.querySelector(`[data-action="reveal"][data-run="${r.id}"]`)?.insertAdjacentHTML("afterend",`${button("retry","同じ条件で再試行",`data-run="${r.id}"`)}${r.stale?'<p class="warning">過去の原稿・正解・補正に対する比較です。現在の内容へ結果を流用しないでください。</p>':''}`);for(const v of r.variants){const text=document.getElementById(`transcript-${v.id}`);if(text){text.parentElement?.insertAdjacentHTML("beforebegin",asrPanel(s,v.file,v.hash,`transcript-${v.id}`));text.parentElement?.insertAdjacentHTML("afterend",`${button("differences","差分区間を表示",`data-id="${v.id}" data-run="${r.id}"`)}<div id="diff-${v.id}"></div>`);}}}}
}
function exportView(){return `<h2>台本を1本の音声に</h2><p>採用したテイクを発話順に結合します。未採用の発話がある場合は、その行へ戻って選択してください。</p><p class="muted">トリムと間は各発話を選び、この画面で指定します。原音は変更しません。</p>${current()?`<section class="panel"><h3>${esc(current()!.text)}</h3><div class="fields"><label>先頭を除く（秒）<input id="trim_start" type="number" min="0" step="0.01" value="${current()!.trim_start}"></label><label>末尾を除く（秒）<input id="trim_end" type="number" min="0" step="0.01" value="${current()!.trim_end}"></label><label>後ろに足す間（秒）<input id="gap" type="number" min="0" max="60" step="0.01" value="${current()!.gap}"></label></div>${button("trim","時間設定を保存")}</section>`:""}<div class="toolbar">${button("export","完成WAVを作成",'class="primary"')}${button("report","評価レポートを出力")}</div><p id="export-result"></p><p>出力：48 kHz / ステレオ / PCM16 WAV<br>作業フォルダの exports に、音声と結合順の記録を保存します。</p>`;}
function bind(){const extension=document.getElementById("stop-extension") as HTMLInputElement|null;if(extension)extension.onchange=()=>{stopExtension=Math.max(0,Math.min(60,Number(extension.value)||0));extension.value=String(stopExtension);};root.querySelectorAll<HTMLButtonElement>("[data-segment]").forEach(b=>b.onclick=()=>{selected=b.dataset.segment!;focusTake="";render();});root.querySelectorAll<HTMLButtonElement>("[data-tab]").forEach(b=>b.onclick=async()=>{await draftSave;const before=tab as Stage;const scroll=root.querySelector<HTMLElement>("[data-navigation-scroll]");if(scroll)saveScroll(before,scroll.scrollTop);const next=b.dataset.tab as Stage;if(next!=="recording")await endContinuous();tab=next;if(["recording","reading","evaluation"].includes(next)&&current()&&!matchesSegment(next,current()!))setFilter(next,"all");render();});const f=document.getElementById("filter") as HTMLSelectElement|null;if(f)f.onchange=()=>{setFilter(tab as Stage,f.value);render();};root.querySelectorAll<HTMLButtonElement>("[data-reset-filter]").forEach(b=>b.onclick=()=>{setFilter(tab as Stage,"all");render();});root.querySelectorAll<HTMLButtonElement>("[data-source-tree-line]").forEach(b=>b.onclick=()=>{const target=document.getElementById(`source-line-${b.dataset.sourceTreeLine}`);root.querySelectorAll(".source-line.focused").forEach(row=>row.classList.remove("focused"));target?.classList.add("focused");target?.scrollIntoView?.({block:"center"});});
 const script=document.getElementById("text")as HTMLTextAreaElement|null;if(script)script.onblur=()=>{draftSave=saveScriptDraft();};const skip=document.getElementById("skip")as HTMLInputElement|null;if(skip)skip.onchange=()=>{void saveScriptDraft();};root.querySelectorAll<HTMLTextAreaElement>('[data-take-memo]').forEach(memo=>memo.onblur=()=>{void saveTakeMemo(memo);});root.querySelectorAll<HTMLSelectElement>("[data-take-score]").forEach(field=>field.onchange=()=>{void saveTakeScore(field);});
 const range=document.getElementById("range-text") as HTMLTextAreaElement|null;if(range)range.onselect=()=>{(document.getElementById("start")as HTMLInputElement).value=String([...range.value.slice(0,range.selectionStart)].length);(document.getElementById("end")as HTMLInputElement).value=String([...range.value.slice(0,range.selectionEnd)].length);};
 root.querySelectorAll<HTMLButtonElement>("[data-action]").forEach(b=>b.onclick=async()=>{const action=b.dataset.action!;const s=current();const sid={segment:s?.id};switch(action){
 case"talk_export":try{const r=await rpc<{path:string;warning:string}>(action,{...sid,run:b.dataset.run,variant:b.dataset.id});notice(`${r.path} ／ ${r.warning}`);}catch(e){notice(String(e),true);}break;
 case"talk_inspect":try{const r=await rpc<unknown>(action);if(r)document.getElementById("talk-inspection")!.textContent=JSON.stringify(r,null,2);}catch(e){notice(String(e),true);}break;
 case"punctuation":await act("compare",{...sid,punctuation:value("punctuation-text"),base_run:value("punctuation-run")});break;
 case"rule_create":await act(action,{...sid,run:b.dataset.run,dictionary:value("rule-dictionary")});break;
 case"rule_validate":await act(action,{id:b.dataset.id,run:value(`validation-${b.dataset.id}`),dictionary:value(`dictionary-${b.dataset.id}`)});break;
 case"rule_approve":case"rule_reject":await act("rule_decide",{id:b.dataset.id,decision:action==="rule_approve"?"approved":"rejected"});break;
 case"rules_export":try{const r=await rpc<{path:string}>(action);notice(`ルール保存先：${r.path}`);}catch(e){notice(String(e),true);}break;
 case"batch":{const fixed=document.getElementById("batch-fixed")as HTMLInputElement;await act(action,{kind:value("batch-kind"),segments:[...batchSelected],fixed:fixed.type==="hidden"?fixed.value!=="false":fixed.checked});break;}
 case"batch_retry":await act("batch",{retry:b.dataset.id});break;
 case"batch_all":state.project?.segments.filter(s=>matches(s)&&!s.skip).forEach(s=>batchSelected.add(s.id));render();break;
 case"batch_none":batchSelected.clear();render();break;
 case"range_play":{const hash=b.dataset.hash!,file=b.dataset.file!;const start=num(`listen-start-${hash}`),end=num(`listen-end-${hash}`);if(!Number.isFinite(start)||!Number.isFinite(end)||start<0||end<=start){notice("区間は開始より終了を後にしてください",true);break;}listenRanges.set(hash,{start,end});await play(file,hash);const audio=player();const apply=()=>{if(end>audio.duration){notice("終了時刻が音声の長さを超えています",true);audio.pause();return;}audio.currentTime=start;audio.ontimeupdate=()=>{if(audio.currentTime>=end){if((document.getElementById("listen-loop")as HTMLInputElement)?.checked)audio.currentTime=start;else audio.pause();}};};if(audio.readyState>=1)apply();else audio.addEventListener("loadedmetadata",apply,{once:true});break;}
 case"reuse_search":{const query=value("reuse-query");const examples=state.project!.segments.flatMap(other=>other.id===s?.id?[]:other.corrections.filter(c=>c.decision==="accepted"&&c.original.includes(query)).map(c=>({c,other})));document.getElementById("reuse-results")!.innerHTML=examples.map(({c,other})=>`<article><p>${esc(other.text)}</p><p>${esc(c.original)} → ${esc(c.replacement)}（位置${c.start}）</p>${button("reuse_candidate","選択範囲の補正候補へ",`data-original="${esc(c.original)}" data-reading="${esc(c.replacement)}"`)}</article>`).join("")||"該当する承認済み事例はありません";bind();break;}
 case"reuse_candidate":if([...s!.text].slice(num("start"),num("end")).join("")!==b.dataset.original){notice("原文欄で同じ語の出現位置を選んでください",true);break;}await act("correction",{...sid,start:num("start"),end:num("end"),replacement:b.dataset.reading});break;
 case"load_wave":await loadWave();break;
 case"preview_trim":case"preview_seam":try{const r=await rpc<{file:string;hash:string}>("preview_edit",{...sid,trim_start:num("trim_start"),trim_end:num("trim_end"),gap:num("gap"),gain_db:num("gain_db"),seam:action==="preview_seam"});await play(r.file,r.hash);player().loop=(document.getElementById("loop-edit")as HTMLInputElement)?.checked??false;}catch(e){notice(String(e),true);}break;
 case"gain":await act(action,{...sid,gain_db:num("gain_db")});break;
 case"suggest_gain":if(wave)(document.getElementById("gain_db")as HTMLInputElement).value=wave.suggested_gain.toFixed(1);break;
 case"split":{const index=state.project!.segments.findIndex(segment=>segment.id===s?.id);const t=document.getElementById("text")as HTMLTextAreaElement;if(await act(action,{...sid,position:[...t.value.slice(0,t.selectionStart)].length})){selected=state.project?.segments[index]?.id??selected;render();}break;}
 case"merge":{const index=state.project!.segments.findIndex(segment=>segment.id===s?.id);if(await act(action,sid)){selected=state.project?.segments[index]?.id??selected;render();}break;}
 case"restore_segment":await act(action,{segment:b.dataset.id});break;
 case"revision_preview":try{revisionText=value("revision-text");const r=await rpc<{generation:number;rows:{before:string|null;after:string|null;status:string}[]}>(action,{text:revisionText});revisionGeneration=r.generation;document.getElementById("revision-diff")!.innerHTML=`<table><thead><tr><th>変更前</th><th>変更後</th><th>扱い</th></tr></thead><tbody>${r.rows.map(r=>`<tr><td>${esc(r.before)}</td><td>${esc(r.after)}</td><td>${esc({unchanged:"保持",changed:"再確認",added:"追加",removed:"履歴へ"}[r.status])}</td></tr>`).join("")}</tbody></table>`;document.querySelector<HTMLButtonElement>('[data-action="revise_script"]')!.disabled=false;}catch(e){notice(String(e),true);}break;
 case"revise_script":if(revisionText!==value("revision-text")){notice("差分を再確認してください",true);break;}await act(action,{text:revisionText,generation:revisionGeneration});break;
 case"next_pending":navigateSegment(1,true);break;
 case"adopt_next":await adoptNext();break;
 case"previous_take":case"next_take":await cycleTake(action==="previous_take"?-1:1);break;
 case"stop":await stopRecording();break;
 case"new":case"open":case"cancel_record":case"import_script":case"report":if(action==="report"){try{const r=await rpc<{path:string}>(action);notice(`レポート保存先：${r.path}`);}catch(e){notice(String(e),true)}}else await act(action);break;
 case"append":await act(action,{text:value("append")});break;
 case"script_insert_empty":try{
  if(!sourceOrder.length)addEmptyScriptSource();
  await flushSourceDraft();
  const previous=new Set(state.project?.segments.map(row=>row.id));
  await rpc('script_insert_empty',{project_id:(state.project as any)?.id,base_revision:sourceRevision,base_generation:sourceGeneration,source_id:(s as any)?.source_id??selectedSource,...(s?{after_segment:s.id}:{})});
  await refresh(true);selected=state.project?.segments.find(row=>!previous.has(row.id))?.id??selected;render();
 }catch(e){notice(String(e),true);}break;
 case"recording_asr":try{await rpc('automatic_asr',{segment:selected,take:b.dataset.take});}catch(e){notice(String(e),true);}break;
 case"record":await startRecording();break;
 case"compare":player().pause();await act(action,sid);break;
 case"select":await adoptTake(b.dataset.id!);break;
 case"delete_take":showDeleteModal(b.dataset.id!);break;
 case"play":if(b.dataset.take)focusTake=b.dataset.take;await play(b.dataset.file!,b.dataset.hash!,b.dataset.take);break;
 case"selected_audio":{const t=s?.takes.find(t=>t.id===s.selected);if(t)await playSavedTake(t);break;}
 case"edit":await act(action,{...sid,text:value("text"),skip:(document.getElementById("skip")as HTMLInputElement).checked,trim_start:s?.trim_start,trim_end:s?.trim_end,gap:s?.gap});break;
 case"trim":await act("edit",{...sid,text:s?.text,skip:s?.skip,trim_start:num("trim_start"),trim_end:num("trim_end"),gap:num("gap")});break;
 case"previous_segment":case"next_segment":navigateSegment(action==="previous_segment"?-1:1);break;
 case"reference":await act(action,{...sid,kana:value("kana"),mappings:s?.mappings??[]});break;
 case"correction":await act(action,{...sid,start:num("start"),end:num("end"),replacement:value("replacement")});break;
 case"mapping":await act(action,{...sid,start:num("start"),end:num("end"),replacement:value("replacement")});break;
 case"clear_mappings":await act(action,sid);break;
 case"fixed_compare":await act("compare",{...sid,fixed:true});break;
 case"retry":{const r=s?.comparisons.find(r=>r.id===b.dataset.run);await act("compare",{...sid,retry:b.dataset.run,...(r?.kind==="punctuation"?{punctuation:r.variants.find(v=>v.label==="P1")?.text}:{})});break;}
 case"asr":player().pause();await act(action,{...sid,file:b.dataset.file,hash:b.dataset.hash});break;
 case"adopt_asr":{const result=s?.transcriptions?.find(t=>t.id===b.dataset.asr);const field=document.getElementById(b.dataset.target!) as HTMLTextAreaElement;if(field&&result?.kana)field.value=result.kana;notice("候補を入力しました。音声を聴いて修正・保存してください。");break;}
 case"interval":await play(b.dataset.file!,b.dataset.hash!);player().currentTime=Number(b.dataset.start);player().ontimeupdate=()=>{if(player().currentTime>=Number(b.dataset.end)){player().pause();player().ontimeupdate=null;player().loop=false;}};break;
 case"differences":try{const run=s?.comparisons.find(r=>r.id===b.dataset.run);const rows=await rpc<{expected:string;observed:string;expected_start:number;expected_end:number;observed_start:number;observed_end:number;ambiguous:boolean;source_candidates?:{start:number;end:number;original:string;kana:string;ambiguous:boolean}[]}[]>("diff",{segment:run?.stale?null:selected,expected:run?.reference_kana||s?.kana,observed:value(`transcript-${b.dataset.id}`)});document.getElementById(`diff-${b.dataset.id}`)!.innerHTML=rows.map(d=>`<p>正解［${d.expected_start}–${d.expected_end}］${esc(d.expected)||"∅"} → 音声［${d.observed_start}–${d.observed_end}］${esc(d.observed)||"∅"}<br>${d.ambiguous?"対応が曖昧です。要聴取・保留。":"原文対応と音声を確認して補正してください。"}</p>${d.source_candidates?.map(c=>`<p>原文候補［${c.start}–${c.end}］${esc(c.original)} → ${esc(c.kana)}${c.ambiguous?'（複数対応・要確認）':''}</p>`).join('')||'<p>原文位置は未確定です。「読み確認・補正」で対応を登録してください。</p>'}`).join("")||"かな列は一致しています。";}catch(e){notice(String(e),true)}break;
 case"accept":case"reject":case"hold":await act("decide",{...sid,id:b.dataset.id,decision:{accept:"accepted",reject:"rejected",hold:"pending"}[action]});break;
 case"reveal":await act(action,{...sid,run:b.dataset.run});break;
 case"transcript":await act(action,{...sid,run:b.dataset.run,variant:b.dataset.id,text:value(`transcript-${b.dataset.id}`)});break;
 case"rate":{const rating:Record<string,unknown>={comment:value(`comment-${b.dataset.id}`),created:Math.floor(Date.now()/1000)};for(const key of ["accuracy","naturalness","accent","rhythm","emotion"]){rating[key]=value(`${key}-${b.dataset.id}`)?num(`${key}-${b.dataset.id}`):null;}await act(action,{...sid,run:b.dataset.run,variant:b.dataset.id,rating});break;}
 case"export":try{notice("結合しています…");const r=await rpc<{file:string;hash:string;clipped_samples:number}>(action);notice(`保存しました：${r.file}${r.clipped_samples?" ／ 音量超過あり":""}`);await play(r.file,r.hash);}catch(e){notice(String(e),true)}break;
 }});
}
function updateMeter(){updateRecordingWave();const db=20*Math.log10(Math.max(state.peak,0.00001));const level=Math.round(Math.max(0,Math.min(100,(db+60)/60*100)));const m=document.getElementById("meter");if(m)m.textContent=state.busy?`${state.job==="recording"?`${(state.elapsed/1000).toFixed(1)} 秒 ／ Peak ${db.toFixed(0)} dB`:"処理中"}`:"自動保存";const bar=document.getElementById("input-level")as HTMLMeterElement|null;if(bar)bar.value=state.job==="recording"?level:0;const label=document.getElementById("input-level-value");if(label)label.textContent=`${state.job==="recording"?level:0}%`;}
async function saveScriptDraft(){const s=current();const text=document.getElementById("text")as HTMLTextAreaElement|null;const skip=document.getElementById("skip")as HTMLInputElement|null;if(!s||!text||(text.value===s.text&&(!skip||skip.checked===s.skip)))return;try{await rpc("edit",{segment:s.id,text:text.value,skip:skip?.checked??s.skip,trim_start:s.trim_start,trim_end:s.trim_end,gap:s.gap});s.text=text.value;drafts.delete(`${s.id}:${tab}`);}catch(e){notice(String(e),true);}}
async function saveTakeScore(field:HTMLSelectElement){const s=current(),take=field.dataset.takeScore;if(!s||!take)return;const previous=s.take_notes?.[take]?.score??null,score=field.value===""?null:Number(field.value);field.disabled=true;try{await rpc("take_score",{segment:s.id,take,score});s.take_notes??={};s.take_notes[take]={...(s.take_notes[take]??{memo:"",favorite:false}),score};state.project!.generation++;generation=state.project!.generation;}catch(e){field.value=previous==null?"":String(previous);notice(String(e),true);}finally{field.disabled=state.busy||busy;}}
async function saveTakeMemo(field:HTMLTextAreaElement){const s=current();const take=field.dataset.takeMemo;if(!s||!take)return;const previous=s.take_notes?.[take]?.memo??"";if(field.value===previous)return;try{await rpc("take_note",{segment:s.id,take,memo:field.value,favorite:false});s.take_notes??={};s.take_notes[take]={...s.take_notes[take],memo:field.value,favorite:false};state.project!.generation++;generation=state.project!.generation;notice("テイクメモを保存しました");}catch(e){notice(String(e),true);}}
const takeWaveLoading=new Set<string>();
function drawTakeWave(id:string,peaks:number[]){const svg=document.getElementById(`take-wave-${id}`);if(!svg||!peaks.length)return;svg.innerHTML=peaks.map((peak,index)=>{const height=waveAmplitude(peak)*11;if(!height)return "";return `<line x1="${index+.5}" x2="${index+.5}" y1="${12-height}" y2="${12+height}"/>`;}).join("");}
async function loadTakeWaves(){if(tab!=="recording")return;const s=current();if(!s)return;await Promise.all(s.takes.filter(t=>t.status==="ready").map(async take=>{const cached=takeWaveCache.get(take.hash);if(cached){drawTakeWave(take.id,cached);return;}if(takeWaveLoading.has(take.hash))return;takeWaveLoading.add(take.hash);try{const wave=await rpc<{peaks:number[]}>("waveform",{file:take.file,hash:take.hash});if(!wave?.peaks?.length)return;const peaks=Array.from({length:100},(_,index)=>{const from=Math.floor(index*wave.peaks.length/100),to=Math.max(from+1,Math.floor((index+1)*wave.peaks.length/100));return Math.max(...wave.peaks.slice(from,to));});takeWaveCache.set(take.hash,peaks);if(selected===s.id)drawTakeWave(take.id,peaks);}catch{}finally{takeWaveLoading.delete(take.hash);}}));}
let refreshing=false;
async function refresh(force=false){if(refreshing||(busy&&!force))return;refreshing=true;try{const next=await rpc<Snapshot>("state");const needs=force||(next.project?.generation??-1)!==generation||next.root!==state.root||next.busy!==state.busy||next.job!==state.job||next.stop_scheduled!==state.stop_scheduled;if(state.busy&&!next.busy&&recordingSegment===selected){focusTake=next.project?.segments.find(s=>s.id===selected)?.takes.filter(t=>t.status==="ready").at(-1)?.id??"";if(focusTake&&!recordedBefore.has(focusTake)){await playCue("saved");recordedBefore.add(focusTake);}}state=next;if(!current())selected=activeSegments()[0]?.id??"";generation=state.project?.generation??-1;if(needs)render();else{updateMeter();if(state.busy)notice(state.message);}}catch(e){notice(String(e),true)}finally{refreshing=false;}if(pendingRerecord&&!state.busy&&!busy&&continuousRecording){const sid=pendingRerecord;pendingRerecord="";if(selected===sid)await startRecording();}}
async function loadDevices(){try{devices=await rpc("devices");endpoint=devices.find(d=>d.is_default)?.endpoint_id??devices[0]?.endpoint_id??"";}catch(e){notice(String(e),true)}}
document.addEventListener("keydown",e=>{if(e.repeat||document.querySelector("[role=dialog]"))return;if(e.key==="Escape"&&state.busy){player()?.pause();void act("stop");}});
document.addEventListener("keydown",e=>{if(e.repeat||document.querySelector("[role=dialog]")||e.target instanceof HTMLInputElement||e.target instanceof HTMLTextAreaElement||e.target instanceof HTMLSelectElement||(e.target as HTMLElement)?.isContentEditable)return;if(e.ctrlKey&&e.key==="Enter"){e.preventDefault();void adoptNext();}else if(e.altKey&&["ArrowLeft","ArrowRight"].includes(e.key)){e.preventDefault();void cycleTake(e.key==="ArrowLeft"?-1:1);}else if(e.code==="Space"){e.preventDefault();if(!state.busy){if(player()&&!player().paused)player().pause();else if(player())void cycleTake(0);}}});
function navigateSegment(delta:number,onlyMatches=false){if(state.busy||busy)return;const list=activeSegments();const index=list.findIndex(s=>s.id===selected);for(let i=index+delta;i>=0&&i<list.length;i+=delta){const s=list[i];if(!onlyMatches||matches(s)){selected=s.id;focusTake="";render();return;}}}
async function stopRecording(){
 if(state.job==="recording"){
  if(state.stop_scheduled){void playCue("invalid");return;}
  await act("stop",stopExtension?{delay_seconds:stopExtension}:{});
 }else await act("stop");
}
const keyOwner=window as Window&{recordingKeys?:AbortController};
keyOwner.recordingKeys?.abort();keyOwner.recordingKeys=new AbortController();
document.addEventListener("keydown",e=>{
 if(continuousRecording||tab!=="recording"||!desktop||!/^F[1-4]$/.test(e.key)||e.ctrlKey||e.altKey||e.metaKey)return;
 e.preventDefault();if(!e.repeat)void continuousKey(e.key,true);
},{signal:keyOwner.recordingKeys.signal});
async function startRecording(){
 await draftSave;if(state.busy||busy||startingRecording||!current())return;
 startingRecording=true;busy=true;const sid=selected;
 try{
  player()?.pause();await loadDevices();
  if(!endpoint){await endContinuous();notice('Windowsの既定入力マイクが見つかりません',true);return;}
  await playCue('start');
  if(selected!==sid)return;
  recordingSegment=sid;recordedBefore=new Set(current()?.takes.filter(t=>t.status==='ready').map(t=>t.id));
  busy=false;await act('record',{segment:sid,endpoint});
 }finally{startingRecording=false;busy=false;render();}
}
async function cycleTake(delta:number){if(state.busy)return;const s=current();const ts=s?.takes.filter(t=>t.status==="ready")??[];if(!ts.length)return;let index=ts.findIndex(t=>t.id===(focusTake||s?.selected));if(index<0)index=ts.length-1;const t=ts[(index+delta+ts.length)%ts.length];focusTake=t.id;await playSavedTake(t);notice(`Take ${s!.takes.indexOf(t)+1} を試聴中`);}
async function adoptTake(take:string,qualityConfirmed=false){if(isTakeTrimDirty(take)){void playCue("invalid");notice("トリミングを保存してから採用してください",true);return;}if(state.busy||busy)return;const s=current();if(!s)return;const warning=s.takes.find(t=>t.id===take)?.warning;if(!qualityConfirmed&&warning?.startsWith("要確認：")){showQualityConfirmation(take,warning);return;}if(await act("select",{segment:s.id,take})){await playCue("next");const list=activeSegments(),index=list.findIndex(x=>x.id===s.id);if(index===list.length-1){await endContinuous();tab="reading";render();}else{selected=list[index+1].id;focusTake="";render();if(continuousRecording)await startRecording();}}}
async function adoptNext(){const s=current();const t=s?.takes.find(t=>t.id===focusTake&&t.status==="ready")??s?.takes.filter(t=>t.status==="ready").at(-1);if(t)await adoptTake(t.id);}
function ruleView(s:Segment){if(tab!=="evaluation")return;
for(const r of s.comparisons){for(const v of r.variants.filter(v=>v.status==="succeeded")){document.querySelector(`[data-action="rate"][data-id="${v.id}"]`)?.insertAdjacentHTML("afterend",button("talk_export","Talk編集ファイルを出力（試験対応）",`data-run="${r.id}" data-id="${v.id}"`));}}
document.querySelector("main")?.insertAdjacentHTML("beforeend",`<details><summary>Talk保存ファイルを再読込して確認</summary><p>田中さん・ハスキー0の生成音声に対応。Talk 1.3.8.2・田中さん2.0.1の既定条件で編集・保存・再起動・再合成を確認済みです。他の版・非既定パラメーターは未検証です。</p>${button("talk_inspect","tstprjを選んで内容を確認")}<pre id="talk-inspection"></pre></details>`);
const runs=s.comparisons.filter(r=>r.kind!=="punctuation"&&!r.stale&&r.variants.some(v=>v.label==="C"&&v.status==="succeeded"));
const latest=runs.at(-1);const base=latest?.variants.find(v=>v.label==="C")?.text??s.text;
document.querySelector('[data-action="compare"]')?.insertAdjacentHTML("beforebegin",`<details><summary>読点だけを変えて比較</summary><label>基準比較<select id="punctuation-run">${runs.slice().reverse().map(r=>`<option value="${r.id}">${esc(r.id.slice(0,8))}</option>`).join("")}</select></label><label>局所補正済み原稿の読点「、」だけを変更<textarea id="punctuation-text">${esc(base)}</textarea></label>${button("punctuation","同じ声で読点比較を生成")}</details>`);
document.querySelector("main")?.insertAdjacentHTML("beforeend",`<details><summary>評価から読みルールを作成</summary><p>補正1件だけの比較で、読みが改善し、自然さが低下しない事例を候補にします（補正後の正確さ・自然さは各4以上）。承認には候補作成後の別の文での検証が必要です。</p><label>この比較時の辞書条件（利用者が確認）<input id="rule-dictionary" placeholder="例：個人辞書なし、2026-09-08から変更なし"></label>${runs.map(r=>button("rule_create",`比較 ${esc(r.id.slice(0,8))} から候補を作成`,`data-run="${r.id}"`)).join("")}${state.project?.rules?.map(r=>`<article class="panel"><h3>${esc(r.original)} → ${esc(r.reading)}</h3><p>${esc(r.context)}</p><p>状態：${esc({draft:"候補",approved:"承認済み",rejected:"却下"}[r.status])} ／ 検証 ${r.validations.length}件</p><p>辞書条件：${esc(r.dictionary)}</p><label>別の文の検証比較<select id="validation-${r.id}">${state.project!.segments.flatMap(s=>s.comparisons.filter(x=>x.kind!=="punctuation"&&!x.stale&&x.id!==r.source.id).map(x=>`<option value="${x.id}">${esc(s.text)} ／ ${esc(x.id.slice(0,8))}</option>`)).join("")}</select></label><label>検証時に確認した辞書条件<input id="dictionary-${r.id}"></label>${button("rule_validate","検証結果を登録",`data-id="${r.id}"`)}${button("rule_approve","ルールを承認",`data-id="${r.id}"`)}${button("rule_reject","ルールを却下",`data-id="${r.id}"`)}</article>`).join("")??""}${button("rules_export","承認済みルールをプロンプト用に出力")}</details>`);}
function batchView(stage:"reading"|"evaluation"){const p=state.project!,kind=stage==="reading"?"asr":"compare",title=stage==="reading"?"採用音声を一括認識":"比較音声を一括生成";document.querySelector("#stage-extras")?.insertAdjacentHTML("beforeend",`<details><summary>${title}</summary>${button("batch_all","表示中の発話を選択")}${button("batch_none","選択解除")}<div class="batch-list">${p.segments.filter(s=>!s.skip).map(s=>`<label class="inline"><input type="checkbox" data-batch-segment="${s.id}" ${batchSelected.has(s.id)?"checked":""}>${esc(s.text)}</label>`).join("")}</div><input id="batch-kind" type="hidden" value="${kind}">${kind==="compare"?`<label class="inline"><input id="batch-fixed" type="checkbox" checked>AI提案なし・標準の声</label><p class="muted">チェックを外した合成は原稿をCodexへ送信します。結果は個別に確認・採用してください。</p>`:`<input id="batch-fixed" type="hidden" value="true">`}${button("batch","選択した発話を処理")}${button("stop","一括処理を停止")}${p.batches?.slice().reverse().map(b=>`<section><strong>${b.items.filter(i=>i.status==="succeeded").length}/${b.items.length} 完了</strong>${b.items.map(i=>`<p>${esc(p.segments.find(s=>s.id===i.payload.segment)?.text??"旧発話")}：${statusName(i.status)} ${esc(i.error)}</p>`).join("")}${button("batch_retry","失敗・中断分だけ再試行",`data-id="${b.id}"`)}</section>`).join("")??""}</details>`);}
function bindBatch(){document.querySelectorAll<HTMLInputElement>('[data-batch-segment]').forEach(el=>el.onchange=()=>{if(el.checked)batchSelected.add(el.dataset.batchSegment!);else batchSelected.delete(el.dataset.batchSegment!);});}
function comparisonTools(s:Segment){if(tab==="reading"){document.querySelector("main")?.insertAdjacentHTML("beforeend",`<details><summary>承認済みの補正事例を検索</summary><label>語を検索<input id="reuse-query"></label>${button("reuse_search","事例を検索")}<p class="muted">文脈と現在の音声を確認し、原文の出現位置を選んで候補へ追加します。</p><div id="reuse-results"></div></details>`);}if(tab!=="evaluation")return;const t=s.takes.find(t=>t.id===s.selected);const run=s.comparisons.filter(r=>r.kind!=="punctuation"&&!r.stale).at(-1);const voices=[...(t?[{name:"人間音声",file:t.file,hash:t.hash}]:[]),...(run?run.order.map((id,i)=>{const v=run.variants.find(v=>v.id===id)!;return {name:run.revealed?v.label:`音声 ${i+1}`,file:v.file,hash:v.hash};}).filter(v=>v.hash):[])];document.querySelector('[data-action="compare"]')?.insertAdjacentHTML("beforebegin",`<details><summary>該当箇所を並べて比較試聴</summary><p class="muted">前後の文脈を含め、各音声に対応する区間を秒で指定してください。長さが違うため時刻は個別に調整します。</p><label class="inline"><input id="listen-loop" type="checkbox">区間を反復</label>${voices.map(v=>{const r=listenRanges.get(v.hash)??{start:0,end:1};return `<div class="range-row"><strong>${esc(v.name)}</strong><label>開始<input id="listen-start-${v.hash}" type="number" min="0" step="0.01" value="${r.start}"></label><label>終了<input id="listen-end-${v.hash}" type="number" min="0" step="0.01" value="${r.end}"></label>${button("range_play","この区間を試聴",`data-file="${esc(v.file)}" data-hash="${v.hash}"`)}</div>`;}).join("")}</details>`);}
function editingView(s:Segment){
 if(tab==="recording"){document.querySelector('.recording-script .compact-actions')?.insertAdjacentHTML("afterend",`<div class="toolbar compact-actions">${button("split","カーソル位置で分割")}${button("merge","次と結合")}</div>`);document.querySelector("aside nav")?.insertAdjacentHTML("afterend",`<details class="script-revision"><summary>台本全体の編集・履歴</summary><label>差し替える台本（1行1発話）<textarea id="revision-text" rows="6">${esc(state.project!.segments.map(s=>s.text).join("\n"))}</textarea></label>${button("revision_preview","差分を確認")}${button("revise_script","改訂を適用","disabled")}<div id="revision-diff"></div><h3>変更前の発話</h3>${state.project!.archived_segments?.map(s=>`<p>${esc(s.text)} ${button("restore_segment","末尾へ復元",`data-id="${s.id}"`)}</p>`).join("")??""}</details>`);}
 if(tab==="export"){document.querySelector('[data-action="trim"]')?.insertAdjacentHTML("afterend",`<section class="wave-editor"><h3>波形・音量</h3>${button("load_wave","採用音声の波形・音量を表示")}<p id="wave-stats"></p><svg id="wave" viewBox="0 0 800 160" role="img" aria-label="音声波形。ドラッグで残す区間を選択"></svg><p class="muted">波形上をドラッグして残す区間を指定。秒数入力でも調整できます。</p><label>完成音声の音量調整（dB）<input id="gain_db" type="number" min="-36" max="24" step="0.1" value="${s.gain_db??0}"></label>${button("suggest_gain","RMS -20 dBFSを目安に調整案を入力")}${button("gain","音量設定を保存")}<p class="muted">調整案はピークを-1 dBFS以下に抑える値です。原音・読みの比較音声には適用しません。</p><label class="inline"><input id="loop-edit" type="checkbox">繰り返し試聴</label>${button("preview_trim","現在のトリム・音量を試聴")}${button("preview_seam","次の発話とのつなぎ目を試聴")}</section>`);wave=null;}
}
async function loadWave(){const t=current()?.takes.find(t=>t.id===current()?.selected);if(!t)return notice("採用テイクを選んでください",true);const sid=selected;try{const result=await rpc<NonNullable<typeof wave>>("waveform",{file:t.file,hash:t.hash});if(sid!==selected||tab!=="export")return;wave=result;document.getElementById("wave-stats")!.textContent=`${wave.duration.toFixed(2)} 秒 ／ RMS ${wave.rms_db.toFixed(1)} dBFS ／ Peak ${wave.peak_db.toFixed(1)} dBFS`;drawWave();}catch(e){notice(String(e),true);}}
function drawWave(){const svg=document.getElementById("wave");if(!svg||!wave)return;const w=wave;const a=num("trim_start")/w.duration*800,b=(w.duration-num("trim_end"))/w.duration*800;svg.innerHTML=`<rect x="${a}" width="${Math.max(0,b-a)}" y="0" height="160" fill="#b9dab9"/><path d="${w.peaks.map((p,i)=>`M${i/w.peaks.length*800},${80-p*75}v${p*150}`).join(" ")}" stroke="#304c44" stroke-width="1"/>`;}
function bindWave(){if(player()){player().onplay=syncTakePlayback;player().onpause=syncTakePlayback;player().addEventListener("ended",syncTakePlayback,{once:true});}syncTakePlayback();const svg=document.getElementById("wave");if(!svg)return;let start=0;const pos=(e:PointerEvent)=>Math.max(0,Math.min(1,(e.clientX-svg.getBoundingClientRect().left)/svg.getBoundingClientRect().width));svg.onpointerdown=e=>{if(!wave)return;start=pos(e);svg.setPointerCapture(e.pointerId);};svg.onpointerup=e=>{if(!wave||!svg.hasPointerCapture(e.pointerId))return;svg.releasePointerCapture(e.pointerId);const end=pos(e);if(Math.abs(end-start)<0.002)return;(document.getElementById("trim_start")as HTMLInputElement).value=(Math.min(start,end)*wave.duration).toFixed(3);(document.getElementById("trim_end")as HTMLInputElement).value=((1-Math.max(start,end))*wave.duration).toFixed(3);drawWave();};for(const id of ["trim_start","trim_end"]){document.getElementById(id)?.addEventListener("input",drawWave);}}
function legacyEnsureScriptDraft(){
 if(!markdownDraft||markdownDraft.root!==state.root){
  const p=state.project as any;
  markdownDraft={root:state.root,text:p?.source_markdown??'',lines:p?.script_lines??[]};
  scriptBase=p?.source_markdown??'';scriptBaseGeneration=p?.generation??0;
  scriptDecisions.clear();scriptChoices.clear();scriptPreviewVersion++;scriptPreviewPending=false;scriptPreviewError='';
  scriptResolved={text:scriptBase,origins:Object.fromEntries((p?.script_lines??[]).map((r:any)=>[r.line,r.line]))};
 }
 else if(scriptBase===(state.project as any)?.source_markdown){scriptBaseGeneration=state.project?.generation??0;}
}
function legacyScriptTab(){
 ensureScriptDraft();
 return `<section class="script-tab script-workspace"><div class="script-heading"><h2>台本</h2></div><div class="script-columns" style="grid-template-columns:${scriptWidths[0]}fr 8px ${scriptWidths[1]}fr 8px ${scriptWidths[2]}fr"><section class="script-editor"><h3>読み込む・編集する</h3><div class="toolbar script-import">${button('markdown_import','Markdownを読み込む')}<label>読込方法<select id="script-import-mode"><option value="replace" ${scriptImportMode==='replace'?'selected':''}>差分として読み込む</option><option value="append" ${scriptImportMode==='append'?'selected':''}>末尾に追加</option></select></label></div><p class="muted">Markdown / TXTをここにドロップ。複数は名前順に結合します。</p><div class="script-drop-hint">ここに原稿を読み込みます</div><textarea id="markdown-source" aria-label="編集する台本" spellcheck="false">${esc(markdownDraft!.text)}</textarea></section>${scriptSeparator(0)}<section class="script-review"><h3>差分を確認する</h3><div id="script-difference" class="script-difference" aria-live="polite">${scriptDiffHtml()}</div></section>${scriptSeparator(1)}<section class="script-targets"><h3>読み上げ対象を選ぶ</h3><div id="script-target-list" class="source-list">${scriptTargetHtml()}</div></section></div><div class="script-bottom"><span id="script-review-status">${scriptStatus()}</span>${button('markdown_commit','台本を確定',`class="primary" ${scriptBlocked()?'disabled':''}`)}</div></section>`;
}
let scriptImportMode='append';
let legacyScriptWidths=[1,1,1];
let scriptBase='',scriptBaseGeneration=0,scriptPreviewVersion=0,scriptPreviewPending=false,scriptPreviewError='';
let scriptTimer:ReturnType<typeof setTimeout>|undefined;
let scriptResolved:{text:string;origins:Record<number,number>}={text:'',origins:{}};
const scriptDecisions=new Map<string,Decision>(),scriptChoices=new Map<string,boolean>();
let markdownDraft:{root:string|null;text:string;lines:any[]}|null=null;
function legacyCurrentReview(){return reviewRows(scriptBase,markdownDraft?.text??'');}
function legacyUnresolvedScript(){return legacyCurrentReview().filter(r=>r.kind!=='same'&&!scriptDecisions.has(r.key)).length;}
function legacyScriptBlocked(){return scriptPreviewPending||!!scriptPreviewError||legacyUnresolvedScript()>0||scriptBaseGeneration!==state.project?.generation;}
function legacyScriptStatus(){return scriptBaseGeneration!==state.project?.generation?'保存済み台本が更新されました。プロジェクトを開き直してください。':scriptPreviewError||`未確認 ${legacyUnresolvedScript()}件${scriptPreviewPending?' ／ 読み上げ対象を更新中':' ／ 承認した変更だけを確定します'}`;}
function legacyScriptSeparator(index:number){return `<div class="script-separator" role="separator" tabindex="0" aria-label="${index+1}番目のペイン境界" aria-orientation="vertical" data-separator="${index}" aria-valuemin="15" aria-valuemax="85" aria-valuenow="50"></div>`;}
function legacyScriptDiffHtml(){
 const rows=legacyCurrentReview(),changed=rows.filter(r=>r.kind!=='same');
 return `<p class="diff-count">追加 ${changed.filter(r=>r.kind==='added').length}行 ／ 削除 ${changed.filter(r=>r.kind==='removed').length}行</p>${changed.length?rows.map((r,index)=>r.kind==='same'?'':`<div class="diff-row diff-${r.kind}"><b aria-label="${r.kind==='added'?'追加':'削除'}">${r.kind==='added'?'＋':'−'}</b><span class="diff-text">${esc(r.text)}</span><span class="diff-decisions">${(['accept','reject'] as const).map(choice=>`<button data-review-index="${index}" data-decision="${choice}" aria-pressed="${scriptDecisions.get(r.key)===choice}">${choice==='accept'?'承認':'却下'}</button>`).join('')}</span></div>`).join(''):'<p class="muted">変更はありません（空行の増減は無視します）。</p>'}`;
}
function legacyScriptTargetHtml(){return (markdownDraft?.lines??[]).filter(x=>String(x.text??'').trim()).map(x=>`<button id="source-line-${x.line}" class="source-line ${x.enabled?'enabled':'disabled'}" aria-pressed="${x.enabled}" data-script-line="${x.line}"><span>${x.enabled?'☑':'☐'}</span><b>${['本文','部','章','節','項'][x.kind]??'本文'}</b><em>${esc(x.text||x.raw)}</em></button>`).join('');}
function legacyUpdateScriptPanels(){
 if(tab!=='script')return;
 const diff=document.getElementById('script-difference');if(diff)diff.innerHTML=legacyScriptDiffHtml();
 const targets=document.getElementById('script-target-list');if(targets)targets.innerHTML=legacyScriptTargetHtml();
 const status=document.getElementById('script-review-status');if(status)status.textContent=legacyScriptStatus();
 const commit=root.querySelector<HTMLButtonElement>('[data-action=markdown_commit]');if(commit)commit.disabled=busy||state.busy||legacyScriptBlocked();
 legacyBindScriptDecisions();
}
function choiceKey(row:any,origins=scriptResolved.origins){return origins[row.line]!==undefined?`old:${origins[row.line]}`:`new:${row.line}:${row.raw}`;}
function scheduleScriptPreview(){
 clearTimeout(scriptTimer);scriptPreviewVersion++;scriptPreviewPending=true;scriptPreviewError='';updateScriptPanels();
 scriptTimer=setTimeout(()=>void previewScript(),120);
}
async function previewScript(){
 clearTimeout(scriptTimer);const version=++scriptPreviewVersion,draft=markdownDraft;if(!draft)return;
 scriptPreviewPending=true;scriptPreviewError='';
 const result=resolveReview(scriptBase,draft.text,legacyCurrentReview(),scriptDecisions);
 try{
  const parsed=await rpc<{lines:any[]}>('markdown_preview',{text:result.text});
  if(version!==scriptPreviewVersion||draft!==markdownDraft)return;
  const originals=(state.project as any)?.script_lines??[];
  draft.lines=parsed.lines.map(row=>({...row,enabled:scriptChoices.get(choiceKey(row,result.origins))??originals.find((r:any)=>r.line===result.origins[row.line])?.enabled??row.enabled}));
  scriptResolved=result;
 }catch(e){if(version===scriptPreviewVersion)scriptPreviewError=String(e);}
 finally{if(version===scriptPreviewVersion){scriptPreviewPending=false;updateScriptPanels();}}
}
async function acceptMarkdown(result:{text:string;lines:any[]}){
 ensureScriptDraft();
 markdownDraft!.text=scriptImportMode==='append'&&markdownDraft!.text?markdownDraft!.text+'\n\n'+result.text:result.text;
 const keys=new Set(legacyCurrentReview().map(r=>r.key));for(const key of scriptDecisions.keys())if(!keys.has(key))scriptDecisions.delete(key);
 await previewScript();
}
function setScriptDrag(active:boolean){root.querySelector('.script-editor')?.classList.toggle('drag-over',active&&tab==='script'&&!busy&&!state.busy);}
async function loadDroppedMarkdown(paths:string[]){
 if(tab!=="script"||!state.project){notice("プロジェクトを開き、台本画面でドロップしてください",true);return;}
 if(busy||state.busy||dialogAction){notice("処理が終わってからドロップしてください",true);return;}
 const projectRoot=state.root;
 busy=true;
 try{
  const result=await rpc<{text:string;lines:any[];names:string[];files?:{name:string;text:string}[]}>("markdown_drop",{paths});
  if(state.root!==projectRoot)return;
  if(hasSourceMode())await applyRegisteredFiles(result.files??[{name:result.names?.[0]??'原稿.md',text:result.text}]);else await acceptMarkdown(result);
  drafts.delete(`${selected}:script`);
  state.message=`読込順：${result.names.join(" → ")}。台本はまだ確定していません。`;
 }catch(e){state.message=String(e);}finally{busy=false;render();}
}
if(desktop){void listen("tauri://drag-enter",()=>setScriptDrag(true));void listen("tauri://drag-leave",()=>setScriptDrag(false));void listen<{paths:string[]}>("tauri://drag-drop",event=>{setScriptDrag(false);void loadDroppedMarkdown(event.payload.paths);}).catch(e=>notice(String(e),true));}
const collapsed=new Set<string>();
function scriptTree(segments:Segment[],depth=0):string{
 if(depth>=3)return segments.map(line=>`<button class="segment ${line.id===selected?"selected":""}" data-segment="${line.id}"><span>${String(state.project!.segments.indexOf(line)+1).padStart(4,"0")}</span>${esc(line.text)}</button>`).join("");
 let html="";for(let i=0;i<segments.length;){const n=segments[i].hierarchy?.[depth]??0;if(!n){html+=scriptTree([segments[i++]],depth+1);continue;}let end=i+1;while(end<segments.length&&segments[end].hierarchy?.[depth]===n)end++;const key=(segments[i].hierarchy??[]).slice(0,depth+1).join("-");html+=`<details class="tree-group" data-tree="${key}" ${collapsed.has(key)?"":"open"}><summary>第${n}${["部","章","節"][depth]}</summary>${scriptTree(segments.slice(i,end),depth+1)}</details>`;i=end;}return html;
}
function render(){
 const previousNavigation=root.querySelector<HTMLElement>("[data-navigation-scroll]");if(previousNavigation)saveScroll(navigationRenderedStage,previousNavigation.scrollTop);
 const priorScroll=root.querySelector<HTMLElement>("[data-navigation-scroll]");if(priorScroll)saveScroll(tab as Stage,priorScroll.scrollTop);
 root.removeAttribute("aria-busy");
 if(renderedKey){const fields:Record<string,string>={};root.querySelectorAll<HTMLTextAreaElement>('textarea[id]').forEach(el=>fields[el.id]=el.value);drafts.set(renderedKey,fields);}
 const p=state.project;if(!p&&sourceRoot!==undefined){sourceEpoch++;sourcePreviewVersion++;sourceSaveScheduled=false;sourceSavePromise=Promise.resolve();sourceDrafts.clear();sourceOrder=[];selectedSource="";sourceRoot=undefined;sourceDirty=false;sourceSaveError='';clearTimeout(sourcePreviewTimer);clearTimeout(sourceSaveTimer);}root.classList.toggle("welcome-root",!p);root.style.gridTemplateRows=p?"auto auto minmax(0,1fr) auto":"";
 const oldPlayer=document.getElementById("player")as HTMLAudioElement|null;
 const keepPlayer=oldPlayer&&!oldPlayer.paused?oldPlayer:null;
 if(!p){root.innerHTML=`<main class="welcome"><h1>読みあわせ</h1><div class="welcome-options"><section>${button("new","新規プロジェクト",'class="primary"')}<p>台本と録音を保存するプロジェクトを作成します。</p></section><section>${button("open","開く")}<p>保存済みの .scriptsync ファイルを開きます。</p></section></div><ol class="flow-graphic">${["台本","録音","読み確認・補正","比較・採点","結合・出力"].map((x,i)=>`<li><b>${i+1}</b> ${x}</li>`).join("")}</ol><span id="notice" role="status">${esc(state.message)}</span></main>`;renderedKey="";bind();}
 else{
 if(!current())selected=activeSegments()[0]?.id??"";const s=current();
 resetNavigation(state.root??p.title);
 const exportContext={project:p,selected,select:(_id:string)=>{},rpc,play:(file:string,hash:string)=>play(file,hash),notice};
 const stage=tab as Stage;
 ensureScriptDraft();
 const sidebar=tab==="export"||(tab==="script"&&(hasSourceMode()||!markdownDraft!.lines.length))?"":tab==="script"?`<aside class="navigation-aside"><label>フィルター<select id="filter">${optionsFor("script").map(([value,label])=>`<option value="${value}" ${filterFor(stage)===value?"selected":""}>${label}</option>`).join("")}</select></label><nav data-navigation-scroll aria-label="元Markdownの構成">${renderSourceTree(markdownDraft!.lines)}</nav></aside>`:`<aside class="navigation-aside"><label>フィルター<select id="filter">${optionsFor(stage as "recording"|"reading"|"evaluation").map(([value,label])=>`<option value="${value}" ${filterFor(stage)===value?"selected":""}>${label}</option>`).join("")}</select></label><nav data-navigation-scroll aria-label="確定原稿ツリー">${renderSegmentTree(p.segments,stage as "recording"|"reading"|"evaluation",selected)}</nav></aside>`;
 root.innerHTML=`<header><strong>読みあわせ</strong><div class="toolbar">${button("new","新規プロジェクト")}${button("open","開く")}</div><div class="statusbar"><span id="notice" role="status">${esc(state.message)}</span><span id="meter"></span></div></header>${renderStageBar(stage,activeSegments().length>0&&activeSegments().every(s=>s.takes.some(t=>t.id===s.selected&&t.status==="ready")))}<div class="layout navigation-layout ${!sidebar?"no-sidebar":""}">${sidebar}<main>${tab==="script"?scriptTab():tab==="export"?modernExportView(exportContext):s?segmentView(s):"<p>台本画面で原稿を追加してください。</p>"}<div id="stage-extras" class="stage-extras"></div></main></div><footer><label>試聴 <audio id="player" controls></audio></label></footer>`;
 if(tab==="script")root.querySelector("footer")?.setAttribute("hidden","");
 if(keepPlayer)document.getElementById("player")?.replaceWith(keepPlayer);
 renderedKey=`${selected}:${tab}`;for(const [id,value]of Object.entries(drafts.get(renderedKey)??{})){const field=document.getElementById(id);if(field instanceof HTMLTextAreaElement&&id!=="markdown-source")field.value=value;}
 if(s&&(tab==="reading"||tab==="evaluation")){enhance(s);comparisonTools(s);ruleView(s);}

 if(tab==="reading"||tab==="evaluation")batchView(tab);
 if(tab==="recording"){testGuide();if(s)root.querySelector('#text')?.insertAdjacentHTML('afterend',`<div class="toolbar">${button("split","カーソル位置で分割")}${button("merge","次と結合")}${button("script_insert_empty","空の台本行を挿入")}</div>`);else root.querySelector('#stage-extras')?.insertAdjacentHTML('beforeend',button("script_insert_empty","空の台本行を挿入"));renderRecordingAsrCards(s);bindContinuousControls();}
 fitWorkspace();bind();bindSourceScriptTab();bindBatch();if(tab==="recording")bindRecordingArchives({project:state.project!,rpc,play,refresh:()=>refresh(true),notice});updateMeter();void loadTakeWaves();
 if(tab==="recording"&&s)bindTakeTrim({segment:s,rpc:async<T>(action:string,payload?:unknown)=>{const result=await rpc<T>(action,payload);if(action!=="trim_take")return result;const latest=await rpc<Snapshot>("state");return latest.project?.segments.find(x=>x.id===s.id)?.takes.find(x=>x.id===(payload as {take:string}).take) as T;},play,isPlaying:take=>playingTake===take&&!player()?.paused,stop:()=>{player()?.pause();syncTakePlayback();},notice,onSaved:async()=>{player()?.pause();await refresh(true);}});
 if(player()){player().onplay=syncTakePlayback;player().onpause=syncTakePlayback;player().onended=()=>{updateTakeProgress();syncTakePlayback();};}syncTakePlayback();
 bindTrimNavigation();
 if(tab==="export")bindExportView(exportContext);
 navigationRenderedStage=stage;const navScroll=root.querySelector<HTMLElement>('[data-navigation-scroll]');if(navScroll)navScroll.scrollTop=scrollFor(stage);
 root.querySelectorAll<HTMLDetailsElement>('[data-tree]').forEach(el=>el.ontoggle=()=>setCollapsed(stage,el.dataset.tree!,!el.open));
 }
 if(busy||state.busy){root.setAttribute("aria-busy","true");root.querySelectorAll<HTMLButtonElement>("button").forEach(b=>{if(!["stop","cancel_record"].includes(b.dataset.action??"")&&b.id!=="continuous-stop")b.disabled=true;});root.querySelectorAll<HTMLInputElement>('input,textarea,select').forEach(el=>el.disabled=true);}
 if(dialogAction){root.insertAdjacentHTML("beforeend",`<div id="dialog-modal" class="dialog-modal" role="dialog" aria-modal="true"><div><strong>${esc(dialogAction)}</strong><p>選択画面を閉じるまでお待ちください。</p></div></div>`);}
 if(deleteRequest)showDeleteModal(deleteRequest);
}
function legacyBindScriptDecisions(){
 root.querySelectorAll<HTMLButtonElement>('[data-review-index]').forEach(b=>b.onclick=()=>{
  if(busy||state.busy)return;
  const row=legacyCurrentReview()[Number(b.dataset.reviewIndex)];if(!row)return;
  scriptDecisions.set(row.key,b.dataset.decision as Decision);scheduleScriptPreview();
 });
 root.querySelectorAll<HTMLButtonElement>('[data-script-line]').forEach(b=>b.onclick=()=>{
  if(busy||state.busy||scriptPreviewPending)return;
  const row=markdownDraft?.lines.find(x=>x.line===Number(b.dataset.scriptLine));if(!row)return;
  row.enabled=!row.enabled;scriptChoices.set(choiceKey(row),row.enabled);updateScriptPanels();
 });
}
function legacyBindScriptTab(){
 const mode=document.getElementById('script-import-mode') as HTMLSelectElement|null;
 if(mode)mode.onchange=()=>{scriptImportMode=mode.value;};
 const input=document.getElementById('markdown-source') as HTMLTextAreaElement|null;
 if(input)input.oninput=()=>{
  if(!markdownDraft)return;markdownDraft.text=input.value;
  const keys=new Set(legacyCurrentReview().map(r=>r.key));for(const key of scriptDecisions.keys())if(!keys.has(key))scriptDecisions.delete(key);
  scheduleScriptPreview();
 };
 const editor=root.querySelector<HTMLElement>('.script-editor');
 if(editor){editor.ondragover=e=>{if(e.dataTransfer?.types.includes('Files')){e.preventDefault();setScriptDrag(true);}};editor.ondragleave=e=>{if(!editor.contains(e.relatedTarget as Node))setScriptDrag(false);};editor.ondrop=()=>setScriptDrag(false);}
 bindScriptDecisions();bindReviewAll();
 root.querySelectorAll<HTMLButtonElement>('[data-collapse-pane]').forEach(button=>button.onclick=()=>{const index=Number(button.dataset.collapsePane);scriptCollapsed[index]=!scriptCollapsed[index];render();});
 const columns=root.querySelector<HTMLElement>('.script-columns');
 root.querySelectorAll<HTMLElement>('[data-separator]').forEach(bar=>{
  const index=Number(bar.dataset.separator);
  const move=(delta:number)=>{if(scriptCollapsed[index]||scriptCollapsed[index+1])return;const total=scriptWidths[index]+scriptWidths[index+1];const left=Math.max(.45,Math.min(total-.45,scriptWidths[index]+delta));scriptWidths[index]=left;scriptWidths[index+1]=total-left;if(columns)columns.style.gridTemplateColumns=`${scriptWidths[0]}fr 8px ${scriptWidths[1]}fr 8px ${scriptWidths[2]}fr`;bar.setAttribute('aria-valuenow',String(Math.round(left/total*100)));};
  bar.onkeydown=e=>{if(e.key==='ArrowLeft'||e.key==='ArrowRight'){e.preventDefault();move(e.key==='ArrowLeft'?-.08:.08);}};
  let previous=0;
  bar.onpointerdown=e=>{previous=e.clientX;bar.setPointerCapture(e.pointerId);};
  bar.onpointermove=e=>{if(!bar.hasPointerCapture(e.pointerId)||!columns)return;move((e.clientX-previous)/Math.max(1,columns.clientWidth-16)*3);previous=e.clientX;};
  bar.onpointerup=e=>{if(bar.hasPointerCapture(e.pointerId))bar.releasePointerCapture(e.pointerId);};
 });
 const bindAction=(name:string,callback:()=>Promise<void>)=>{const el=root.querySelector<HTMLButtonElement>(`[data-action="${name}"]`);if(el)el.onclick=()=>void callback();};
 bindAction('markdown_import',async()=>{if(busy)return;busy=true;dialogAction='台本ファイルを選択しています';render();try{const r=await rpc<{text:string;lines:any[]}>('markdown_import');if(r){await acceptMarkdown(r);drafts.delete(`${selected}:script`);}}catch(e){state.message=String(e);}finally{busy=false;dialogAction='';render();}});
 bindAction('markdown_commit',async()=>{
  if(busy||state.busy||scriptBlocked())return;
  const text=scriptResolved.text,lines=markdownDraft!.lines.map(row=>({...row,previous_line:scriptResolved.origins[row.line]??null}));
  if(await act('markdown_commit',{text,lines,base_generation:scriptBaseGeneration})){
   markdownDraft=null;scriptPreviewVersion++;drafts.clear();tab='recording';render();
  }
 });
}
let sourceDrafts=new Map<string,SourceDraft>();
let sourceOrder:string[]=[];
let selectedSource="";
let sourceEpoch=0;
let sourceRoot:string|null|undefined,sourceRevision=0,sourceGeneration=-1,sourceDirty=false,sourceSaveTimer:ReturnType<typeof setTimeout>|undefined,sourcePreviewTimer:ReturnType<typeof setTimeout>|undefined,sourceSaveVersion=0,sourceSaveScheduled=false,sourceSavePromise:Promise<void>=Promise.resolve();
let sourcePreviewVersion=0,sourcePreviewPending=false,sourcePreviewError="",sourceSaveError="";
function hasSourceMode(){return Array.isArray((state.project as any)?.script_sources);}
function ensureScriptDraft(){
 if(!hasSourceMode()){legacyEnsureScriptDraft();return;}
 const p=state.project as any,sources=p.script_sources as ScriptSource[];
 if(sourceRoot!==state.root||!sourceOrder.length||sources.some(source=>!sourceDrafts.has(source.id))){
  sourceEpoch++;clearTimeout(sourceSaveTimer);clearTimeout(sourcePreviewTimer);sourceSaveScheduled=false;sourcePreviewPending=false;sourcePreviewVersion++;sourceSavePromise=Promise.resolve();sourceDrafts=new Map(sources.map(source=>[source.id,makeDraft(source)]));sourceOrder=sources.map(source=>source.id);selectedSource=sourceOrder[0]??"";sourceRoot=state.root;sourceRevision=p.script_revision??0;sourceGeneration=p.generation??-1;sourceDirty=false;sourceSaveError='';
 }else if(p.generation!==undefined&&p.generation>sourceGeneration){if(sourceDirty){notice('外部の下書き更新を検出しました。現在の編集を保存せず再読込してください。',true);}else{sourceDrafts=new Map(sources.map(source=>[source.id,makeDraft(source)]));sourceOrder=sources.map(source=>source.id);sourceRevision=p.script_revision??sourceRevision;sourceGeneration=p.generation;}}
}
function selectedDraft(){ensureScriptDraft();return sourceDrafts.get(selectedSource);}
function sourceRows(){ensureScriptDraft();return sourceOrder.map(id=>sourceDrafts.get(id)).filter((x):x is SourceDraft=>!!x);}
function unresolvedScript(){return hasSourceMode()?sourceRows().reduce((total,draft)=>total+unresolvedSource(draft),0):legacyUnresolvedScript();}
function scriptBlocked(){return hasSourceMode()?sourcePreviewPending||!!sourcePreviewError||!!sourceSaveError||unresolvedScript()>0:legacyScriptBlocked();}
function scriptStatus(){return hasSourceMode()?sourceSaveError||sourcePreviewError||`未確認 ${unresolvedScript()}件${sourcePreviewPending?' ／ 読み上げ対象を更新中':' ／ ファイルごとに承認してから全体を確定します'}`:legacyScriptStatus();}
function scriptSeparator(index:number){return `<div class="script-separator" role="separator" tabindex="0" aria-label="${index+1}番目のペイン境界" aria-orientation="vertical" data-separator="${index}" aria-valuemin="15" aria-valuemax="85" aria-valuenow="50"></div>`;}
function sourceListHtml(){return `<div class="script-source-actions">${button('script_import','ファイルを登録')}${button('script_empty','空の原稿')}</div><div id="script-source-list" class="script-source-list">${sourceRows().map((draft,index)=>`<button draggable="true" class="script-source ${draft.source.id===selectedSource?'selected':''}" data-source-id="${esc(draft.source.id)}" data-source-index="${index}"><b>${esc(draft.source.name||'無題の原稿')}</b><span>${unresolvedSource(draft)?`未確認 ${unresolvedSource(draft)}件`:'確認済み'}</span></button>`).join('')||'<p class="muted">原稿ファイルを登録してください。</p>'}</div>`;}
function currentReview(){return hasSourceMode()?(selectedDraft()?reviewRows(selectedDraft()!.source.confirmed_text,selectedDraft()!.text):[]):legacyCurrentReview();}
function scriptDiffHtml(){if(!hasSourceMode())return legacyScriptDiffHtml();const draft=selectedDraft();if(!draft)return '<p class="muted">左で原稿を選択してください。</p>';const rows=currentReview(),changed=rows.filter(row=>row.kind!=="same");return `<div class="script-review-toolbar"><p class="diff-count">追加 ${changed.filter(row=>row.kind==='added').length}行 ／ 削除 ${changed.filter(row=>row.kind==='removed').length}行</p><div class="script-review-all">${button('script_approve_all','すべて承認',`aria-pressed="${changed.length>0&&unresolvedSource(draft)===0}"`)}${button('script_clear_all','すべて解除')}</div></div>${changed.length?rows.map((row,index)=>row.kind==='same'?'':`<div class="diff-row diff-${row.kind}"><b>${row.kind==='added'?'＋':'−'}</b><span class="diff-text">${esc(row.text)}</span><button data-review-index="${index}" aria-pressed="${draft.decisions.get(row.key)==='accept'}">承認</button></div>`).join(''):'<p class="muted">変更はありません。</p>'}`;}
function sourceCandidates(draft:SourceDraft){const sourceId=draft.source.id;return [...(state.project?.segments??[]),...((state.project as any)?.archived_segments??[])].filter((segment:any)=>segment.source_id===sourceId);}
function changedSourceLine(draft:SourceDraft,line:any){return line.previous_line===undefined||line.previous_line===null||draft.source.confirmed_text.split(/\r?\n/)[line.previous_line]!==draft.text.split(/\r?\n/)[line.line];}
function lineToken(line:any){return String(line.line);}
function mappingDetails(draft:SourceDraft,line:any){if(!changedSourceLine(draft,line)&&!((state.project as any)?.archived_segments??[]).some((s:any)=>s.source_id===draft.source.id&&s.source_line===line.previous_line))return "";const candidates=sourceCandidates(draft),used=new Set(draft.lines.filter(other=>other!==line).map((other:any)=>other.previous_id).filter(Boolean)),selected=line.previous_id??"";return `<details class="source-mapping"><summary>既存の発話・読みを維持する</summary><label>対応先<select data-previous-id="${esc(lineToken(line))}"><option value="">新規の発話として扱う</option>${candidates.map((segment:any)=>`<option value="${esc(segment.id)}" ${selected===segment.id?'selected':''} ${used.has(segment.id)&&selected!==segment.id?'disabled':''}>${esc(segment.text||segment.id)}</option>`).join('')}</select></label><label class="inline"><input type="checkbox" data-keep-line="${esc(lineToken(line))}" ${line.keep?'checked':''} ${selected?'':'disabled'}> この発話の既存の読み・テイクを維持する</label></details>`;}
function scriptTargetHtml(){if(!hasSourceMode())return legacyScriptTargetHtml();const draft=selectedDraft();return (draft?.lines??[]).filter(line=>String((line as any).text??(line as any).raw??'').trim()).map(line=>`<div class="source-target-row"><button class="source-line ${line.enabled?'enabled':'disabled'}" aria-pressed="${line.enabled}" data-script-line="${line.line}"><span>${line.enabled?'☑':'☐'}</span><em>${esc((line as any).text??(line as any).raw??'')}</em></button>${mappingDetails(draft!,line)}</div>`).join('')||'<p class="muted">読み上げ対象の行はありません。</p>';}
function scriptTab(){if(!hasSourceMode())return legacyScriptTab();ensureScriptDraft();return `<section class="script-tab script-workspace"><div class="script-heading"><h2>台本</h2></div><div class="script-columns script-four-columns" style="grid-template-columns:${scriptGrid()}"><section class="script-files ${scriptCollapsed[0]?'collapsed':''}">${scriptPaneHeading(0,'原稿を登録する')}${sourceListHtml()}</section>${scriptSeparator(0)}<section class="script-editor ${scriptCollapsed[1]?'collapsed':''}">${scriptPaneHeading(1,'原稿を閲覧／編集する')}<textarea id="markdown-source" aria-label="選択した原稿を編集" spellcheck="false" ${selectedDraft()?'':'disabled'}>${esc(selectedDraft()?.text??'')}</textarea></section>${scriptSeparator(1)}<section class="script-review"><h3>差分を確認する</h3><div id="script-difference" class="script-difference">${scriptDiffHtml()}</div></section>${scriptSeparator(2)}<section class="script-targets"><h3>読み上げ対象を選択する</h3><div id="script-target-list" class="source-list">${scriptTargetHtml()}</div></section></div><div class="script-bottom"><span id="script-review-status">${scriptStatus()}</span>${button('script_commit','台本確定',`class="primary" ${scriptBlocked()?'disabled':''}`)}</div></section>`;}
let scriptWidths=[1,1,1,1];
let scriptCollapsed=[false,false];
function scriptGrid(){return scriptWidths.flatMap((width,index)=>{const column=scriptCollapsed[index]?(index===0?'var(--script-files-collapsed, 180px)':'52px'):`minmax(0, ${width}fr)`;return index<3?[column,'8px']:[column];}).join(' ');}
function scriptPaneHeading(index:number,title:string){return `<div class="script-pane-heading"><h3 title="${title}">${title}</h3><button data-collapse-pane="${index}" aria-label="${title}を${scriptCollapsed[index]?'展開':'折りたたむ'}" aria-expanded="${!scriptCollapsed[index]}">${scriptCollapsed[index]?'›':'‹'}</button></div>`;}
function bindReviewAll(){
 const approve=root.querySelector<HTMLButtonElement>('[data-action=script_approve_all]');
 if(approve)approve.onclick=()=>{const draft=selectedDraft();if(!draft)return;currentReview().filter(row=>row.kind!=="same").forEach(row=>draft.decisions.set(row.key,'accept'));queueSourceDraftSave();updateScriptPanels();};
 const clear=root.querySelector<HTMLButtonElement>('[data-action=script_clear_all]');
 if(clear)clear.onclick=()=>{selectedDraft()?.decisions.clear();queueSourceDraftSave();updateScriptPanels();};
}
function queueSourcePreview(){sourceDirty=true;sourceSaveVersion++;clearTimeout(sourcePreviewTimer);sourcePreviewPending=true;sourcePreviewError='';const version=++sourcePreviewVersion,sourceId=selectedSource;updateScriptPanels();sourcePreviewTimer=setTimeout(()=>void previewSelectedSource(version,sourceId),120);}
async function previewSelectedSource(version:number,sourceId=selectedSource){
 const draft=sourceDrafts.get(sourceId);if(!draft)return;
 const text=draft.text;
 try{
  const parsed=await rpc<{lines:any[]}>('markdown_preview',{text});
  if(version!==sourcePreviewVersion||draft!==sourceDrafts.get(sourceId)||text!==draft.text)return;
  updateParsedLines(draft,parsed.lines);sourcePreviewPending=false;queueSourceDraftSave();updateScriptPanels();
 }catch(error){if(version===sourcePreviewVersion){sourcePreviewError=String(error);sourcePreviewPending=false;updateScriptPanels();}}
}
function sourcePayloads(){return sourceRows().map(sourcePayload);}
function queueSourceDraftSave(){sourceDirty=true;sourceSaveScheduled=true;sourceSaveVersion++;clearTimeout(sourceSaveTimer);sourceSaveTimer=setTimeout(()=>void startSourceSave(),220);}
function startSourceSave(){if(!sourceSaveScheduled)return;sourceSaveScheduled=false;const version=sourceSaveVersion,epoch=sourceEpoch;sourceSavePromise=sourceSavePromise.then(()=>saveSourceDraft(version,epoch));}
async function flushSourceDraft(){
 do{if(sourceSaveScheduled){clearTimeout(sourceSaveTimer);startSourceSave();}await sourceSavePromise;}while(sourceSaveScheduled);
 if(sourceSaveError)throw new Error(sourceSaveError);
}
async function saveSourceDraft(version:number,epoch:number){
 if(epoch!==sourceEpoch)return;
 try{
  const drafts=sourceRows();
  for(const draft of drafts){
   const text=draft.text,parsed=await rpc<{lines:any[]}>('markdown_preview',{text});
   if(epoch!==sourceEpoch)return;
   if(text!==draft.text){queueSourceDraftSave();return;}
   updateParsedLines(draft,parsed.lines);
  }
  if(epoch!==sourceEpoch)return;
  const payload={project_id:(state.project as any)?.id,base_revision:sourceRevision,base_generation:sourceGeneration,sources:drafts.map(sourcePayload)};
  const result=await rpc<{generation?:number;script_revision?:number}>('script_draft',payload);
  if(epoch!==sourceEpoch)return;
  sourceGeneration=result?.generation??sourceGeneration;sourceRevision=result?.script_revision??sourceRevision;sourceSaveError='';
  if(version===sourceSaveVersion&&!sourceSaveScheduled&&!sourcePreviewPending)sourceDirty=false;
  updateScriptPanels();
 }catch(error){if(epoch===sourceEpoch){sourceDirty=true;sourceSaveError=String(error);notice(`下書きを保存できません: ${sourceSaveError}`,true);updateScriptPanels();}}
}
function updateScriptPanels(){if(!hasSourceMode()){legacyUpdateScriptPanels();return;}if(tab!=="script")return;const diff=document.getElementById('script-difference');if(diff)diff.innerHTML=scriptDiffHtml();const targets=document.getElementById('script-target-list');if(targets)targets.innerHTML=scriptTargetHtml();const status=document.getElementById('script-review-status');if(status)status.textContent=scriptStatus();const commit=root.querySelector<HTMLButtonElement>('[data-action=script_commit]');if(commit)commit.disabled=busy||state.busy||scriptBlocked();root.querySelectorAll<HTMLElement>('[data-source-id]').forEach(el=>{const draft=sourceDrafts.get(el.dataset.sourceId!),badge=el.querySelector('span');if(draft&&badge)badge.textContent=unresolvedSource(draft)?`未確認 ${unresolvedSource(draft)}件`:'確認済み';});bindScriptDecisions();bindReviewAll();}
function bindScriptDecisions(){if(!hasSourceMode()){legacyBindScriptDecisions();return;}root.querySelectorAll<HTMLButtonElement>('[data-review-index]').forEach(button=>button.onclick=()=>{const draft=selectedDraft(),row=currentReview()[Number(button.dataset.reviewIndex)];if(!draft||!row)return;draft.decisions.set(row.key,'accept');queueSourceDraftSave();updateScriptPanels();});root.querySelectorAll<HTMLButtonElement>('[data-script-line]').forEach(button=>button.onclick=()=>{const draft=selectedDraft(),line=draft?.lines.find(line=>line.line===Number(button.dataset.scriptLine));if(!line)return;line.enabled=!line.enabled;queueSourceDraftSave();updateScriptPanels();});root.querySelectorAll<HTMLSelectElement>('[data-previous-id]').forEach(select=>select.onchange=()=>{const draft=selectedDraft(),line=draft?.lines.find(line=>line.line===Number(select.dataset.previousId));if(!draft||!line)return;const id=select.value||undefined;if(id&&draft.lines.some(other=>other!==line&&(other as any).previous_id===id)){notice('同じ既存発話は一度だけ対応できます',true);select.value=(line as any).previous_id??'';return;}(line as any).previous_id=id;(line as any).keep=!!id;queueSourceDraftSave();updateScriptPanels();});root.querySelectorAll<HTMLInputElement>('[data-keep-line]').forEach(input=>input.onchange=()=>{const line=selectedDraft()?.lines.find(line=>line.line===Number(input.dataset.keepLine));if(!line)return;(line as any).keep=input.checked;if(!input.checked)delete line.previous_id;queueSourceDraftSave();updateScriptPanels();});}
async function applyRegisteredFiles(files:{name:string;text:string}[]){for(const file of files){const existing=sourceRows().find(draft=>draft.source.name===file.name);if(existing&&existing.text!==existing.source.confirmed_text&&!window.confirm(`未確定の「${file.name}」を読み込んだ内容で置き換えますか？`))continue;const id=existing?.source.id??crypto.randomUUID();const source:ScriptSource={id,name:file.name,kind:existing?.source.kind??'markdown',confirmed_text:existing?.source.confirmed_text??'',draft_text:file.text,lines:existing?.source.lines??[],draft_lines:existing?.lines??[],blank_rows:existing?.source.blank_rows};sourceDrafts.set(id,makeDraft(source));if(!sourceOrder.includes(id))sourceOrder.push(id);}for(const id of sourceOrder){selectedSource=id;const version=++sourcePreviewVersion;await previewSelectedSource(version);}selectedSource=sourceOrder.at(-1)??'';queueSourceDraftSave();render();}
async function registerScriptFiles(){try{const result=await rpc<{files?:{name:string;text:string}[];text?:string}|null>('markdown_import');if(!result)return;const files=result.files??(result.text!==undefined?[{name:'原稿.md',text:result.text}]:[]);await applyRegisteredFiles(files);}catch(error){notice(String(error),true);}}
function addEmptyScriptSource(){const existing=new Set(sourceRows().map(draft=>draft.source.name));let number=1,name='空の原稿.md';while(existing.has(name)){number++;name=`空の原稿 (${number}).md`;}const id=crypto.randomUUID();sourceDrafts.set(id,makeDraft({id,name,kind:'markdown',confirmed_text:'',draft_text:'',lines:[],draft_lines:[]}));sourceOrder.push(id);selectedSource=id;queueSourceDraftSave();render();}
function bindSourceScriptTab(){
 if(!hasSourceMode()){legacyBindScriptTab();return;}
 const action=(name:string,handler:()=>void)=>{const element=root.querySelector<HTMLButtonElement>(`[data-action="${name}"]`);if(element)element.onclick=handler;};
 const input=document.getElementById('markdown-source') as HTMLTextAreaElement|null;
 if(input)input.oninput=()=>{const draft=selectedDraft();if(!draft)return;sourceDirty=true;draft.text=input.value;draft.decisions.clear();queueSourcePreview();};
 root.querySelectorAll<HTMLButtonElement>('[data-source-id]').forEach(button=>button.onclick=()=>{selectedSource=button.dataset.sourceId!;render();});
 root.querySelectorAll<HTMLElement>('[data-source-index]').forEach(item=>{item.ondragstart=event=>event.dataTransfer?.setData('text/plain',item.dataset.sourceIndex!);item.ondragover=event=>event.preventDefault();item.ondrop=event=>{event.preventDefault();const from=Number(event.dataTransfer?.getData('text/plain')),to=Number(item.dataset.sourceIndex);if(Number.isFinite(from)&&Number.isFinite(to)&&from!==to){sourceOrder=reorder(sourceOrder,from,to);queueSourceDraftSave();render();}};});
 action('script_import',()=>void registerScriptFiles());action('script_empty',addEmptyScriptSource);
 const source=selectedDraft();const actions=root.querySelector<HTMLElement>('.script-source-actions');if(source&&actions)bindLegacySources(actions,{source:{...source.source,draft_text:source.text},segments:[...(state.project?.segments??[]),...((state.project as any)?.archived_segments??[])],rpc,flush:flushSourceDraft,base:()=>({base_revision:sourceRevision,base_generation:sourceGeneration}),refresh:async()=>{sourceDrafts.clear();sourceOrder=[];sourceRoot=undefined;await refresh(true);},notice});
 action('script_approve_all',()=>{const draft=selectedDraft();if(!draft)return;currentReview().filter(row=>row.kind!=="same").forEach(row=>draft.decisions.set(row.key,'accept'));queueSourceDraftSave();updateScriptPanels();});
 action('script_clear_all',()=>{selectedDraft()?.decisions.clear();queueSourceDraftSave();updateScriptPanels();});
 action('script_commit',()=>void (async()=>{if(scriptBlocked())return;try{await flushSourceDraft();if(scriptBlocked())return;const result=await rpc<{generation?:number;script_revision?:number}>('script_commit',{project_id:(state.project as any)?.id,base_revision:sourceRevision,base_generation:sourceGeneration,sources:sourcePayloads()});sourceGeneration=result?.generation??sourceGeneration;sourceRevision=result?.script_revision??sourceRevision;sourceDrafts.clear();sourceOrder=[];sourceRoot=undefined;await refresh(true);tab='recording';render();}catch(error){sourceSaveError=String(error);notice(String(error),true);updateScriptPanels();}})());
 bindScriptDecisions();bindReviewAll();
 root.querySelectorAll<HTMLButtonElement>('[data-collapse-pane]').forEach(button=>button.onclick=()=>{const index=Number(button.dataset.collapsePane);scriptCollapsed[index]=!scriptCollapsed[index];render();});
 const columns=root.querySelector<HTMLElement>('.script-columns');root.querySelectorAll<HTMLElement>('[data-separator]').forEach(bar=>{const index=Number(bar.dataset.separator);const move=(delta:number)=>{if(scriptCollapsed[index]||scriptCollapsed[index+1])return;const total=scriptWidths[index]+scriptWidths[index+1],left=Math.max(.45,Math.min(total-.45,scriptWidths[index]+delta));scriptWidths[index]=left;scriptWidths[index+1]=total-left;if(columns)columns.style.gridTemplateColumns=scriptGrid();};bar.onkeydown=event=>{if(event.key==='ArrowLeft'||event.key==='ArrowRight'){event.preventDefault();move(event.key==='ArrowLeft'?-.08:.08);}};let previous=0;bar.onpointerdown=event=>{previous=event.clientX;bar.setPointerCapture(event.pointerId);};bar.onpointermove=event=>{if(bar.hasPointerCapture(event.pointerId)&&columns){move((event.clientX-previous)/Math.max(1,columns.clientWidth)*4);previous=event.clientX;}};bar.onpointerup=event=>{if(bar.hasPointerCapture(event.pointerId))bar.releasePointerCapture(event.pointerId);};});
}

function showDeleteModal(take:string){
 deleteRequest=take;document.querySelector('.delete-modal')?.remove();
 root.insertAdjacentHTML('beforeend',`<div class="delete-modal" role="dialog" aria-modal="true" aria-labelledby="delete-title"><section><h2 id="delete-title">テイクを削除しますか？</h2><p>このテイクの音声とメモを削除します。</p><div class="toolbar"><button id="delete-cancel">キャンセル</button><button id="delete-confirm" class="danger">削除</button></div></section></div>`);
 const modal=root.querySelector<HTMLElement>('.delete-modal')!;
 const siblings=[...root.children].filter(el=>el!==modal) as HTMLElement[];
 siblings.forEach(el=>el.inert=true);
 const close=()=>{deleteRequest='';siblings.forEach(el=>el.inert=false);modal.remove();};
 const cancel=document.getElementById('delete-cancel') as HTMLButtonElement,confirm=document.getElementById('delete-confirm') as HTMLButtonElement;
 cancel.onclick=close;confirm.onclick=()=>{const id=deleteRequest;close();void act('delete_take',{segment:selected,take:id});};
 modal.onkeydown=e=>{if(e.key==='Escape'){e.preventDefault();e.stopPropagation();close();}if(e.key==='Tab'){e.preventDefault();(document.activeElement===cancel?confirm:cancel).focus();}};
 cancel.focus();
}async function endContinuous(){pendingRerecord="";if(continuousRecording){await invoke("continuous_hotkeys",{enabled:false});continuousRecording=false;}}
function updateRecordingWave(){
 const svg=document.getElementById('recording-wave'),time=document.getElementById('recording-elapsed');
 if(time)time.textContent=`${(state.elapsed/1000).toFixed(1)} 秒`;
 if(!svg)return;
 const peaks=state.recording_peaks??[],count=Math.min(600,peaks.length);
 const lines=Array.from({length:count},(_,i)=>{const start=Math.floor(i*peaks.length/count),end=Math.floor((i+1)*peaks.length/count);let peak=0;for(let j=start;j<end;j++)peak=Math.max(peak,peaks[j]);const height=waveAmplitude(peak)*21,x=(i+.5)/count*600;return `M${x},${22-height}v${height*2}`;}).join(' ');
 svg.innerHTML=`<path d="M0 22H600" stroke="#c8d9d0"/><path d="${lines}" stroke="#29836b" stroke-width="1"/>`;
}
function bindContinuousControls(){
 if(tab!=='recording'||!current())return;
 const controls=document.querySelector('.utterance-heading');if(!controls)return;
 controls.insertAdjacentHTML('beforeend',`<div class="toolbar continuous-controls"><button id="continuous-start" class="primary" ${continuousRecording?'disabled':''}>連続録音を開始</button><button id="continuous-stop" ${continuousRecording?'':'disabled'}>連続録音を終了</button>${continuousRecording?'<span class="badge">F1 取消して再録音 / F2 保存 / F3 リプレイ / F4 採用して次へ</span>':''}</div>`);
 document.getElementById('continuous-start')!.onclick=async()=>{if(continuousRecording||busy||state.busy)return;try{await invoke('continuous_hotkeys',{enabled:true});continuousRecording=true;render();await startRecording();}catch(e){notice(String(e),true);}};
 document.getElementById('continuous-stop')!.onclick=async()=>{try{await endContinuous();if(state.job==='recording')await act('stop');render();}catch(e){notice(String(e),true);}};
}
async function continuousKey(key:string,local=false){
 if(!continuousRecording&&(!local||tab!=="recording"))return;
 if(document.querySelector('[role=dialog]')){void playCue('invalid');return;}
 if(key==='F1'){
  if(startingRecording){void playCue('invalid');return;}
  if(busy||state.job==='saving'||pendingRerecord){if(continuousRecording)pendingRerecord=selected;else void playCue('invalid');return;}
  if(state.job==='recording'){pendingRerecord=continuousRecording?selected:'';if(!await act('cancel_record'))pendingRerecord='';}
  else if(!state.busy)await startRecording();
  else void playCue('invalid');
  return;
 }
 if(busy||state.busy&&state.job!=='recording'||pendingRerecord){void playCue('invalid');return;}
 if(key==='F2'&&state.job==='recording'&&!state.stop_scheduled){await stopRecording();return;}
 if((key==='F3'||key==='F4')&&!state.busy){
  const take=current()?.takes.filter(t=>t.status==='ready').at(-1);
  if(take){if(key==='F3')await playSavedTake(take);else await adoptTake(take.id);return;}
 }
 void playCue('invalid');
}
async function initializeUi(){
 try{const result=await rpc<{recovering?:boolean}>('ui_attach');if(result?.recovering)tab='recording';await loadDevices();await refresh(true);}
 catch(e){notice(String(e),true);}finally{uiReady=true;}
}if(desktop){void listen<string>('scriptsync-hotkey',event=>{void continuousKey(event.payload);}).catch(e=>notice(String(e),true));}render();if(desktop){void initializeUi();setInterval(()=>{if(uiReady)void refresh();},700);setInterval(()=>{if(uiReady&&state.busy&&state.job==='recording')void refresh();},100);setInterval(()=>{if(uiReady)void rpc<{interrupted?:boolean}>('ui_heartbeat').then(async result=>{if(result?.interrupted){uiReady=false;await endContinuous();await initializeUi();}}).catch(()=>{});},1000);window.addEventListener('pagehide',()=>{void rpc('ui_detach').catch(()=>{});});}else{notice('画面プレビューです。録音・保存はデスクトップアプリで使用できます。');}

function testGuide(){
 if(!state.project)return;
 const hint=readingTestHint(current()?.text??"");
 if(hint&&(tab==="recording"||tab==="reading"))document.querySelector('main .section-head')?.insertAdjacentHTML('afterend',`<details class="panel"><summary>この発話の読み上げテスト手順</summary><p>${esc(hint.check)}</p><p>意図する読み：${esc(hint.reading)}</p><p class="muted">これは録音の手引きです。実際の音声と異なる場合は録り直し、試聴してから正解読みを確定してください。比較・採点中はこの手引きを表示しません。</p></details>`);
}
// 開発時だけ、検証用プロジェクトを読み取り専用で表示する。
if(import.meta.env.DEV&&!desktop){const preview=new URLSearchParams(location.search).get("preview");if(preview?.startsWith("output/tests/")&&!preview.includes("..")){void fetch(`/${preview}/project.json`).then(r=>r.json()).then(project=>{state={project,root:preview,busy:false,job:"",message:"" ,peak:0,elapsed:0};selected=project.segments[0]?.id??"";render();});}}

async function playSavedTake(take:Take){
 if(stopTakePlayback(take.id))return;
 const start=take.trim_start??0,end=take.trim_end||take.duration;
 if(start===0&&end===take.duration){await play(take.file,take.hash,take.id);return;}
 try{const clip=await rpc<{file:string;hash:string}>('preview_take_trim',{segment:selected,take:take.id,start,end});await play(clip.file,clip.hash,take.id,start,end-start);}catch(e){notice(String(e),true);}
}
function bindTrimNavigation(){
 root.querySelectorAll<HTMLButtonElement>('[data-segment],[data-tab],[data-action=previous_segment],[data-action=next_segment]').forEach(button=>{
  const handler=button.onclick;if(!handler)return;
  button.onclick=async event=>{await guardTakeTrimMove(async()=>{await handler.call(button,event);});};
 });
}

function qualityMessage(warning:string){return (warning??"").replace("欠落した発話は復元できません。試聴して再録音を検討してください", "").trim();}
function showQualityConfirmation(take:string,warning:string){
 const modal=document.createElement('div');modal.className='take-trim-modal';modal.setAttribute('role','dialog');modal.setAttribute('aria-modal','true');
 modal.innerHTML=`<section><h2>録音品質を確認してください</h2><p style="white-space:pre-wrap">${esc(qualityMessage(warning))}</p><div class="toolbar quality-actions"><button data-quality-cancel>戻って試聴する</button><button data-quality-adopt class="primary">採用</button></div></section>`;
 const siblings=[...root.children] as HTMLElement[];siblings.forEach(x=>x.inert=true);root.append(modal);
 const close=()=>{siblings.forEach(x=>x.inert=false);modal.remove();};
 const cancel=modal.querySelector<HTMLButtonElement>('[data-quality-cancel]')!,adopt=modal.querySelector<HTMLButtonElement>('[data-quality-adopt]')!;
 cancel.onclick=close;adopt.onclick=()=>{close();void adoptTake(take,true);};
 modal.onkeydown=e=>{if(e.key==='Enter'){e.preventDefault();adopt.click();}if(e.key==='Escape'){e.preventDefault();close();}if(e.key==='Tab'){e.preventDefault();(document.activeElement===cancel?adopt:cancel).focus();}};adopt.focus();
}
