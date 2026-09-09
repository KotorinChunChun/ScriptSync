import {expect,it} from 'vitest';
import {reviewRows,resolveReview,type Decision} from '../../src/scriptReview';
it('空白行の増減を候補にせず、保持元の行を追跡する',()=>{
 const rows=reviewRows('甲\n\n乙','\n甲\n \n\n乙\n');
 expect(rows.map(r=>r.kind)).toEqual(['same','same']);
 expect(rows.map(r=>r.oldLine)).toEqual([0,2]);
});
it('2本目の原稿を読み込んでも未承認の削除で既存原稿は消えない',()=>{
 const before='# 第1章\n本文A',after='# 第2章\n本文B',rows=reviewRows(before,after);
 const decisions=new Map<string,Decision>();
 expect(resolveReview(before,after,rows,decisions).text).toBe(before);
 rows.forEach(r=>decisions.set(r.key,r.kind==='added'?'accept':'reject'));
 const result=resolveReview(before,after,rows,decisions);
 expect(result.text).toBe(before+'\n'+after);expect(result.origins).toEqual({0:0,1:1});
});
it('本文の変更は削除と追加を独立に選べる',()=>{
 const before='前\n旧\n後',after='前\n新\n後',rows=reviewRows(before,after),decisions=new Map<string,Decision>();
 rows.filter(r=>r.kind!=='same').forEach(r=>decisions.set(r.key,'accept'));
 expect(resolveReview(before,after,rows,decisions)).toEqual({text:after,origins:{0:0,2:2}});
});
it('同じ新規文が複数あっても承認を別々に扱う',()=>{
 const rows=reviewRows('','重複\n重複');expect(rows[0].key).not.toBe(rows[1].key);
 expect(resolveReview('','重複\n重複',rows,new Map([[rows[1].key,'accept']])).text).toBe('重複');
});
