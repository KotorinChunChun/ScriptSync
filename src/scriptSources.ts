import {reviewRows, resolveReview, type Decision, type ReviewRow} from "./scriptReview";

export type ScriptLine={line:number;enabled:boolean;previous_line?:number|null;previous_id?:string|null;[key:string]:unknown};
export type ScriptSource={id:string;name:string;kind?:string;confirmed_text:string;draft_text?:string;lines:ScriptLine[];draft_lines?:ScriptLine[];blank_rows?:{segment_id:string;line:number}[];approved_text?:string};
export type SourceDraft={source:ScriptSource;text:string;parsedText:string;lines:ScriptLine[];decisions:Map<string,Decision>;resolved:{text:string;origins:Record<number,number>}};

export function sourceText(source:ScriptSource){return source.draft_text??source.confirmed_text;}
export function makeDraft(source:ScriptSource):SourceDraft{
 const text=sourceText(source),origins=physicalOrigins(source.confirmed_text,text),physical=text.split(/\r?\n/);
 const metadata=source.draft_lines?.length?source.draft_lines:source.lines??[];
 const lines=metadata.map(line=>({...line,raw:physical[line.line]??'',text:line.text??physical[line.line]??'',previous_line:origins[line.line]??null}));
 const decisions=new Map<string,Decision>();
 if(source.approved_text===text)reviewRows(source.confirmed_text,text).filter(row=>row.kind!=="same").forEach(row=>decisions.set(row.key,"accept"));
 return {source,text,parsedText:text,lines,decisions,resolved:{text,origins}};
}
export function physicalOrigins(before:string,after:string):Record<number,number>{return Object.fromEntries(reviewRows(before,after).filter(row=>row.kind==='same'&&row.newLine!==undefined&&row.oldLine!==undefined).map(row=>[row.newLine!,row.oldLine!]));}
export function updateParsedLines(draft:SourceDraft,parsed:ScriptLine[]){
 const origins=physicalOrigins(draft.source.confirmed_text,draft.text),prior=physicalOrigins(draft.parsedText,draft.text);
 draft.lines=parsed.map(line=>{
  const previous=prior[line.line]===undefined?undefined:draft.lines.find(old=>old.line===prior[line.line]);
  const blank=draft.source.blank_rows?.find(row=>row.line===line.line);
  const blankChoice=blank?draft.lines.find(old=>old.previous_id===blank.segment_id):undefined;
  const keep=previous??(draft.parsedText===draft.text?blankChoice:undefined);
  return {...line,enabled:keep?.enabled??line.enabled,previous_line:origins[line.line]??null,...(keep?.previous_id?{previous_id:keep.previous_id,keep:keep.keep,blank_segment_id:keep.blank_segment_id}:{})};
 });
 draft.parsedText=draft.text;draft.resolved={text:draft.text,origins};
}
export function changedRows(draft:SourceDraft):ReviewRow[]{return reviewRows(draft.source.confirmed_text,draft.text);}
export function unresolved(draft:SourceDraft){return changedRows(draft).filter(row=>row.kind!=="same"&&!draft.decisions.has(row.key)).length;}
export function resolveDraft(draft:SourceDraft){return resolveReview(draft.source.confirmed_text,draft.text,changedRows(draft),draft.decisions);}
export function approvedText(draft:SourceDraft){return unresolved(draft)===0?draft.text:undefined;}
export function sourcePayload(draft:SourceDraft){
 return {id:draft.source.id,name:draft.source.name,kind:draft.source.kind,text:draft.text,
  lines:draft.lines.map(line=>({...line,previous_line:line.previous_line??null})),
  ...(draft.source.blank_rows?{blank_rows:draft.source.blank_rows}:{}),...(unresolved(draft)===0?{approved_text:draft.text}:{})};
}
export function reorder<T>(values:T[],from:number,to:number){const next=[...values], [item]=next.splice(from,1);next.splice(to,0,item);return next;}
