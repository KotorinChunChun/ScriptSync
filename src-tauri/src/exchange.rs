//! Web交換ZIP。検査中は展開せず、承認された対応先だけへ録音を追加する。
use crate::{domain::*, storage::{Store, hash_bytes, safe, write_new}};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::{HashMap, HashSet}, fs::File, io::{Cursor, Read, Write}, path::Path};

const MIB: u64 = 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioInfo {
    pub path: String, pub mime: String, pub sha256: String, pub duration: f64,
    #[serde(rename="sampleRate")] pub sample_rate: u32,
    pub channels: u16,
    #[serde(rename="bitsPerSample")] pub bits_per_sample: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExchangeTake {
    pub id: String,
    #[serde(flatten)] pub audio: AudioInfo,
    #[serde(default)] pub created: Option<String>,
    #[serde(default)] pub original: Option<AudioInfo>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExchangeLine { pub order: usize, pub text: String, pub selected: Option<String>, pub takes: Vec<ExchangeTake> }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String, pub title: String, pub source: String,
    #[serde(default, rename="sourceFileName")] pub source_file_name: Option<String>,
    #[serde(default, rename="scriptPath")] pub script_path: Option<String>,
    #[serde(rename="exportedAt")] pub exported_at: String,
    pub scope: String, pub lines: Vec<ExchangeLine>,
}
pub struct ValidatedExchange { pub token: String, pub manifest: Manifest, audio: HashMap<String, Vec<u8>> }
impl ValidatedExchange {
    pub fn summary(&self) -> Value { json!({"token":self.token,"manifest":self.manifest}) }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LineMapping { pub order: usize, pub segment_id: String }
#[derive(Debug, Serialize)]
pub struct MergeReport { pub added: usize, pub duplicates: usize }

fn valid_path(name: &str) -> bool {
    !name.is_empty() && name.len() <= 512 && !name.contains(['\\', ':', '\0'])
        && name.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}
fn validate_manifest_limits(manifest: &Manifest) -> Result<()> {
    // WebのString.lengthと同じUTF-16コード単位で判定する（絵文字は通常2）。
    if manifest.title.encode_utf16().count() > 100 { return Err("Webの作品名は100文字以内にしてください（絵文字等は2文字分）".into()); }
    if !(1..=1000).contains(&manifest.lines.len()) { return Err("Webへ渡す読み上げ行は1～1000行にしてください".into()); }
    if manifest.source_file_name.as_ref().is_some_and(|name| name.encode_utf16().count() > 255) { return Err("交換原稿ファイル名が255文字を超えています".into()); }
    for line in &manifest.lines {
        if line.text.encode_utf16().count() > 20000 { return Err(format!("{}行目がWebの20000文字上限を超えています（絵文字等は2文字分）",line.order)); }
        if line.takes.len() > 200 { return Err(format!("{}行目のテイク数が200件を超えています",line.order)); }
    }
    Ok(())
}
fn verify_audio(info: &AudioInfo, bytes: &[u8]) -> Result<()> {
    if info.mime != "audio/wav" || info.sample_rate != 48000 || info.bits_per_sample != 16
        || !(1..=2).contains(&info.channels) || !info.duration.is_finite() || info.duration <= 0.0
        || info.sha256.len() != 64 || info.sha256 != hash_bytes(bytes) {
        return Err("交換音声の形式またはSHA-256が一致しません".into());
    }
    let mut wav = hound::WavReader::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let spec = wav.spec();
    if spec.sample_format != hound::SampleFormat::Int || spec.sample_rate != info.sample_rate
        || spec.channels != info.channels || spec.bits_per_sample != info.bits_per_sample {
        return Err("WAVの実形式と交換情報が一致しません".into());
    }
    let duration = wav.duration() as f64 / spec.sample_rate as f64;
    if (duration - info.duration).abs() > 1.0 / 48000.0 { return Err("WAVの長さと交換情報が一致しません".into()); }
    let mut count = 0;
    for sample in wav.samples::<i16>() { sample.map_err(|e| e.to_string())?; count += 1; }
    if count == 0 || count % spec.channels as usize != 0 { return Err("WAVのサンプル数が不正です".into()); }
    Ok(())
}

pub fn inspect_zip(path: &Path, password: Option<&str>) -> Result<ValidatedExchange> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 100 * MIB { return Err("ZIPは100MiB以内にしてください".into()); }
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    if zip.len() > 5000 { return Err("ZIPの項目が5000件を超えています".into()); }
    let mut files = HashMap::new();
    let mut names = HashSet::new();
    let mut total = 0u64;
    for index in 0..zip.len() {
        let mut entry = match password {
            Some(value) => zip.by_index_decrypt(index, value.as_bytes()),
            None => zip.by_index(index),
        }.map_err(|e| format!("ZIPを開けません。暗号化ZIPは正しいキーを指定してください: {e}"))?;
        let name = entry.name().to_owned();
        if !valid_path(&name) || !names.insert(name.to_lowercase())
            || entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err("ZIPのパス、重複項目、リンクが不正です".into());
        }
        let limit = if name == "project.json" { 4 * MIB } else { 100 * MIB };
        total = total.checked_add(entry.size()).ok_or("展開サイズが不正です")?;
        if entry.size() > limit || total > 120 * MIB { return Err("ZIPの展開サイズが上限を超えています".into()); }
        let mut bytes = Vec::new();
        entry.by_ref().take(limit + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() as u64 != entry.size() || bytes.len() as u64 > limit { return Err("ZIPの実サイズが不正です".into()); }
        files.insert(name, bytes);
    }
    let manifest: Manifest = serde_json::from_slice(files.get("project.json").ok_or("project.jsonがありません")?).map_err(|e| e.to_string())?;
    validate_manifest_limits(&manifest)?;
    if manifest.format != "scriptsyncweb-exchange-1" || !["all", "selected"].contains(&manifest.scope.as_str()) || manifest.lines.len() > 1000 {
        return Err("未対応の交換形式、範囲、行数です".into());
    }
    if let Some(script_path) = &manifest.script_path {
        if !valid_path(script_path) || files.get(script_path).is_none_or(|bytes| bytes != manifest.source.as_bytes()) {
            return Err("交換原稿とsourceが一致しません".into());
        }
    }
    let mut audio_total = 0u64;
    let mut referenced = HashSet::new();
    for (index, line) in manifest.lines.iter().enumerate() {
        if line.order != index + 1 || line.text.trim().is_empty() { return Err("交換行の順序または本文が不正です".into()); }
        let mut ids = HashSet::new();
        for take in &line.takes {
            if take.id.is_empty() || !ids.insert(&take.id) { return Err("テイクIDが重複しています".into()); }
            for info in std::iter::once(&take.audio).chain(take.original.iter()) {
                if !valid_path(&info.path) || !info.path.starts_with("audio/") || !referenced.insert(info.path.clone()) { return Err("音声パスが不正または重複しています".into()); }
                let bytes = files.get(&info.path).ok_or("交換音声が不足しています")?;
                audio_total += bytes.len() as u64;
                if audio_total > 100 * MIB { return Err("交換音声合計が100MiBを超えています".into()); }
                verify_audio(info, bytes)?;
            }
        }
        if line.selected.as_ref().is_some_and(|selected| !ids.contains(selected)) { return Err("採用テイクが存在しません".into()); }
    }
    files.retain(|name, _| referenced.contains(name));
    Ok(ValidatedExchange { token: id(), manifest, audio: files })
}

pub fn export_script(project: &Project, source_id: &str, path: &Path, exported_at: &str) -> Result<()> {
    let source = project.script_sources.iter().find(|source| source.id == source_id).ok_or("出力する台本がありません")?;
    let lines: Vec<_> = project.segments.iter().filter(|segment| segment.source_id == source_id && !segment.skip && !segment.text.trim().is_empty())
        .enumerate().map(|(index, segment)| ExchangeLine { order:index+1, text:segment.text.clone(), selected:None, takes:vec![] }).collect();
    let stem: String = source.name.trim_end_matches(".md").trim_end_matches(".txt").chars()
        .map(|ch| if ch.is_control() || "\\/:*?\"<>|".contains(ch) { '_' } else { ch }).take(100).collect();
    let filename = format!("{}.md", if stem.trim_matches([' ', '.']).is_empty() { "原稿" } else { stem.trim_matches([' ', '.']) });
    let manifest = Manifest { format:"scriptsyncweb-exchange-1".into(), title:project.title.clone(), source:source.confirmed_text.clone(),
        source_file_name:Some(filename.clone()), script_path:Some(format!("scripts/{filename}")), exported_at:exported_at.into(), scope:"all".into(), lines };
    validate_manifest_limits(&manifest)?;
    let json = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
    if json.len() as u64 > 4 * MIB { return Err("交換原稿が4MiBを超えています".into()); }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in [("project.json".to_string(), json), (format!("scripts/{filename}"), manifest.source.into_bytes()),
        ("script.txt".into(), manifest.lines.iter().map(|line| format!("{:03}\t{}",line.order,line.text)).collect::<Vec<_>>().join("\n").into_bytes())] {
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    let bytes = zip.finish().map_err(|e| e.to_string())?.into_inner();
    // 作成失敗時に既存ZIPを壊さない。保存ダイアログで新しい名前を選ぶ。
    write_new(path, &bytes)
}

pub fn merge_audio(store: &mut Store, exchange: &ValidatedExchange, mappings: &[LineMapping]) -> Result<MergeReport> {
    let mut destinations = HashMap::new();
    let mut used = HashSet::new();
    for mapping in mappings {
        if destinations.insert(mapping.order, mapping.segment_id.clone()).is_some() || !used.insert(mapping.segment_id.clone()) {
            return Err("行対応が重複しています".into());
        }
        if !exchange.manifest.lines.iter().any(|line| line.order == mapping.order)
            || !store.project.segments.iter().any(|segment| segment.id == mapping.segment_id) { return Err("行対応の対象が存在しません".into()); }
    }
    for line in exchange.manifest.lines.iter().filter(|line| !line.takes.is_empty()) {
        if !destinations.contains_key(&line.order) { return Err(format!("{}行目の音声の対応先を指定してください", line.order)); }
    }
    let before = store.project.clone();
    let mut created = Vec::new();
    let result = (|| {
        let mut report = MergeReport { added:0, duplicates:0 };
        for line in &exchange.manifest.lines {
            let Some(destination) = destinations.get(&line.order) else { continue; };
            let segment = store.project.segments.iter_mut().find(|segment| &segment.id == destination).ok_or("対応発話がありません")?;
            for incoming in &line.takes {
                if segment.takes.iter().any(|take| take.hash == incoming.audio.sha256) { report.duplicates += 1; continue; }
                let take_id = id();
                let relative = format!("takes/{take_id}/audio.wav");
                let path = safe(&store.root, &relative)?;
                write_new(&path, &exchange.audio[&incoming.audio.path])?;
                created.push(path);
                if let Some(original) = &incoming.original {
                    let path = safe(&store.root, &format!("takes/{take_id}/original.wav"))?;
                    write_new(&path, &exchange.audio[&original.path])?; created.push(path);
                }
                let metadata = safe(&store.root, &format!("takes/{take_id}/web-exchange.json"))?;
                write_new(&metadata, &serde_json::to_vec(&json!({"take":incoming,"web_selected":line.selected.as_ref()==Some(&incoming.id),"line_text":line.text})).map_err(|e| e.to_string())?)?;
                created.push(metadata);
                let (_, peak) = crate::audio::inspect(&safe(&store.root, &relative)?)?;
                segment.takes.push(Take { id:take_id, file:relative, hash:incoming.audio.sha256.clone(), status:"ready".into(), duration:incoming.audio.duration, peak,
                    warning:String::new(), created:now(), endpoint:"ScriptSyncWeb".into(), script_text:line.text.clone(), trim_start:0.0, trim_end:0.0 });
                report.added += 1;
            }
        }
        if report.added > 0 { store.save()?; }
        Ok(report)
    })();
    if result.is_err() {
        store.project = before;
        for path in created.into_iter().rev() { let _ = std::fs::remove_file(path); }
    }
    result
}

#[cfg(test)]
#[path="exchange_tests.rs"]
mod tests;
