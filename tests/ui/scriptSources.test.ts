import {expect,it} from 'vitest';
import {changedRows,makeDraft,reorder,resolveDraft,sourcePayload,unresolved,updateParsedLines} from '../../src/scriptSources';

it('ファイルごとに下書きと承認状態を保持する',()=>{
 const draft=makeDraft({id:'a',name:'a.md',confirmed_text:'旧',draft_text:'新',lines:[]});
 expect(unresolved(draft)).toBe(2);
 const rows=resolveDraft(draft);
 expect(rows.text).toBe('旧');
 for(const row of changedRows(draft).filter(row=>row.kind!=='same'))draft.decisions.set(row.key,'accept');
 expect(sourcePayload(draft).approved_text).toBe('新');
});
it('ドラッグしたファイル順を保存順へ反映する',()=>expect(reorder(['a','b','c'],2,0)).toEqual(['c','a','b']));
it('未変更の既存行は初回保存でも元の物理行を保持する',()=>{
 const draft=makeDraft({id:'a',name:'a.md',confirmed_text:'甲\n乙',lines:[{line:0,enabled:false},{line:1,enabled:true}]});
 expect(sourcePayload(draft).lines.map(line=>line.previous_line)).toEqual([0,1]);
});
it('先頭追加で後続行の対象選択を盗まない',()=>{
 const draft=makeDraft({id:'a',name:'a.md',confirmed_text:'甲\n乙',lines:[{line:0,enabled:false},{line:1,enabled:true}]});
 draft.text='新\n甲\n乙';
 updateParsedLines(draft,[{line:0,enabled:true,text:'新'},{line:1,enabled:true,text:'甲'},{line:2,enabled:true,text:'乙'}]);
 expect(draft.lines[1].enabled).toBe(false);
 expect(draft.lines[0].previous_id).toBeUndefined();
});
it('ASR補完後の下書きを解析しても空録音行のIDとテイク維持指定を保つ',()=>{
 const draft=makeDraft({id:'a',name:'a.md',confirmed_text:'甲',draft_text:'甲\n認識結果',lines:[{line:0,enabled:true}],draft_lines:[{line:0,enabled:true},{line:1,enabled:true,previous_id:'blank',keep:true,blank_segment_id:'blank'}],blank_rows:[{segment_id:'blank',line:1}]});
 updateParsedLines(draft,[{line:0,enabled:true,text:'甲'},{line:1,enabled:true,text:'認識結果'}]);
 expect(sourcePayload(draft).lines[1]).toMatchObject({previous_id:'blank',keep:true,previous_line:null});
});
