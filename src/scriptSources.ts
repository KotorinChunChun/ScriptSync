import {reviewRows, resolveReview, type Decision, type ReviewRow} from "./scriptReview";

export type ScriptLine={line:number;enabled:boolean;reading_order?:number;previous_line?:number|null;previous_id?:string|null;[key:string]:unknown};
export type ScriptSource={id:string;name:string;kind?:string;confirmed_text:string;draft_text?:string;lines:ScriptLine[];draft_lines?:ScriptLine[];blank_rows?:{segment_id:string;line:number}[];approved_text?:string};
export type SourceDraft={source:ScriptSource;text:string;parsedText:string;lines:ScriptLine[];decisions:Map<string,Decision>;resolved:{text:string;origins:Record<number,number>}};

export function sourceText(source:ScriptSource){return source.draft_text??source.confirmed_text;}
export function makeDraft(source:ScriptSource):SourceDraft{
 const text=sourceText(source),origins=physicalOrigins(source.confirmed_text,text),physical=text.split(/\r?\n/);
 const metadata=source.draft_lines?.length?source.draft_lines:source.lines??[];
 const lines=readingLines(metadata).map(line=>({...line,raw:physical[line.line]??'',text:line.text??physical[line.line]??'',previous_line:line.force_new?null:origins[line.line]??null}));
 const decisions=new Map<string,Decision>();
 if(source.approved_text===text)reviewRows(source.confirmed_text,text).filter(row=>row.kind!=="same").forEach(row=>decisions.set(row.key,"accept"));
 return {source,text,parsedText:text,lines,decisions,resolved:{text,origins}};
}
export function physicalOrigins(before:string,after:string):Record<number,number>{return Object.fromEntries(reviewRows(before,after).filter(row=>row.kind==='same'&&row.newLine!==undefined&&row.oldLine!==undefined).map(row=>[row.newLine!,row.oldLine!]));}
export function updateParsedLines(draft:SourceDraft,parsed:ScriptLine[]){
 const origins=physicalOrigins(draft.source.confirmed_text,draft.text),prior=physicalOrigins(draft.parsedText,draft.text);
 const ordered=draft.lines.some(line=>line.reading_order!==undefined),last=Math.max(-1,...draft.lines.map(line=>line.reading_order??-1));
 draft.lines=parsed.map(line=>{
  const previous=prior[line.line]===undefined?undefined:draft.lines.find(old=>old.line===prior[line.line]);
  const blank=draft.source.blank_rows?.find(row=>row.line===line.line);
  const blankChoice=blank?draft.lines.find(old=>old.previous_id===blank.segment_id):undefined;
  const keep=previous??(draft.parsedText===draft.text?blankChoice:undefined);
  return {...line,...(ordered?{reading_order:keep?.reading_order??last+1+line.line}:{}),enabled:keep?.enabled??line.enabled,previous_line:keep?.force_new?null:origins[line.line]??null,...(keep?.force_new?{force_new:true}:{}),...(keep?.previous_id?{previous_id:keep.previous_id,keep:true,blank_segment_id:keep.blank_segment_id}:{})};
 });
 draft.parsedText=draft.text;draft.resolved={text:draft.text,origins};
}
export function changedRows(draft:SourceDraft):ReviewRow[]{return reviewRows(draft.source.confirmed_text,draft.text);}
export function unresolved(draft:SourceDraft){return changedRows(draft).filter(row=>row.kind!=="same"&&!draft.decisions.has(row.key)).length;}
export function resolveDraft(draft:SourceDraft){return resolveReview(draft.source.confirmed_text,draft.text,changedRows(draft),draft.decisions);}
export function approvedText(draft:SourceDraft){return unresolved(draft)===0?draft.text:undefined;}
export function sourcePayload(draft:SourceDraft){
 return {id:draft.source.id,name:draft.source.name,kind:draft.source.kind,text:draft.text,
  lines:draft.lines.map(line=>({...line,keep:!!line.previous_id,previous_line:line.force_new?null:line.previous_line??null})),
  ...(draft.source.blank_rows?{blank_rows:draft.source.blank_rows}:{}),...(unresolved(draft)===0?{approved_text:draft.text}:{})};
}
export function reorder<T>(values:T[],from:number,to:number){const next=[...values], [item]=next.splice(from,1);next.splice(to,0,item);return next;}

export type SourceSegment={id:string;line_id?:string;source_id?:string;source_line?:number|null;source_text?:string;text:string};
export function importedSource(existing:SourceDraft|undefined,file:{name:string;text:string},id:string):SourceDraft {
 return makeDraft({id,name:file.name,kind:existing?.source.kind??'markdown',confirmed_text:existing?.source.confirmed_text??'',draft_text:file.text,lines:existing?.source.lines??[],draft_lines:existing?.lines??[],blank_rows:existing?.source.blank_rows});
}
export function sourceMapping(draft:SourceDraft,line:ScriptLine,active:SourceSegment[],archived:SourceSegment[]){
 const inSource=active.filter(s=>s.source_id===draft.source.id);
 const automatic=(row:ScriptLine)=>!row.force_new&&row.previous_line!=null?inSource.find(s=>s.source_line===row.previous_line&&(s.source_text??s.text)===String(row.text??row.raw??'')):undefined;
 const matched=automatic(line);
 const used=new Set(draft.lines.filter(row=>row!==line).flatMap(row=>[row.previous_id,automatic(row)?.id]).filter(Boolean));
 const removed=new Set(changedRows(draft).filter(row=>row.kind==='removed').map(row=>row.oldLine));
 const candidates=[...inSource.filter(s=>s.source_line!=null&&removed.has(s.source_line)),...archived.filter(s=>s.source_id===draft.source.id)]
  .filter((s,i,all)=>!used.has(s.id)&&all.findIndex(other=>other.id===s.id)===i);
 const selected=matched?.id??line.previous_id??'';
 const current=[...inSource,...archived].find(s=>s.id===selected);
 if(current&&!candidates.some(s=>s.id===selected))candidates.unshift(current);
 return {matched,selected,candidates};
}

export function readingLines(lines:ScriptLine[]){return [...lines].sort((a,b)=>(a.reading_order??a.line)-(b.reading_order??b.line)||a.line-b.line);}
export function reorderReading(draft:SourceDraft,from:number,to:number){
 const lines=readingLines(draft.lines),a=lines.findIndex(line=>line.line===from),b=lines.findIndex(line=>line.line===to);
 if(a<0||b<0||a===b)return;
 draft.lines=reorder(lines,a,b).map((line,reading_order)=>({...line,reading_order}));
}
