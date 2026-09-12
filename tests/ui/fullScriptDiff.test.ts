import {expect,it} from 'vitest';
import {reviewRows} from '../../src/scriptReview';
import {fullDiffRows,fullDiffHtml} from '../../src/fullScriptDiff';
it('左右の全文を空行も含めて保ち、同一行と差分の承認位置を揃える',()=>{
 const before='先頭\n\n旧1\n旧2\n末尾\n',after='先頭\n新1\n末尾\n';
 const review=reviewRows(before,after),pairs=fullDiffRows(before,after,review);
 expect(pairs.flatMap(p=>p.old?[p.old.text]:[]).join('\n')).toBe(before);
 expect(pairs.flatMap(p=>p.next?[p.next.text]:[]).join('\n')).toBe(after);
 expect(pairs.find(p=>p.old?.text==='末尾')?.next?.text).toBe('末尾');
 const host=document.createElement('div');host.innerHTML=fullDiffHtml(before,after,review,()=>true);
 expect(host.querySelectorAll('[data-review-index]')).toHaveLength(review.filter(row=>row.kind!=='same').length);
 expect(host.querySelectorAll('[aria-pressed=true]')).toHaveLength(3);
 expect(host.querySelector('.full-diff-gap')).toBeTruthy();
});
it('空原稿からの追加と同文の繰り返しでも全文を欠落させない',()=>{
 for(const [before,after] of [['','追加\n'],['甲\n乙\n甲','甲\n甲\n乙']]){
  const pairs=fullDiffRows(before,after,reviewRows(before,after));
  expect(pairs.flatMap(p=>p.old?[p.old.text]:[]).join('\n')).toBe(before);
  expect(pairs.flatMap(p=>p.next?[p.next.text]:[]).join('\n')).toBe(after);
 }
});
