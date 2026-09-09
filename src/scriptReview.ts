import {scriptDifferences,type Difference} from './scriptDiff';
export type ReviewRow=Difference&{key:string;oldLine?:number;newLine?:number};
export type Decision='accept'|'reject';
export function reviewRows(before:string,after:string):ReviewRow[]{
 const indexes=(text:string)=>text.replace(/\r\n/g,'\n').split('\n').flatMap((s,i)=>s.trim()?[i]:[]);
 const a=indexes(before),b=indexes(after);let i=0,j=0;const occurrences=new Map<string,number>();
 return scriptDifferences(before,after).map(row=>{
  const oldLine=row.kind!=='added'?a[i++]:undefined,newLine=row.kind!=='removed'?b[j++]:undefined;
  const base=JSON.stringify([row.kind,row.text,oldLine]);const n=occurrences.get(base)??0;occurrences.set(base,n+1);
  return {...row,oldLine,newLine,key:base+':'+n};
 });
}
// 空行を比較から除外しつつ、採用元の原文ブロック（空行・字下げ）を保存する。
export function resolveReview(before:string,after:string,rows:ReviewRow[],decisions:Map<string,Decision>){
 const old=before.replace(/\r\n/g,'\n').split('\n'),next=after.replace(/\r\n/g,'\n').split('\n');
 const output:string[]=[],origins:Record<number,number>={};
 for(const row of rows){
  const accepted=decisions.get(row.key)==='accept';
  if(row.kind==='added'&&!accepted||row.kind==='removed'&&accepted)continue;
  const fromOld=row.kind==='removed',source=fromOld?old:next,index=(fromOld?row.oldLine:row.newLine)!;
  let start=index;while(start>0&&!source[start-1].trim())start--;
  output.push(...source.slice(start,index));
  if(row.oldLine!==undefined)origins[output.length]=row.oldLine;
  output.push(row.text);
 }
 return {text:output.join('\n'),origins};
}
