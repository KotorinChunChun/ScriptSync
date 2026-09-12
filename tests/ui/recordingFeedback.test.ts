import {expect,it} from 'vitest';
import {feedbackSource} from '../../src/recordingFeedback';
import {makeDraft} from '../../src/scriptSources';
const draft=()=>makeDraft({id:'a',name:'a.md',confirmed_text:'# 題名\n甲\n乙',draft_text:'# 題名\n甲\n乙\n追記',lines:[]});
it('修正全文・複数行・空への削除を正しい位置へ反映し他の下書きを保持する',()=>{
 const result=feedbackSource(draft(),[{id:'1',source_id:'a',source_line:0,recording_draft:'変更題名'},{id:'2',source_id:'a',source_line:1,recording_draft:'変更\n追加'},{id:'3',source_id:'a',source_line:2,recording_draft:''}]);
 expect(result.text).toBe('# 変更題名\n変更\n追加\n\n追記');expect(result.mappings).toEqual([{line:0,id:'1'},{line:1,id:'2'}]);
});
it('台本側でも同じ行が編集されていたら上書きせず停止する',()=>{
 const d=draft();d.text=d.text.replace('甲','既存編集');
 expect(()=>feedbackSource(d,[{id:'2',source_id:'a',source_line:1,recording_draft:'録音編集'}])).toThrow('台本でも編集');
 expect(d.text).toContain('既存編集');
});
