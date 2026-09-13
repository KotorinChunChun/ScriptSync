import {expect,it} from 'vitest';
import {aiReviewHtml,improvementDiff} from '../../src/aiReview';
it('語の置換を候補にし、追加のみは登録しない',()=>{
 expect(improvementDiff('Use codex today.','Use Codex today.').candidates).toEqual([{from:'codex',to:'Codex'}]);
 expect(improvementDiff('これは本文です。','これは追加の本文です。').candidates).toEqual([]);
});
it('左右差分はHTMLを実行せず改行を保持する',()=>{
 const result=improvementDiff('<script>\nAlpha','<script>\nBeta');
 expect(result.left).toContain('&lt;');expect(result.left).toContain('\n');expect(result.right).toContain('<ins>Beta</ins>');
});
it('単語候補を初期未選択として承認と取消を表示する',()=>{
 const html=aiReviewHtml({key:'x',before:'codex',after:'Codex',prepared:'codex',editVersion:0,viewVersion:0,candidates:[{from:'codex',to:'Codex'}],checked:[]});
 expect(html).toContain('改善前の修正案');expect(html).toContain('approve_ai');expect(html).toContain('discard_ai');expect(html).not.toContain(' checked');
});
