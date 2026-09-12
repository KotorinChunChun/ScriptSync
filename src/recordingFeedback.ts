import {physicalOrigins,type SourceDraft} from './scriptSources';
export type Feedback={id:string;source_id?:string;source_line?:number|null;recording_draft?:string|null};
/** 既存の台本下書きを保持し、一致する原稿位置だけ置換する。 */
export function feedbackSource(draft:SourceDraft,segments:Feedback[]){
 const pending=segments.filter(s=>s.source_id===draft.source.id&&s.recording_draft!=null);
 const origins=physicalOrigins(draft.source.confirmed_text,draft.text);
 const replacements=new Map<number,Feedback>();
 for(const segment of pending){
  const entry=Object.entries(origins).find(([,old])=>old===segment.source_line);
  if(!entry)throw new Error(`${draft.source.name}: 修正元の行が台本でも編集されています。録音画面の修正原稿を確認してください`);
  replacements.set(Number(entry[0]),segment);
 }
 const mappings:{line:number;id:string}[]=[];const output:string[]=[];
 draft.text.split(/\r?\n/).forEach((raw,line)=>{
  const replacement=replacements.get(line);
  if(!replacement){output.push(raw);return;}
  const prefix=raw.match(/^\s*#{1,4}\s+/)?.[0]??'';
  const text=replacement.recording_draft!;
  if(text.trim())mappings.push({line:output.length,id:replacement.id});
  output.push(...(text.trim()?prefix+text:'').split(/\r?\n/));
 });
 return {text:output.join('\n'),mappings,feedback:pending.map(s=>({id:s.id,text:s.recording_draft!}))};
}
