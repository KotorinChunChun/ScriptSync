export type Stage = "script"|"recording"|"reading"|"evaluation"|"export";

export type NavigationSegment={id:string;text:string;skip:boolean;selected:string|null;confirmed:boolean;needs_review?:boolean;hierarchy?:number[];takes:{status:string}[];corrections:{decision:string}[];comparisons:{stale?:boolean;variants:{status:string;rating:Record<string,unknown>|null}[]}[]};
export type SourceLine={line:number;text:string;raw:string;enabled:boolean;kind:number};

const stages:[Stage,string][]=[["script","台本"],["recording","録音"],["reading","読み確認・補正"],["evaluation","比較・採点"],["export","結合・出力"]];
const labels:Record<Exclude<Stage,"export">,[string,string][]>={
 script:[["all","すべて"],["enabled","読み上げオン"],["disabled","読み上げオフ"]],
 recording:[["all","すべて"],["unrecorded","未収録"],["unselected","未採用"],["review","原稿変更後の再確認"]],
 reading:[["all","すべて"],["unconfirmed","読み未確認"],["pending","補正保留"],["unselected","採用音声なし"]],
 evaluation:[["all","すべて"],["missing","比較未生成"],["unrated","未採点"],["review","原稿変更後の再比較"]],
};
const filters:Record<Stage,string>={script:"all",recording:"all",reading:"all",evaluation:"all",export:"all"};
const collapsed:Record<Stage,Set<string>>={script:new Set(),recording:new Set(),reading:new Set(),evaluation:new Set(),export:new Set()};
const scrolls:Partial<Record<Stage,number>>={};
let projectKey="";

export function resetNavigation(key:string){if(key===projectKey)return;projectKey=key;for(const stage of Object.keys(filters) as Stage[]){filters[stage]="all";collapsed[stage].clear();delete scrolls[stage];}}
export function filterFor(stage:Stage){return filters[stage];}
export function setFilter(stage:Stage,value:string){filters[stage]=value;}
export function optionsFor(stage:Exclude<Stage,"export">){return labels[stage];}
export function saveScroll(stage:Stage,value:number){scrolls[stage]=value;}
export function scrollFor(stage:Stage){return scrolls[stage]??0;}
export function setCollapsed(stage:Stage,key:string,isCollapsed:boolean){if(isCollapsed)collapsed[stage].add(key);else collapsed[stage].delete(key);}
export function isCollapsed(stage:Stage,key:string){return collapsed[stage].has(key);}
function successfulVariants(s:NavigationSegment){return s.comparisons.filter(run=>!run.stale).flatMap(run=>run.variants.filter(variant=>variant.status==="succeeded"));}
function hasIncompleteRating(variant:{rating:Record<string,unknown>|null}){return !variant.rating||["accuracy","naturalness","accent","rhythm","emotion"].some(key=>variant.rating![key]===null||variant.rating![key]===undefined||variant.rating![key]==="");}
export function matchesSegment(stage:Stage,s:NavigationSegment){
 const filter=filters[stage];
 if(filter==="all")return true;
 if(filter==="unrecorded")return !s.takes.some(t=>t.status==="ready");
 if(filter==="unselected")return !s.selected;
 if(filter==="unconfirmed")return !s.confirmed;
 if(filter==="pending")return s.corrections.some(c=>c.decision==="pending");
 if(filter==="review")return !!s.needs_review||(stage==="evaluation"&&s.comparisons.some(run=>!!run.stale));
 const variants=successfulVariants(s);
 if(filter==="missing")return !variants.length;
 if(filter==="unrated")return variants.some(hasIncompleteRating);
 return true;
}
export function matchesSource(line:SourceLine){const filter=filters.script;return filter==="all"||(filter==="enabled"&&line.enabled)||(filter==="disabled"&&!line.enabled);}
export function renderStageBar(active:Stage){return `<nav class="stage-bar" aria-label="工程">${stages.map(([key,label],index)=>`<button role="tab" data-tab="${key}" aria-selected="${active===key}"><span>${index+1}</span>${label}</button>`).join("")}</nav>`;}
export function renderSourceTree(lines:SourceLine[]){
 const row=(line:SourceLine)=>`<button class="source-tree-line ${line.enabled?"enabled":"disabled"}" data-source-tree-line="${line.line}"><span>${line.line+1}</span><b>${["本文","部","章","節","項"][line.kind]??"本文"}</b><em>${escapeHtml(line.text||line.raw)}</em></button>`;
 const render=(items:SourceLine[]):string=>{let html="";for(let index=0;index<items.length;){const line=items[index];if(!line.kind){if(matchesSource(line))html+=row(line);index++;continue;}let end=index+1;while(end<items.length&&(!items[end].kind||items[end].kind>line.kind))end++;const children=render(items.slice(index+1,end));if(matchesSource(line)||children){const key=`source-${line.line}`;html+=`<details class="tree-group source-tree-group" data-tree="${key}" ${isCollapsed("script",key)?"":"open"}><summary data-source-tree-line="${line.line}">第${line.line+1}行　${escapeHtml(line.text||line.raw)}</summary>${children}</details>`;}index=end;}return html;};
 return render(lines)||emptyTree();
}
export function renderSegmentTree(segments:NavigationSegment[],stage:Exclude<Stage,"script"|"export">,selected:string){
 const visible=segments.filter(segment=>matchesSegment(stage,segment)&&!segment.skip);
 const render=(items:NavigationSegment[],depth=0):string=>{
  if(depth>=3)return items.map(line=>`<button class="segment ${line.id===selected?"selected":""}" data-segment="${line.id}"><span>${String(segments.indexOf(line)+1).padStart(4,"0")}</span>${status(stage,line)}${escapeHtml(line.text)}</button>`).join("");
  let html="";for(let index=0;index<items.length;){const level=items[index].hierarchy?.[depth]??0;if(!level){html+=render([items[index++]],depth+1);continue;}let end=index+1;while(end<items.length&&(items[end].hierarchy?.[depth]??0)===level)end++;const key=(items[index].hierarchy??[]).slice(0,depth+1).join("-");html+=`<details class="tree-group" data-tree="${key}" ${isCollapsed(stage,key)?"":"open"}><summary>第${level}${["部","章","節"][depth]}</summary>${render(items.slice(index,end),depth+1)}</details>`;index=end;}
  return html;
 };
 return render(visible)||emptyTree();
}
function status(stage:Stage,s:NavigationSegment){if(stage==="recording")return `<i>${!s.takes.some(t=>t.status==="ready")?"未収録":!s.selected?"未採用":"●"}</i>`;if(stage==="reading")return `<i>${!s.selected?"音声なし":s.confirmed?"確認済":"未確認"}</i>`;const variants=successfulVariants(s);return `<i>${s.comparisons.some(run=>run.stale)?"要再比較":!variants.length?"未生成":variants.some(hasIncompleteRating)?"未採点":"採点済"}</i>`;}
function emptyTree(){return `<p class="empty-tree">該当する原稿がありません。<button type="button" data-reset-filter>すべて表示</button></p>`;}
function escapeHtml(value:unknown){return String(value??"").replace(/[&<>"']/g,char=>({"&":"&amp;","<":"&lt;",">":"&gt;","\"":"&quot;","'":"&#39;"}[char]!));}
