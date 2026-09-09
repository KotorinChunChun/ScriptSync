//! アーカイブの確認付き完全消去。保存世代から同じ発話が復活しないよう消去記録を残す。
use crate::{domain::*,storage::*};
use serde_json::{json,Value};
use std::{collections::HashSet,fs,path::Path};
fn files(s:&Segment)->Vec<String>{
    let mut result:Vec<String>=s.takes.iter().map(|t|t.file.clone())
        .chain(s.comparisons.iter().flat_map(|r|r.variants.iter().map(|v|v.file.clone())))
        .filter(|f|!f.is_empty()).collect();
    // ASRのJSON・stderrは実行IDごとの専用フォルダに置く。IDをパスとして受け取らない。
    for run in &s.transcriptions {
        if let Some(run)=run["id"].as_str().filter(|run|uuid::Uuid::parse_str(run).is_ok()) {
            result.push(format!("asr/{run}/result.json"));
            result.push(format!("asr/{run}/stderr.txt"));
        }
    }
    result
}
fn scrub(p:&mut Project,target:&str){
    p.segments.retain(|s|s.id!=target);p.archived_segments.retain(|s|s.id!=target);
    for source in &mut p.script_sources {
        source.blank_rows.retain(|r|r.segment_id!=target);
        for row in &mut source.draft_lines{if row["previous_id"]==target{row["previous_id"]=Value::Null;row["keep"]=false.into();}}
    }
    p.match_history.retain(|r|!r.to_string().contains(target));
    p.deletion_records.retain(|r|r["segment_id"]!=target);
}
fn atomic_json(path:&Path,value:&Project)->Result<()>{
    let tmp=path.with_extension(format!("{}.purge-tmp",id()));
    json_new(&tmp,value)?;
    if let Err(e)=fs::rename(&tmp,path){let _=fs::remove_file(&tmp);return Err(e.to_string());}
    Ok(())
}
/// 一度消去を確定した後の失敗はjournalを保持し、同じ操作で再実行する。
pub fn purge(store:&mut Store,target:&str,confirmed:bool)->Result<()> {
    if !confirmed{return Err("完全消去の確認が必要です".into());}
    uuid::Uuid::parse_str(target).map_err(|_|"発話IDが不正です")?;
    if store.project.segments.iter().any(|s|s.id==target){return Err("現役の発話は完全消去できません".into());}
    let pending=store.project.deletion_records.iter().any(|r|r["segment_id"]==target);
    if !pending && !store.project.archived_segments.iter().any(|s|s.id==target){return Err("廃棄済み発話がありません".into());}
    let dir=safe(&store.root,"revisions")?;
    let mut snapshots=Vec::new();let mut owned=HashSet::new();let mut shared=HashSet::new();
    for entry in fs::read_dir(&dir).map_err(|e|e.to_string())?{
        let path=entry.map_err(|e|e.to_string())?.path();
        if path.extension().and_then(|x|x.to_str())!=Some("json"){continue;}
        let p:Project=serde_json::from_slice(&fs::read(&path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        for s in p.segments.iter().chain(&p.archived_segments){if s.id==target{owned.extend(files(s));}else{shared.extend(files(s));}}
        for record in &p.deletion_records { if record["segment_id"]!=target { if let Some(paths)=record["files"].as_array(){shared.extend(paths.iter().filter_map(|path|path.as_str().map(str::to_owned)));} } }
        snapshots.push((path,p));
    }
    for s in store.project.segments.iter().chain(&store.project.archived_segments){if s.id==target{owned.extend(files(s));}else{shared.extend(files(s));}}
    for record in &store.project.deletion_records { if record["segment_id"]!=target { if let Some(paths)=record["files"].as_array(){shared.extend(paths.iter().filter_map(|path|path.as_str().map(str::to_owned)));} } }
    for r in &store.project.deletion_records{if r["segment_id"]==target{if let Some(paths)=r["files"].as_array(){owned.extend(paths.iter().filter_map(|p|p.as_str().map(str::to_owned)));}}}
    let exclusive:Vec<String>=owned.difference(&shared).cloned().collect();
    // 先に全パスを検査する。任意のディレクトリや外部パスの削除はしない。
    for file in &exclusive {safe(&store.root,file)?;}
    let before=store.project.clone();
    scrub(&mut store.project,target);
    store.project.deletion_records.retain(|r|r["segment_id"]!=target);
    store.project.deletion_records.push(json!({"segment_id":target,"status":"pending","files":exclusive,"created":now()}));
    if let Err(e)=store.save(){store.project=before;return Err(e);}
    // 公開された最新世代で削除を宣言してから、過去世代を消去用例外として再構成する。
    for (path,mut p) in snapshots {scrub(&mut p,target);atomic_json(&path,&p)?;}
    for file in &exclusive {let path=safe(&store.root,file)?;if path.exists(){fs::remove_file(&path).map_err(|e|format!("音声消去に失敗しました。再実行できます: {e}"))?;}}
    if let Some(record)=store.project.deletion_records.iter_mut().find(|r|r["segment_id"]==target){record["status"]="done".into();record["files"]=json!([]);}
    store.save()?;
    // 閲覧用索引も録音を含まない現在の状態へ置き換える。
    atomic_json(&safe(&store.root,"project.json")?,&store.project)?;
    Ok(())
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
}
