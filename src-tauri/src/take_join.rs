//! 前後の保存済みテイクを連結し、元テイクを保持したまま新規追加する。
use crate::{audio, domain::*, storage::{safe, hash_file, Store}};
use serde_json::{json, Value};
use std::{fs::{self, OpenOptions}, io::BufWriter, path::Path};

fn inputs(project: &Project, v: &Value) -> Result<(Take, Take)> {
    if v["project_id"].as_str() != Some(project.id.as_str()) {
        return Err("プロジェクトが変わっています。結合画面を開き直してください".into());
    }
    let sid = v["segment"].as_str().ok_or("発話が必要です")?;
    let row = project.segments.iter().find(|s| s.id == sid).ok_or("発話がありません")?;
    let mut takes = Vec::new();
    for key in ["before", "after"] {
        let tid = v[key].as_str().ok_or("前と後のテイクを選択してください")?;
        let take = row.takes.iter().find(|t| t.id == tid && t.status == "ready").ok_or("結合できる保存済みテイクがありません")?;
        if v[format!("{key}_hash")].as_str() != Some(take.hash.as_str()) {
            return Err("テイクが更新されています。結合画面を開き直してください".into());
        }
        takes.push(take.clone());
    }
    if takes[0].id == takes[1].id { return Err("前と後には異なるテイクを選択してください".into()); }
    Ok((takes.remove(0), takes.remove(0)))
}
fn joined_samples(root: &Path, before: &Take, after: &Take) -> Result<Vec<f32>> {
    let mut result = Vec::new();
    for take in [before, after] {
        let path = safe(root, &take.file)?;
        if hash_file(&path)? != take.hash { return Err("音声が変わっています。結合を中止しました".into()); }
        let samples = audio::samples(&path)?;
        let (start, end) = take.trimmed_range()?;
        let from = (start * 48000.0).round() as usize * 2;
        let to = (end * 48000.0).round() as usize * 2;
        if from >= to || to > samples.len() { return Err("テイクの保存範囲と音声の長さが一致しません".into()); }
        if (result.len() + to - from) * 4 + 44 > 128 * 1024 * 1024 { return Err("結合後の音声は128 MB以内にしてください".into()); }
        result.extend_from_slice(&samples[from..to]);
    }
    Ok(result)
}
fn write_wav(path: &Path, samples: &[f32]) -> Result<()> {
    let file = OpenOptions::new().write(true).create_new(true).open(path).map_err(|e|e.to_string())?;
    let result = (|| {
        let spec = hound::WavSpec {channels:2,sample_rate:48000,bits_per_sample:32,sample_format:hound::SampleFormat::Float};
        let mut writer = hound::WavWriter::new(BufWriter::new(file), spec).map_err(|e|e.to_string())?;
        for sample in samples { writer.write_sample(*sample).map_err(|e|e.to_string())?; }
        writer.finalize().map_err(|e|e.to_string())
    })();
    if result.is_err() { let _ = fs::remove_file(path); }
    result
}
pub fn preview(store: &Store, v: &Value) -> Result<Value> {
    let (before, after) = inputs(&store.project, v)?;
    let samples = joined_samples(&store.root, &before, &after)?;
    let file = format!("previews/take-join-{}.wav", id());
    let path = safe(&store.root, &file)?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|e|e.to_string())?;
    write_wav(&path, &samples)?;
    Ok(json!({"file":file,"hash":hash_file(&path)?,"duration":samples.len() as f64 / 96000.0}))
}
pub fn save(store: &mut Store, v: &Value) -> Result<Value> {
    let (before, after) = inputs(&store.project, v)?;
    let samples = joined_samples(&store.root, &before, &after)?;
    let tid = id();
    let file = format!("takes/{tid}/audio.wav");
    let path = safe(&store.root, &file)?;
    let folder = path.parent().unwrap();
    fs::create_dir_all(folder).map_err(|e|e.to_string())?;
    let previous = store.project.clone();
    let result = (|| {
        write_wav(&path, &samples)?;
        let hash = hash_file(&path)?;
        let duration = samples.len() as f64 / 96000.0;
        let peak = samples.iter().fold(0.0_f32, |a, b|a.max(b.abs()));
        let sid = v["segment"].as_str().unwrap();
        let row = store.project.segments.iter_mut().find(|s|s.id == sid).ok_or("発話がありません")?;
        let mut warning = [before.warning.as_str(), after.warning.as_str()].into_iter().filter(|s|!s.is_empty()).collect::<Vec<_>>().join(" / ");
        if (before.warning.starts_with("要確認：") || after.warning.starts_with("要確認：")) && !warning.starts_with("要確認：") {
            warning = format!("要確認：{warning}");
        }
        let script_text = if before.script_text == after.script_text { before.script_text.clone() } else { format!("{}\n{}",before.script_text,after.script_text) };
        row.takes.push(Take {id:tid.clone(),file,hash,status:"ready".into(),duration,peak,warning,created:now(),endpoint:"joined".into(),script_text,trim_start:0.0,trim_end:0.0});
        row.take_notes.insert(tid.clone(), TakeNote {memo:format!("テイク結合：前 {} → 後 {}",before.id,after.id),score:None,favorite:false});
        // 新規テイクは未採用。既存の採用、読み確認、ASR結果に触れない。
        store.save()?;
        Ok(json!({"take_id":tid,"duration":duration}))
    })();
    if result.is_err() {
        store.project = previous;
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(folder);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (Store, Value) {
        let root = std::env::temp_dir().join(format!("scriptsync-join-{}",id()));
        let mut store = Store::open(root, true).unwrap();
        let mut row = Segment::new("結合試験".into());
        for value in [0.25_f32,-0.5_f32] {
            let tid=id(); let file=format!("takes/{tid}/audio.wav");let path=safe(&store.root,&file).unwrap();fs::create_dir_all(path.parent().unwrap()).unwrap();write_wav(&path,&vec![value;960]).unwrap();
            row.takes.push(Take{id:tid,file,hash:hash_file(&path).unwrap(),status:"ready".into(),duration:0.01,peak:value.abs(),warning:String::new(),created:now(),endpoint:"test".into(),script_text:row.text.clone(),trim_start:0.0,trim_end:0.0});
        }
        row.selected=Some(row.takes[0].id.clone());
        let v=json!({"project_id":store.project.id,"segment":row.id,"before":row.takes[0].id,"before_hash":row.takes[0].hash,"after":row.takes[1].id,"after_hash":row.takes[1].hash});
        store.project.segments.push(row);store.save().unwrap();(store,v)
    }
    #[test]
    fn join_preserves_sample_order_originals_selection_and_reopens() {
        let (mut store,v)=setup();store.project.segments[0].takes[0].warning="クリッピングの可能性".into();store.project.segments[0].takes[1].warning="要確認：欠落あり".into();let originals=store.project.segments[0].takes.clone();let selected=store.project.segments[0].selected.clone();
        let preview=preview(&store,&v).unwrap();assert_eq!(store.project.segments[0].takes.len(),2);
        let preview_data=audio::samples(&safe(&store.root,preview["file"].as_str().unwrap()).unwrap()).unwrap();
        let result=save(&mut store,&v).unwrap();assert_eq!(result["duration"],0.02);assert_eq!(store.project.segments[0].selected,selected);
        let joined=store.project.segments[0].takes.last().unwrap();assert!(joined.warning.starts_with("要確認："));let samples=audio::samples(&safe(&store.root,&joined.file).unwrap()).unwrap();assert_eq!(samples,preview_data);assert_eq!(&samples[..960],vec![0.25;960]);assert_eq!(&samples[960..],vec![-0.5;960]);
        for t in originals{assert_eq!(hash_file(&safe(&store.root,&t.file).unwrap()).unwrap(),t.hash);}
        let root=store.root.clone();drop(store);let reopened=Store::open(root.clone(),false).unwrap();assert_eq!(reopened.project.segments[0].takes.len(),3);drop(reopened);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_same_stale_and_missing_sources_without_adding_take() {
        let (mut store,v)=setup();let mut bad=v.clone();bad["after"]=bad["before"].clone();bad["after_hash"]=bad["before_hash"].clone();assert!(save(&mut store,&bad).unwrap_err().contains("異なる"));
        bad=v.clone();bad["before_hash"]=json!("changed");assert!(save(&mut store,&bad).is_err());
        let input=safe(&store.root,&store.project.segments[0].takes[0].file).unwrap();fs::write(input,b"changed").unwrap();assert!(save(&mut store,&v).is_err());assert_eq!(store.project.segments[0].takes.len(),2);
        let root=store.root.clone();drop(store);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn saved_ranges_and_save_failure_are_safe() {
        let (mut store,v)=setup();store.project.segments[0].takes[0].trim_start=0.005;store.project.segments[0].takes[0].trim_end=0.01;
        assert_eq!(preview(&store,&v).unwrap()["duration"],0.015);
        // 次の世代を既存として置き、保存競合を起こす。
        let conflict=store.root.join(format!("revisions/{:020}.json",store.project.generation+1));fs::write(&conflict,b"reserved").unwrap();
        assert!(save(&mut store,&v).is_err());assert_eq!(store.project.segments[0].takes.len(),2);assert_eq!(fs::read_dir(store.root.join("takes")).unwrap().count(),2);
        let root=store.root.clone();drop(store);fs::remove_dir_all(root).unwrap();
    }
}
