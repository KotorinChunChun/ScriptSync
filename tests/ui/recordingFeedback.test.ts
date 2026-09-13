import {expect,it} from 'vitest';
import {feedbackSource} from '../../src/recordingFeedback';
import {makeDraft} from '../../src/scriptSources';
const draft=()=>makeDraft({id:'a',name:'a.md',confirmed_text:'# 題名\n甲\n乙',draft_text:'# 題名\n甲\n乙\n追記',lines:[]});
it('未確定空行の手編集と自動文字起こしを同じ行へ反映し確定本文を保持する',()=>{
 for(const text of ['甲\n\n乙','甲\n認識結果\n乙']){
  const d=makeDraft({id:'a',name:'a.md',confirmed_text:'甲\n乙',draft_text:text,lines:[],blank_rows:[{segment_id:'blank',line:1}]});
  const result=feedbackSource(d,[{id:'blank',source_id:'a',source_line:null,recording_draft:'認識結果'}]);
  expect(result.text).toBe('甲\n認識結果\n乙');expect(result.mappings).toEqual([{line:1,id:'blank'}]);expect(d.source.confirmed_text).toBe('甲\n乙');
 }
});
it('修正全文・複数行・空への削除を正しい位置へ反映し他の下書きを保持する',()=>{
 const result=feedbackSource(draft(),[{id:'1',source_id:'a',source_line:0,recording_draft:'変更題名'},{id:'2',source_id:'a',source_line:1,recording_draft:'変更\n追加'},{id:'3',source_id:'a',source_line:2,recording_draft:''}]);
 expect(result.text).toBe('# 変更題名\n変更\n追加\n\n追記');expect(result.mappings).toEqual([{line:0,id:'1'},{line:1,id:'2'}]);
});
it('台本側でも同じ行が編集されていたら上書きせず停止する',()=>{
 const d=draft();d.text=d.text.replace('甲','既存編集');
 expect(()=>feedbackSource(d,[{id:'2',source_id:'a',source_line:1,recording_draft:'録音編集'}])).toThrow('台本でも編集');
 expect(d.text).toContain('既存編集');
});

it('録音で修正した発話の読み上げ順を引き継ぐ',()=>{
 const d=draft();d.lines=[{line:0,enabled:true,reading_order:2},{line:1,enabled:true,reading_order:0},{line:2,enabled:true,reading_order:1}];
 expect(feedbackSource(d,[{id:'2',source_id:'a',source_line:1,recording_draft:'修正'}]).mappings).toEqual([{line:1,id:'2',reading_order:0}]);
});

it('保存済みの複数行修正と削除を再起動後も二重適用しない',()=>{
 const segments=[{id:'1',source_id:'a',source_line:0,recording_draft:'変更題名'},{id:'2',source_id:'a',source_line:1,recording_draft:'変更\n追加'},{id:'3',source_id:'a',source_line:2,recording_draft:''}];
 const original=makeDraft({id:'a',name:'a.md',confirmed_text:'# 題名\n甲\n乙',lines:[]});const first=feedbackSource(original,segments);
 const restored=makeDraft({...original.source,draft_text:first.text});expect(feedbackSource(restored,segments)).toEqual(first);
});
it('保存済み発話IDと全文一致で再反映を避け、他の追記を保持する',()=>{
 const source={id:'a',name:'a.md',confirmed_text:'甲\n乙',draft_text:'変更\n追加\n乙\n別の追記',lines:[],draft_lines:[{line:0,enabled:true,previous_id:'s',reading_order:3},{line:1,enabled:true},{line:2,enabled:true}]};
 const restored=makeDraft(source);const result=feedbackSource(restored,[{id:'s',source_id:'a',source_line:0,recording_draft:'変更\n追加'}]);expect(result.text).toBe(source.draft_text);expect(result.mappings).toEqual([{line:0,id:'s',reading_order:3}]);
});
it('保存済み発話IDがあっても反映後に台本側で編集した本文は上書きしない',()=>{
 const restored=makeDraft({id:'a',name:'a.md',confirmed_text:'甲\n乙',draft_text:'別の編集\n乙',lines:[],draft_lines:[{line:0,enabled:true,previous_id:'s'}]});
 expect(()=>feedbackSource(restored,[{id:'s',source_id:'a',source_line:0,recording_draft:'変更'}])).toThrow('台本でも編集');expect(restored.text).toBe('別の編集\n乙');
});
