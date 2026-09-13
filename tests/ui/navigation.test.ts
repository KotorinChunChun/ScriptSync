import {beforeEach,describe,expect,it} from "vitest";
import {renderStageBar,renderSegmentTree,renderSourceTree,optionsFor,filterFor,matchesSegment,matchesSource,resetNavigation,setCollapsed,setFilter,isCollapsed,scrollFor,saveScroll} from "../../src/navigation";

const segment:any={id:"s1",text:"原稿",skip:false,selected:null,confirmed:false,takes:[],corrections:[],comparisons:[]};

describe("工程ナビゲーションの状態",()=>{
 beforeEach(()=>resetNavigation(`project-${crypto.randomUUID()}`));
 it("録音は読み上げ対象を既定とし、収録後も表示を維持する",()=>{expect(filterFor("recording")).toBe("reading-target");expect(optionsFor("recording")[0]).toEqual(["reading-target","読み上げ対象"]);const recorded={...segment,takes:[{status:"ready"}],selected:"take1"};expect(matchesSegment("recording",recorded)).toBe(true);expect(renderSegmentTree([recorded],"recording","s1")).toContain('data-segment="s1"');expect(matchesSegment("recording",{...segment,skip:true})).toBe(false);});
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
