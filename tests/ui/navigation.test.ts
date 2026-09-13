import {beforeEach,describe,expect,it} from "vitest";
import {recordingNavigationRows,renderStageBar,renderSegmentTree,renderSourceTree,optionsFor,filterFor,matchesSegment,matchesSource,resetNavigation,setCollapsed,setFilter,isCollapsed,scrollFor,saveScroll} from "../../src/navigation";

const segment:any={id:"s1",text:"原稿",skip:false,selected:null,confirmed:false,takes:[],corrections:[],comparisons:[]};

describe("工程ナビゲーションの状態",()=>{
 beforeEach(()=>resetNavigation(`project-${crypto.randomUUID()}`));
 it("録音は読み上げ対象を既定とし、収録後も表示を維持する",()=>{expect(filterFor("recording")).toBe("reading-target");expect(optionsFor("recording")[1]).toEqual(["reading-target","読み上げ対象"]);const recorded={...segment,takes:[{status:"ready"}],selected:"take1"};expect(matchesSegment("recording",recorded)).toBe(true);expect(renderSegmentTree([recorded],"recording","s1")).toContain('data-segment="s1"');expect(matchesSegment("recording",{...segment,skip:true})).toBe(false);});
 it("すべて表示は対象外を識別して表示し、他工程や未収録では除外する",()=>{const skipped={...segment,id:"skipped",skip:true};setFilter("recording","all");expect(renderSegmentTree([skipped],"recording","")).toContain('data-segment="skipped"');expect(renderSegmentTree([skipped],"recording","")).toContain("対象外");expect(renderSegmentTree([skipped],"reading","")).not.toContain('data-segment="skipped"');expect(renderSegmentTree([skipped],"evaluation","")).not.toContain('data-segment="skipped"');setFilter("recording","unrecorded");expect(renderSegmentTree([skipped],"recording","")).not.toContain('data-segment="skipped"');});
 it("プロジェクト切替時は読み上げ対象へ戻し、同じプロジェクト内では選択を保持する",()=>{resetNavigation("same");setFilter("recording","all");resetNavigation("same");expect(filterFor("recording")).toBe("all");resetNavigation("next");expect(filterFor("recording")).toBe("reading-target");});
 it("空欄案内は原稿と台本を区別する",()=>{expect(renderSourceTree([])).toContain("該当する原稿がありません");expect(renderSegmentTree([],"recording","")).toContain("該当する台本がありません");});
 it("工程ごとにフィルターとスクロール位置を保持する",()=>{setFilter("recording","unrecorded");saveScroll("recording",42);setFilter("reading","pending");expect(filterFor("recording")).toBe("unrecorded");expect(filterFor("reading")).toBe("pending");expect(scrollFor("recording")).toBe(42);});
 it("プロジェクトを切り替えると工程状態を初期化する",()=>{setFilter("evaluation","unrated");setCollapsed("evaluation","1-2",true);resetNavigation("other");expect(filterFor("evaluation")).toBe("all");expect(isCollapsed("evaluation","1-2")).toBe(false);});
 it("録音・読み確認・比較の対象状態を工程別に判定する",()=>{setFilter("recording","unrecorded");expect(matchesSegment("recording",segment)).toBe(true);setFilter("reading","unselected");expect(matchesSegment("reading",segment)).toBe(true);setFilter("evaluation","missing");expect(matchesSegment("evaluation",segment)).toBe(true);});
 it("比較工程は成功候補がなく、または評価が一部未入力なら作業対象にする",()=>{const failed={...segment,comparisons:[{stale:false,variants:[{status:"failed",rating:null}]}]};setFilter("evaluation","missing");expect(matchesSegment("evaluation",failed)).toBe(true);const partial={...segment,comparisons:[{stale:false,variants:[{status:"succeeded",rating:{accuracy:5,naturalness:null,accent:5,rhythm:5,emotion:5}}]}]};setFilter("evaluation","unrated");expect(matchesSegment("evaluation",partial)).toBe(true);});
 it("台本工程では読み上げオンオフを元Markdown行で絞り込む",()=>{setFilter("script","enabled");expect(matchesSource({line:1,text:"本文",raw:"本文",enabled:true,kind:0})).toBe(true);expect(matchesSource({line:2,text:"対象外",raw:"対象外",enabled:false,kind:0})).toBe(false);});
});

it('録音の完了を工程名に表示できる',()=>{
 expect(renderStageBar('recording',true)).toContain('録音（完了）');
 expect(renderStageBar('recording',false)).not.toContain('（完了）');
});


describe("0.0.5.6の対象と階層",()=>{
 beforeEach(()=>resetNavigation(crypto.randomUUID()));
 it("採用なしを未収録へ統合し、変更は本文の差で判定する",()=>{
  expect(optionsFor("recording").map(([key])=>key)).toEqual(["all","reading-target","unrecorded","review","disabled"]);
  setFilter("recording","unrecorded");expect(matchesSegment("recording",{...segment,takes:[{status:"ready"}]})).toBe(true);
  expect(matchesSegment("recording",{...segment,selected:"t"})).toBe(false);
  setFilter("recording","review");expect(matchesSegment("recording",{...segment,needs_review:true,recording_draft:segment.text})).toBe(false);
  expect(matchesSegment("recording",{...segment,recording_draft:"修正"})).toBe(true);
  setFilter("recording","disabled");expect(renderSegmentTree([{...segment,skip:true}],"recording","")).toContain("－");
 });
 const line=(id:string,hierarchy:number[],source_id="a")=>({...segment,id,hierarchy,source_id});
 const titles=(items:any[])=>{document.body.innerHTML=renderSegmentTree(items,"recording","");return [...document.querySelectorAll("summary")].map(el=>el.textContent);};
 it("各ファイルの同じ部番号を結合せず、部を通算する",()=>{
  expect(titles([line("a",[1,0,0]),line("b",[2,0,0]),line("c",[1,0,0],"b")])).toEqual(["第1部","第2部","第3部"]);
 });
 it("下位見出しのある親だけ0階層を作り、見出しのない章は本文へ直結する",()=>{
  expect(titles([line("p",[0,0,0]),line("a",[1,0,0]),line("b",[1,1,0]),line("c",[1,1,1]),line("d",[1,2,0]),line("e",[2,0,0])])).toEqual(["第0部","第1部","第0章","第1章","第0節","第1節","第2章","第2部"]);
 });
 it("部や章を飛ばした見出しでも架空の親は作らない",()=>{
  expect(titles([line("a",[0,0,0])])).toEqual([]);
  expect(titles([line("a",[1,0,0]),line("b",[1,0,1])])).toEqual(["第1部","第0節","第1節"]);
 });
 it("絞り込みで見出し行が消えても0章の文脈を維持する",()=>{
  setFilter("recording","unrecorded");expect(titles([line("a",[1,0,0]),{...line("b",[1,1,0]),selected:"t"}])).toEqual(["第1部","第0章"]);
 });
 it("状態は未採用と未収録が白丸、採用済みが黒丸",()=>{
  const html=renderSegmentTree([segment,{...segment,id:"recorded",takes:[{status:"ready"}]},{...segment,id:"selected",selected:"t"}],"recording","");
  expect(html.match(/<i>○<\/i>/g)).toHaveLength(2);expect(html).toContain("<i>●</i>");
 });
});


it("確定発話に存在しない対象外を表示専用行として補い、録音順と元データを保持する",()=>{
 resetNavigation(crypto.randomUUID());
 const active=[{...segment,id:"later",source_id:"a",source_line:2,hierarchy:[1,0,0]},{...segment,id:"first",source_id:"a",source_line:0,hierarchy:[1,0,0]}];
 const sources=[{id:"a",lines:[{line:0,text:"先",enabled:true},{line:1,text:"対象外",enabled:false,hierarchy:[1,0,0]},{line:2,text:"後",enabled:true},{line:3,text:"",enabled:false}],draft_lines:[{line:0,reading_order:2},{line:1,reading_order:1},{line:2,reading_order:0}]}];
 const before=JSON.stringify({active,sources});const rows=recordingNavigationRows(active,sources);
 expect(rows.map(row=>row.id)).toEqual(["later","excluded:a:1","first"]);
 expect(rows.filter(row=>!row.skip)).toEqual(active);expect(JSON.stringify({active,sources})).toBe(before);
 setFilter("recording","disabled");const html=renderSegmentTree(rows,"recording","");
 expect(html).toContain("対象外");expect(html).toContain("第1部");expect(html).toContain("<i>－</i>");expect(html).not.toContain('data-segment="later"');
 setFilter("recording","reading-target");expect(renderSegmentTree(rows,"recording","")).not.toContain('data-segment="excluded:');
});
it("全行が対象外の原稿も原稿順で補い、既存skip行は重複させない",()=>{
 const active=[{...segment,id:"skip",source_id:"b",source_line:0,skip:true}];
 const rows=recordingNavigationRows(active,[{id:"a",lines:[{line:0,text:"対象外",enabled:false}]},{id:"b",lines:[{line:0,text:"既存対象外",enabled:false}]}]);
 expect(rows.map(row=>row.id)).toEqual(["excluded:a:0","skip"]);
});
