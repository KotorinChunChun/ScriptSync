//! アーカイブの本文・専用音声を全保存世代から除去する。
use crate::{domain::*, storage::*};
use serde::{Serialize,Deserialize};
use serde_json::{json,Value};
use std::{collections::HashSet,fs,path::Path};

fn files(s:&Segment)->Vec<String>{
    let mut paths:Vec<String>=s.takes.iter().map(|t|t.file.clone())
        .chain(s.comparisons.iter().flat_map(|r|r.variants.iter().map(|v|v.file.clone())))
        .filter(|f|!f.is_empty()).collect();
    for run in &s.transcriptions {
        if let Some(run)=run["id"].as_str().filter(|v|uuid::Uuid::parse_str(v).is_ok()){
            paths.push(format!("asr/{run}/result.json"));paths.push(format!("asr/{run}/stderr.txt"));
        }
    }
    paths
}
#[derive(Clone,Serialize,Deserialize,PartialEq,Eq,Hash)]
struct RemovedRow { source_id:String, line:Option<usize>, text:String }
fn has_reference(value:&Value,ids:&HashSet<String>)->bool{
    match value {
        Value::String(s)=>ids.contains(s),
        Value::Array(a)=>a.iter().any(|v|has_reference(v,ids)),
        Value::Object(o)=>o.values().any(|v|has_reference(v,ids)),
        _=>false,
    }
}
fn clean_text(text:&str,source_id:&str,rows:&[RemovedRow],protected:&[Segment])->Result<String>{
    let parsed=crate::markdown::parse(text)?;
    let mut physical:Vec<String>=text.split(char::from(10)).map(str::to_owned).collect();
    for line in parsed {
        let owned=rows.iter().any(|r|r.source_id==source_id&&r.line==Some(line.line)&&r.text==line.text);
        let still_used=protected.iter().any(|s|s.source_id==source_id && s.source_line==Some(line.line) && s.source_text==line.text);
        if owned&&!still_used { physical[line.line].clear(); }
    }
    Ok(physical.join("\n"))
}
fn scrub_project(p:&mut Project,ids:&HashSet<String>,source_id:Option<&str>,rows:&[RemovedRow])->Result<()>{
    let protected:Vec<_>=p.segments.iter().chain(&p.archived_segments).filter(|s|!ids.contains(&s.id)).cloned().collect();
    p.segments.retain(|s|!ids.contains(&s.id));
    p.archived_segments.retain(|s|!ids.contains(&s.id));
    p.script_sources.retain(|s|Some(s.id.as_str())!=source_id);
    p.archived_sources.retain(|s|Some(s.id.as_str())!=source_id);
    p.pending_source_deletions.retain(|id|Some(id.as_str())!=source_id);
    for source in p.script_sources.iter_mut().chain(&mut p.archived_sources){
        let same_draft=source.draft_text==source.confirmed_text;
        source.confirmed_text=clean_text(&source.confirmed_text,&source.id,rows,&protected)?;
        let mut draft_targets=rows.to_vec();
        for row in &source.draft_lines {
            let prior=row["previous_line"].as_u64().map(|n|n as usize);
            if row["previous_id"].as_str().is_some_and(|id|ids.contains(id)) || rows.iter().any(|r|r.source_id==source.id&&r.line.is_some()&&r.line==prior) {
                if let Some(line)=row["line"].as_u64(){
                    if let Some(parsed)=crate::markdown::parse(&source.draft_text)?.into_iter().find(|r|r.line==line as usize){
                        draft_targets.push(RemovedRow{source_id:source.id.clone(),line:Some(line as usize),text:parsed.text});
                    }
                }
            }
        }
        // 下書きは行の移動があるため、一致本文を持つ現役行も保護する。
        let draft_protected:Vec<_>=protected.iter().flat_map(|s|{
            source.draft_lines.iter().filter(move |line|line["previous_id"]==s.id || s.source_line.is_some()&&line["previous_line"].as_u64()==s.source_line.map(|v|v as u64))
                .filter_map(|line|line["line"].as_u64().map(|n|{let mut copy=s.clone();copy.source_line=Some(n as usize);copy}))
        }).collect();
        let draft_guard=if same_draft { &protected } else { &draft_protected };
        source.draft_text=clean_text(&source.draft_text,&source.id,&draft_targets,draft_guard)?;
        if let Some(approved)=&source.approved_text { source.approved_text=Some(clean_text(approved,&source.id,&draft_targets,draft_guard)?); }
        let previous=source.lines.clone();
        source.lines=crate::markdown::parse(&source.confirmed_text)?;
        for line in &mut source.lines {if let Some(old)=previous.iter().find(|r|r.line==line.line){line.enabled=old.enabled;}}
        let draft_rows=crate::markdown::parse(&source.draft_text)?;
        source.draft_lines.retain(|row|!has_reference(row,ids)&&draft_rows.iter().any(|line|row["line"].as_u64()==Some(line.line as u64)));
        for row in &mut source.draft_lines {
            if let Some(line)=draft_rows.iter().find(|line|row["line"].as_u64()==Some(line.line as u64)){
                if row.get("text").is_some(){row["text"]=line.text.clone().into();}
                if row.get("raw").is_some(){row["raw"]=line.raw.clone().into();}
            }
        }
        source.blank_rows.retain(|r|!ids.contains(&r.segment_id));
    }
    p.match_history.retain(|r|!has_reference(r,ids));
    p.batches.iter_mut().for_each(|batch|batch.items.retain(|item|!has_reference(&item.payload,ids)));
    if p.merge_draft.as_ref().is_some_and(|draft|draft.fixed_matches.iter().any(|r|has_reference(r,ids))||draft.proposed_order.iter().any(|id|ids.contains(id))){
        p.merge_draft=None;
    }
    p.source_markdown=p.script_sources.iter().map(|s|s.confirmed_text.as_str()).collect::<Vec<_>>().join("\n\n");
    p.script_lines=if p.source_markdown.len()<=2_000_000 {crate::markdown::parse(&p.source_markdown)?}else{vec![]};
    Ok(())
}
#[cfg(test)]
fn scrub(p:&mut Project,target:&str){
    let ids=HashSet::from([target.to_owned()]);
    let rows=p.segments.iter().chain(&p.archived_segments).filter(|s|s.id==target).map(|s|RemovedRow{source_id:s.source_id.clone(),line:s.source_line,text:s.source_text.clone()}).collect::<Vec<_>>();
    scrub_project(p,&ids,None,&rows).unwrap();
}
fn atomic_json(path:&Path,value:&Project)->Result<()>{
    let temp=path.with_extension(format!("{}.purge-tmp",id()));
    json_new(&temp,value)?;
    for attempt in 0..6 {
        match fs::rename(&temp,path) {
            Ok(()) => return Ok(()),
            Err(error) if attempt < 5 && matches!(error.raw_os_error(),Some(5|32|33)) => {
                std::thread::sleep(std::time::Duration::from_millis(30*(attempt+1)));
            }
            Err(error) => {let _=fs::remove_file(&temp);return Err(format!("保存世代の消去反映に失敗しました（{}）: {error}",path.display()));}
        }
    }
    unreachable!()
}
#[cfg(test)]
pub fn purge(store:&mut Store,target:&str,confirmed:bool)->Result<()>{
    purge_target(store,target,false,confirmed).map(|_|())
}
pub fn purge_target(store:&mut Store,target:&str,is_source:bool,confirmed:bool)->Result<Value>{
    if !confirmed{return Err("完全消去の確認が必要です".into());}
    uuid::Uuid::parse_str(target).map_err(|_|"消去対象IDが不正です")?;
    let key=if is_source{"source_id"}else{"segment_id"};
    let prior=store.project.deletion_records.iter().find(|r|r[key]==target).cloned();
    if prior.as_ref().is_some_and(|r|r["status"]=="done"){return Ok(json!({"shared_files":0}));}
    if is_source {
        if store.project.script_sources.iter().any(|s|s.id==target)||store.project.segments.iter().any(|s|s.source_id==target){return Err("使用中の原稿は完全消去できません".into());}
        if prior.is_none()&&!store.project.archived_sources.iter().any(|s|s.id==target){return Err("削除済み原稿がありません".into());}
    }else{
        if store.project.segments.iter().any(|s|s.id==target){return Err("現役の発話は完全消去できません".into());}
        if prior.is_none()&&!store.project.archived_segments.iter().any(|s|s.id==target){return Err("廃棄済み発話がありません".into());}
    }
    let dir=safe(&store.root,"revisions")?;
    let mut snapshots=Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e|e.to_string())?{
        let path=entry.map_err(|e|e.to_string())?.path();
        if path.extension().and_then(|s|s.to_str())!=Some("json"){continue;}
        let p:Project=serde_json::from_slice(&fs::read(&path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        snapshots.push((path,p));
    }
    let mut ids:HashSet<String>=if is_source{HashSet::new()}else{HashSet::from([target.to_owned()])};
    if let Some(record)=&prior {if let Some(saved)=record["segment_ids"].as_array(){ids.extend(saved.iter().filter_map(|v|v.as_str().map(str::to_owned)));}}
    if is_source{
        for p in snapshots.iter().map(|(_,p)|p).chain(std::iter::once(&store.project)){
            ids.extend(p.segments.iter().chain(&p.archived_segments).filter(|s|s.source_id==target).map(|s|s.id.clone()));
        }
    }
    if store.project.segments.iter().any(|s|ids.contains(&s.id)){
        return Err("別の原稿へ対応付け済みの発話が含まれるため完全消去できません".into());
    }
    if store.project.script_sources.iter().flat_map(|s|&s.draft_lines).any(|row|row["previous_id"].as_str().is_some_and(|id|ids.contains(id))){
        return Err("下書きで対応付け中です。対応を解除または台本確定してから消去してください".into());
    }
    let mut owned=HashSet::new();let mut shared=HashSet::new();let mut row_set=HashSet::new();
    if let Some(record)=&prior{
        if let Some(paths)=record["files"].as_array(){owned.extend(paths.iter().filter_map(|v|v.as_str().map(str::to_owned)));}
        if let Some(rows)=record.get("rows"){row_set.extend(serde_json::from_value::<Vec<RemovedRow>>(rows.clone()).map_err(|e|e.to_string())?);}
    }
    for p in snapshots.iter().map(|(_,p)|p).chain(std::iter::once(&store.project)){
        for s in p.segments.iter().chain(&p.archived_segments){
            if ids.contains(&s.id){
                owned.extend(files(s));
                row_set.insert(RemovedRow{source_id:s.source_id.clone(),line:s.source_line,text:s.source_text.clone()});
            }else{shared.extend(files(s));}
        }
        for record in &p.deletion_records{
            if record[key]!=target&&record["status"]=="pending"{
                if let Some(paths)=record["files"].as_array(){shared.extend(paths.iter().filter_map(|v|v.as_str().map(str::to_owned)));}
            }
        }
    }
    let rows:Vec<_>=row_set.into_iter().collect();
    let exclusive:Vec<_>=owned.difference(&shared).cloned().collect();
    let shared_count=owned.intersection(&shared).count();
    for file in &exclusive{safe(&store.root,file)?;}
    // 全世代を先に構築・検証してからpendingを公開する。
    let source_id=if is_source{Some(target)}else{None};
    for (_,p) in &mut snapshots{scrub_project(p,&ids,source_id,&rows)?;p.deletion_records.retain(|r|r[key]!=target);}
    let before=store.project.clone();
    let mut current=before.clone();scrub_project(&mut current,&ids,source_id,&rows)?;
    current.script_revision=current.script_revision.saturating_add(1);
    current.deletion_records.retain(|r|r[key]!=target);
    let mut record=json!({"status":"pending","files":exclusive,"segment_ids":ids,"rows":rows,"created":now()});
    record[key]=target.into();current.deletion_records.push(record);store.project=current;
    if let Err(e)=store.save(){store.project=before;return Err(e);}
    for (path,p) in snapshots{atomic_json(&path,&p)?;}
    for file in &exclusive{let path=safe(&store.root,file)?;if path.exists(){fs::remove_file(&path).map_err(|e|format!("消去が未完了です。再実行できます: {e}"))?;}}
    // pendingを含む新しい世代にも対象本文（再試行用rows）を残さない。
    let mut done=store.project.clone();
    if let Some(record)=done.deletion_records.iter_mut().find(|r|r[key]==target){record["status"]="done".into();record["files"]=json!([]);record["rows"]=json!([]);}
    for entry in fs::read_dir(&dir).map_err(|e|e.to_string())?{
        let path=entry.map_err(|e|e.to_string())?.path();
        if path.extension().and_then(|s|s.to_str())!=Some("json"){continue;}
        let mut p:Project=serde_json::from_slice(&fs::read(&path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        if p.deletion_records.iter().any(|r|r[key]==target){
            for record in p.deletion_records.iter_mut().filter(|r|r[key]==target){record["status"]="done".into();record["files"]=json!([]);record["rows"]=json!([]);}
            atomic_json(&path,&p)?;
        }
    }
    store.project=done;store.save()?;atomic_json(&safe(&store.root,"project.json")?,&store.project)?;
    Ok(json!({"shared_files":shared_count}))
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn purge_scrubs_all_revisions_and_keeps_shared_audio(){
        let root=std::env::temp_dir().join(id());let mut store=Store::open(root.clone(),true).unwrap();
        let file="takes/shared/audio.wav";write_new(&safe(&root,file).unwrap(),b"shared").unwrap();
        let mut old=Segment::new("古い本文".into());let target=old.id.clone();
        let take=Take{script_text:String::new(),trim_start:0.,trim_end:1.,id:id(),file:file.into(),hash:hash_bytes(b"shared"),status:"ready".into(),duration:1.,peak:0.,warning:String::new(),created:0,endpoint:String::new()};
        old.takes.push(take.clone());store.project.archived_segments.push(old);
        let mut current=Segment::new("残す本文".into());let mut shared=take;shared.id=id();current.takes.push(shared);store.project.segments.push(current);store.save().unwrap();
        assert!(purge(&mut store,&target,false).is_err());purge(&mut store,&target,true).unwrap();assert!(safe(&root,file).unwrap().exists());
        for entry in fs::read_dir(root.join("revisions")).unwrap(){let path=entry.unwrap().path();if path.extension().and_then(|s|s.to_str())==Some("json"){let p:Project=serde_json::from_slice(&fs::read(path).unwrap()).unwrap();assert!(!p.segments.iter().chain(&p.archived_segments).any(|s|s.id==target));}}
    }
    #[test]
    fn purge_removes_exclusive_audio_and_can_be_repeated(){
        let root=std::env::temp_dir().join(id());let mut store=Store::open(root.clone(),true).unwrap();
        let file="takes/only/audio.wav";write_new(&safe(&root,file).unwrap(),b"only").unwrap();
        let mut old=Segment::new("消す本文".into());let target=old.id.clone();old.takes.push(Take{script_text:String::new(),trim_start:0.,trim_end:1.,id:id(),file:file.into(),hash:hash_bytes(b"only"),status:"ready".into(),duration:1.,peak:0.,warning:String::new(),created:0,endpoint:String::new()});
        let run=id();let result=format!("asr/{run}/result.json");let stderr=format!("asr/{run}/stderr.txt");write_new(&safe(&root,&result).unwrap(),b"{}").unwrap();write_new(&safe(&root,&stderr).unwrap(),b"error").unwrap();old.transcriptions.push(json!({"id":run}));store.project.archived_segments.push(old);store.save().unwrap();
        purge(&mut store,&target,true).unwrap();assert!(!safe(&root,file).unwrap().exists());assert!(!safe(&root,&result).unwrap().exists());assert!(!safe(&root,&stderr).unwrap().exists());purge(&mut store,&target,true).unwrap();
    }
    #[test]
    fn pending_journal_retries_without_requiring_the_archived_segment(){
        let root=std::env::temp_dir().join(id());let mut store=Store::open(root.clone(),true).unwrap();
        let file="takes/retry/audio.wav";write_new(&safe(&root,file).unwrap(),b"retry").unwrap();
        let mut old=Segment::new("途中失敗".into());let target=old.id.clone();old.takes.push(Take{script_text:String::new(),trim_start:0.,trim_end:1.,id:id(),file:file.into(),hash:hash_bytes(b"retry"),status:"ready".into(),duration:1.,peak:0.,warning:String::new(),created:0,endpoint:String::new()});store.project.archived_segments.push(old);store.save().unwrap();
        // 最初のjournal保存後、世代再構成・ファイル消去の前に落ちた状態を模擬する。
        scrub(&mut store.project,&target);store.project.deletion_records.push(json!({"segment_id":target,"status":"pending","files":[file]}));store.save().unwrap();
        purge(&mut store,&target,true).unwrap();assert!(!safe(&root,file).unwrap().exists());assert_eq!(store.project.deletion_records.last().unwrap()["status"],"done");
        for entry in fs::read_dir(root.join("revisions")).unwrap(){let path=entry.unwrap().path();if path.extension().and_then(|s|s.to_str())==Some("json"){let p:Project=serde_json::from_slice(&fs::read(path).unwrap()).unwrap();assert!(!p.segments.iter().chain(&p.archived_segments).any(|s|s.id==target));}}
    }
    fn prepare_source(store:&mut Store,name:&str,text:&str)->String{
        let revision=store.project.script_revision;
        crate::script_sources::insert_empty(&mut store.project,&json!({"name":name,"base_revision":revision})).unwrap();
        let source=store.project.script_sources.last().unwrap().clone();
        let mut values=store.project.script_sources.iter().map(|s|json!({"id":s.id,"name":s.name,"text":s.confirmed_text,"lines":s.lines,"approved_text":s.confirmed_text})).collect::<Vec<_>>();
        let last=values.last_mut().unwrap();
        last["text"]=text.into();last["approved_text"]=text.into();last["lines"]=serde_json::to_value(crate::markdown::parse(text).unwrap()).unwrap();
        let value=json!({"base_revision":store.project.script_revision,"sources":values});
        crate::script_sources::commit(&mut store.project,&value).unwrap();store.save().unwrap();source.id
    }
    fn delete_source(store:&mut Store,source_id:&str){
        let value=json!({"base_revision":store.project.script_revision,"deleted_source_ids":[source_id],"sources":store.project.script_sources.iter().map(|s|json!({"id":s.id,"name":s.name,"text":s.confirmed_text,"lines":s.lines,"approved_text":s.confirmed_text})).collect::<Vec<_>>()});
        crate::script_sources::commit(&mut store.project,&value).unwrap();store.save().unwrap();
    }
    fn assert_absent(root:&Path,token:&str){
        for entry in fs::read_dir(root.join("revisions")).unwrap(){
            let path=entry.unwrap().path();
            if path.extension().and_then(|s|s.to_str())==Some("json"){assert!(!fs::read_to_string(path).unwrap().contains(token));}
        }
        assert!(!fs::read_to_string(root.join("project.json")).unwrap().contains(token));
    }
    #[test]
    fn source_purge_removes_manuscript_from_every_revision_and_keeps_other_sources(){
        let root=std::env::temp_dir().join(id());let mut store=Store::open(root.clone(),true).unwrap();
        let target=prepare_source(&mut store,"消す原稿.md","固有秘密本文");
        prepare_source(&mut store,"残す原稿.md","維持本文");
        delete_source(&mut store,&target);
        assert!(purge_target(&mut store,&target,true,false).is_err());
        purge_target(&mut store,&target,true,true).unwrap();
        assert_absent(&root,"固有秘密本文");assert_absent(&root,"消す原稿.md");
        assert_eq!(store.project.script_sources[0].confirmed_text,"維持本文");
        drop(store);let reopened=Store::open(root,false).unwrap();
        assert_eq!(reopened.project.script_sources.len(),1);
    }
    #[test]
    fn row_purge_scrubs_raw_source_fields_but_keeps_the_other_row(){
        let root=std::env::temp_dir().join(id());let mut store=Store::open(root.clone(),true).unwrap();
        let source_id=prepare_source(&mut store,"原稿.md","消去専用の行
保持する行");
        let target=store.project.segments[0].id.clone();
        let value=json!({"base_revision":store.project.script_revision,"sources":[{"id":source_id,"name":"原稿.md","text":"保持する行","approved_text":"保持する行","lines":[{"line":0,"enabled":true,"previous_line":1}]}]});
        crate::script_sources::commit(&mut store.project,&value).unwrap();store.save().unwrap();
        purge(&mut store,&target,true).unwrap();assert_absent(&root,"消去専用の行");
        assert_eq!(store.project.script_sources[0].confirmed_text,"保持する行");
        assert_eq!(store.project.segments[0].text,"保持する行");
    }
    #[test]
    fn recording_free_source_can_be_purged_and_draft_mappings_block_row_purge(){
        let root=std::env::temp_dir().join(id());let mut store=Store::open(root,true).unwrap();
        crate::script_sources::insert_empty(&mut store.project,&json!({"name":"未確定原稿.md","base_revision":0})).unwrap();
        let source_id=store.project.script_sources[0].id.clone();
        store.project.script_sources[0].draft_text="未録音の秘密".into();store.save().unwrap();
        delete_source(&mut store,&source_id);
        purge_target(&mut store,&source_id,true,true).unwrap();
        assert_absent(&store.root,"未録音の秘密");
        let active_source=prepare_source(&mut store,"使用中.md","現役");
        let mut old=Segment::new("以前".into());old.source_id=active_source;let target=old.id.clone();
        store.project.archived_segments.push(old);
        store.project.script_sources[0].draft_lines=vec![json!({"line":0,"enabled":true,"previous_id":target})];
        store.save().unwrap();
        assert!(purge(&mut store,&target,true).unwrap_err().contains("下書き"));
        assert_eq!(store.project.archived_segments.len(),1);
    }

}
