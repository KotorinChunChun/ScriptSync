import {expect,it} from "vitest";
import {scriptDifferences} from "../../src/scriptDiff";
it("先頭挿入で既存の全文を変更扱いにしない",()=>{expect(scriptDifferences("甲\n乙","追加\n甲\n乙")).toEqual([{kind:"added",text:"追加"},{kind:"same",text:"甲"},{kind:"same",text:"乙"}]);});
it("本文変更を旧文と新文で返す",()=>{expect(scriptDifferences("前\n旧文\n後","前\n新文\n後").filter(r=>r.kind!=="same")).toEqual([{kind:"removed",text:"旧文"},{kind:"added",text:"新文"}]);});
