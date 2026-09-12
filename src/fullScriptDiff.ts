import type {ReviewRow} from './scriptReview';
import {escapeHtml as esc} from './html';
export type DiffCell={line:number;text:string;kind:'same'|'added'|'removed';review?:number};
export type DiffPair={old?:DiffCell;next?:DiffCell};
// 同一行を軸に旧／新の物理行を整列する。空行も省略せず本文を再現する。
export function fullDiffRows(before:string,after:string,review:ReviewRow[]):DiffPair[]{
 const old=before?before.replace(/\r\n/g,'\n').split('\n'):[],next=after?after.replace(/\r\n/g,'\n').split('\n'):[];
 const oldKeys=new Map<number,number>(),newKeys=new Map<number,number>();
 review.forEach((row,index)=>{if(row.kind==='removed')oldKeys.set(row.oldLine!,index);if(row.kind==='added')newKeys.set(row.newLine!,index);});
 const result:DiffPair[]=[];let a=0,b=0;
 const block=(endA:number,endB:number)=>{
  while(a<endA||b<endB){
   const left=a<endA?{line:a,text:old[a],kind:'removed' as const,review:oldKeys.get(a++)}:undefined;
   const right=b<endB?{line:b,text:next[b],kind:'added' as const,review:newKeys.get(b++)}:undefined;
   if(left&&right&&left.text===right.text&&left.review===undefined&&right.review===undefined)result.push({old:{...left,kind:'same'},next:{...right,kind:'same'}});
   else result.push({old:left,next:right});
  }
 };
 for(const anchor of review.filter(row=>row.kind==='same')){
  block(anchor.oldLine!,anchor.newLine!);
  result.push({old:{line:a,text:old[a++],kind:'same'},next:{line:b,text:next[b++],kind:'same'}});
 }
 block(old.length,next.length);return result;
}
export function fullDiffHtml(before:string,after:string,review:ReviewRow[],approved:(key:string)=>boolean){
 const cell=(value:DiffCell|undefined)=>!value?'<td class="full-diff-gap" aria-label="対応する行なし"></td>':`<td class="full-diff-${value.kind}"><div class="full-diff-cell"><span class="line-number">${value.line+1}</span><span class="full-diff-text">${esc(value.text)||'&nbsp;'}</span>${value.review!==undefined?`<button data-review-index="${value.review}" aria-label="${value.kind==='removed'?'旧':'新'}原稿${value.line+1}行目を承認" aria-pressed="${approved(review[value.review].key)}">承認</button>`:''}</div></td>`;
 return `<div class="full-diff-scroll" tabindex="0" aria-label="原稿全文の左右比較"><table class="full-diff-table"><thead><tr><th>変更前</th><th>変更後</th></tr></thead><tbody>${fullDiffRows(before,after,review).map(pair=>`<tr>${cell(pair.old)}${cell(pair.next)}</tr>`).join('')}</tbody></table></div>`;
}
