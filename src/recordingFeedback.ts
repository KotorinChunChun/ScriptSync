import {physicalOrigins,type SourceDraft} from './scriptSources';
export type Feedback={id:string;source_id?:string;source_line?:number|null;recording_draft?:string|null};
/** 既存の台本下書きを保持し、一致する原稿位置だけ置換する。 */
export function feedbackSource(draft:SourceDraft,segments:Feedback[]){
 const pending=segments.filter(s=>s.source_id===draft.source.id&&s.recording_draft!=null);
 // 保存済み修正案を再起動後に再処理する場合、同じ変更を二重適用しない。
 if(pending.length&&pending.every(segment=>segment.source_line!=null)&&draft.text!==draft.source.confirmed_text){
  const original=draft.source.confirmed_text.split(/\r?\n/),replayed:string[]=[],mappings:{line:number;id:string;reading_order?:number}[]=[];
  original.forEach((raw,line)=>{const segment=pending.find(item=>item.source_line===line);if(!segment){replayed.push(raw);return;}const text=segment.recording_draft!,prefix=raw.match(/^\s*#{1,4}\s+/)?.[0]??'';if(text.trim())mappings.push({line:replayed.length,id:segment.id});replayed.push(...(text.trim()?prefix+text:'').split(/\r?\n/));});
  if(replayed.join('\n')===draft.text)return {text:draft.text,mappings:mappings.map(mapping=>({...mapping,...(draft.lines.find(line=>line.line===mapping.line)?.reading_order!==undefined?{reading_order:draft.lines.find(line=>line.line===mapping.line)!.reading_order}:{})})),feedback:pending.map(segment=>({id:segment.id,text:segment.recording_draft!}))};
 }
 const alreadyApplied=new Map<number,Feedback>();
 const origins=physicalOrigins(draft.source.confirmed_text,draft.text);
 const replacements=new Map<number,Feedback>();
 for(const segment of pending){
  const blank=draft.source.blank_rows?.find(row=>row.segment_id===segment.id);
  if(blank){
   const current=draft.text.split(/\r?\n/)[blank.line];
   if(current===undefined||(current.trim()&&current!==segment.recording_draft))throw new Error(`${draft.source.name}: 空行の修正案が台本でも編集されています`);
   replacements.set(blank.line,segment);continue;
  }
  const saved=draft.lines.find(line=>line.previous_id===segment.id);
  if(saved){const original=draft.source.confirmed_text.split(/\r?\n/)[segment.source_line??-1]??'',prefix=original.match(/^\s*#{1,4}\s+/)?.[0]??'',expected=(segment.recording_draft!.trim()?prefix+segment.recording_draft!:'').split(/\r?\n/);if(draft.text.split(/\r?\n/).slice(saved.line,saved.line+expected.length).join('\n')===expected.join('\n')){alreadyApplied.set(saved.line,segment);continue;}}
  const entry=Object.entries(origins).find(([,old])=>old===segment.source_line);
  if(!entry)throw new Error(`${draft.source.name}: 修正元の行が台本でも編集されています。録音画面の修正原稿を確認してください`);
  replacements.set(Number(entry[0]),segment);
 }
 const mappings:{line:number;id:string;reading_order?:number}[]=[];const output:string[]=[];
 draft.text.split(/\r?\n/).forEach((raw,line)=>{
  const applied=alreadyApplied.get(line);if(applied&&applied.recording_draft?.trim())mappings.push({line:output.length,id:applied.id,...(draft.lines.find(row=>row.line===line)?.reading_order!==undefined?{reading_order:draft.lines.find(row=>row.line===line)!.reading_order}:{})});
  const replacement=replacements.get(line);
  if(!replacement){output.push(raw);return;}
  const prefix=raw.match(/^\s*#{1,4}\s+/)?.[0]??'';
  const text=replacement.recording_draft!;
  if(text.trim())mappings.push({line:output.length,id:replacement.id,...(draft.lines.find(row=>row.line===line)?.reading_order!==undefined?{reading_order:draft.lines.find(row=>row.line===line)!.reading_order}:{})});
  output.push(...(text.trim()?prefix+text:'').split(/\r?\n/));
 });
 return {text:output.join('\n'),mappings,feedback:pending.map(s=>({id:s.id,text:s.recording_draft!}))};
}
