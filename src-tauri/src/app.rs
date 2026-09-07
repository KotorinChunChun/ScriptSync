use crate::{audio, domain::*, storage::*};
use audio_mixer_backend::windows::{
    devices::{DeviceDirection, list_devices},
    microphone_recording::{MicrophoneRecordingRequest, record_microphone_to_wav_controlled},
};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub struct Inner {
    pub queue_stop: Option<Arc<AtomicBool>>,
    pub media: std::collections::HashMap<String, std::path::PathBuf>,
    pub store: Option<Store>,
    pub busy: bool,
    pub job: String,
    pub message: String,
    pub peak: f32,
    pub elapsed: u64,
    pub stop: Option<Arc<AtomicBool>>,
}
#[derive(Clone, Default)]
pub struct App(pub Arc<Mutex<Inner>>);
pub fn string(v: &Value, key: &str) -> Result<String> {
    v[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{key} が必要です"))
}
pub fn segment<'a>(p: &'a mut Project, id: &str) -> Result<&'a mut Segment> {
    p.segments
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| "発話が見つかりません".into())
}
pub fn take_from_file(
    root: &std::path::Path,
    source: &std::path::Path,
    endpoint: String,
) -> Result<Take> {
    let (duration, peak) = audio::inspect(source)?;
    let tid = id();
    let file = format!("takes/{tid}/audio.wav");
    let path = safe(root, &file)?;
    write_new(&path, &std::fs::read(source).map_err(|e| e.to_string())?)?;
    Ok(Take {
        id: tid,
        file,
        hash: hash_file(&path)?,
        status: "ready".into(),
        duration,
        peak,
        warning: if peak >= 0.999 {
            "クリッピングの可能性があります".into()
        } else if peak < 0.01 {
            "音量が小さいため試聴してください".into()
        } else {
            String::new()
        },
        created: now(),
        endpoint,
    })
}
impl App {
    pub fn snapshot(&self) -> Result<Value> {
        let a = self.0.lock().map_err(|e| e.to_string())?;
        Ok(
            json!({"project":a.store.as_ref().map(|s|&s.project),"root":a.store.as_ref().map(|s|s.root.to_string_lossy()),"busy":a.busy||a.queue_stop.is_some(),"job":a.job,"message":a.message,"peak":a.peak,"elapsed":a.elapsed}),
        )
    }
    pub fn dispatch(&self, action: &str, v: Value) -> Result<Value> {
        if action == "state" {
            return self.snapshot();
        }
        if action == "diff" {
            let expected = string(&v, "expected")?;
            let observed = string(&v, "observed")?;
            let mut result = json!(crate::alignment::differences(&expected, &observed)?);
            let normalized: Vec<char> = normalize(&expected).chars().collect();
            let a = self.0.lock().map_err(|e| e.to_string())?;
            if let Some(s) = a.store.as_ref().and_then(|store| {
                store
                    .project
                    .segments
                    .iter()
                    .find(|s| Some(s.id.as_str()) == v["segment"].as_str())
            }) {
                for row in result.as_array_mut().unwrap() {
                    let start = row["expected_start"].as_u64().unwrap() as usize;
                    let end = row["expected_end"].as_u64().unwrap() as usize;
                    let mut candidates = Vec::new();
                    for m in &s.mappings {
                        let kana: Vec<char> = normalize(&m.kana).chars().collect();
                        if kana.is_empty() {
                            continue;
                        }
                        let hits: Vec<_> = normalized
                            .windows(kana.len())
                            .enumerate()
                            .filter(|(_, w)| *w == kana.as_slice())
                            .map(|(i, _)| i)
                            .collect();
                        if hits.iter().any(|&i| {
                            i <= start && start < i + kana.len() || i < end && end <= i + kana.len()
                        }) {
                            candidates.push(json!({"start":m.start,"end":m.end,"original":slice(&s.text,m.start,m.end)?,"kana":m.kana,"ambiguous":hits.len()!=1}));
                        }
                    }
                    row["source_candidates"] = json!(candidates);
                }
            }
            return Ok(result);
        }
        if action == "stop" {
            let a = self.0.lock().map_err(|e| e.to_string())?;
            if let Some(s) = &a.queue_stop {
                s.store(true, Ordering::Release);
            }
            if let Some(s) = &a.stop {
                s.store(true, Ordering::Release);
            }
            return Ok(json!(null));
        }
        if action == "devices" {
            return list_devices(DeviceDirection::Capture)
                .map(|d| json!(d))
                .map_err(|e| e.to_string());
        }
        if self
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .queue_stop
            .is_some()
        {
            return Err("一括処理中です。停止または完了を待ってください".into());
        }
        if action == "batch" {
            return self.batch(v);
        }
        if action == "record" {
            return self.record(v);
        }
        if action == "compare" {
            return self.compare(v);
        }
        if action == "asr" {
            return self.transcribe(v);
        }
        let mut a = self.0.lock().map_err(|e| e.to_string())?;
        if a.busy {
            return Err("処理中です。完了または停止を待ってください".into());
        }
        if action == "new" || action == "open" {
            let chosen = rfd::FileDialog::new()
                .set_title(if action == "new" {
                    "新しい作業フォルダを選択"
                } else {
                    "ScriptSyncの作業フォルダを開く"
                })
                .pick_folder();
            if let Some(folder) = chosen {
                let root = if action == "new" {
                    folder.join(format!("ScriptSync-{}", id()))
                } else {
                    folder
                };
                let store = Store::open(root, action == "new")?;
                a.store = Some(store);
                a.message = "プロジェクトを開きました".into();
            }
            return Ok(json!(null));
        }
        let store = a.store.as_mut().ok_or("プロジェクトを開いてください")?;
        if action == "audio" {
            let file = string(&v, "file")?;
            let hash = string(&v, "hash")?;
            let path = crate::media::prepared(&store.root, &file, &hash)?;
            let token = id();
            a.media.clear();
            a.media.insert(token.clone(), path);
            return Ok(json!(format!("http://scriptsync-audio.localhost/{token}")));
        }
        if action == "export" {
            let result = audio::assemble(&store.root, &store.project)?;
            a.message = "完成WAVをexportsへ保存しました".into();
            return Ok(result);
        }
        if action == "report" {
            return crate::speech::report(store);
        }
        if action == "talk_export" {
            return crate::talk_project::export(store, &v);
        }
        if action == "talk_inspect" {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Talkプロジェクト", &["tstprj"])
                .pick_file()
            else {
                return Ok(Value::Null);
            };
            return crate::talk_project::inspect_file(&path);
        }
        if action == "rules_export" {
            return crate::rules::export(store);
        }
        if action == "waveform" {
            let file = string(&v, "file")?;
            let path = safe(&store.root, &file)?;
            if hash_file(&path)? != string(&v, "hash")? {
                return Err("音声が変わっています".into());
            }
            return crate::editing::waveform(&path);
        }
        if action == "preview_edit" {
            return crate::editing::preview(store, &v);
        }
        if action == "revision_preview" {
            return crate::editing::revision_preview(&store.project, &string(&v, "text")?);
        }
        let previous = store.project.clone();
        let result = (|| -> Result<()> {
            match action {
                "rule_create" | "rule_validate" | "rule_decide" => {
                    crate::rules::mutate(store, action, &v)?
                }
                "split" | "merge" | "revise_script" | "restore_segment" => {
                    crate::editing::edit_script(&mut store.project, action, &v)?
                }
                "gain" => {
                    let s = segment(&mut store.project, &string(&v, "segment")?)?;
                    let gain = v["gain_db"].as_f64().ok_or("音量が必要です")?;
                    if !gain.is_finite() || !(-36.0..=24.0).contains(&gain) {
                        return Err("音量は-36～24 dBです".into());
                    }
                    s.gain_db = gain;
                }
                "take_note" => {
                    let s = segment(&mut store.project, &string(&v, "segment")?)?;
                    let tid = string(&v, "take")?;
                    if !s.takes.iter().any(|t| t.id == tid) {
                        return Err("テイクがありません".into());
                    }
                    s.take_notes.insert(
                        tid,
                        TakeNote {
                            memo: string(&v, "memo")?,
                            favorite: v["favorite"].as_bool().unwrap_or(false),
                        },
                    );
                }
                "append" | "import_script" => {
                    let text = if action == "import_script" {
                        let Some(path) = rfd::FileDialog::new()
                            .add_filter("台本", &["txt", "md"])
                            .pick_file()
                        else {
                            return Ok(());
                        };
                        std::fs::read_to_string(path).map_err(|e| e.to_string())?
                    } else {
                        string(&v, "text")?
                    };
                    let lines: Vec<_> = text
                        .lines()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .collect();
                    if lines.is_empty() {
                        return Err("台本が空です".into());
                    }
                    if store.project.segments.len() + lines.len() > 1000 {
                        return Err("台本は1000発話以内に分割してください".into());
                    }
                    for line in lines {
                        store.project.segments.push(Segment::new(line.into()));
                    }
                }
                "import_audio" => {
                    let sid = string(&v, "segment")?;
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter("音声 WAV", &["wav"])
                        .pick_file()
                    else {
                        return Ok(());
                    };
                    let take = take_from_file(&store.root, &path, "import".into())?;
                    segment(&mut store.project, &sid)?.takes.push(take);
                }
                "edit" => {
                    let sid = string(&v, "segment")?;
                    let s = segment(&mut store.project, &sid)?;
                    let text = string(&v, "text")?;
                    if text.trim().is_empty() {
                        return Err("原稿が空です".into());
                    }
                    if s.text != text {
                        s.needs_review = true;
                        s.text = text;
                        s.revision += 1;
                        s.confirmed = false;
                        s.mappings.clear();
                        s.corrections.clear();
                        s.kana.clear();
                        s.selected = None;
                    }
                    s.skip = v["skip"].as_bool().unwrap_or(false);
                    for key in ["trim_start", "trim_end", "gap"] {
                        let n = v[key].as_f64().unwrap_or(0.0);
                        if !n.is_finite() || n < 0.0 || n > 3600.0 {
                            return Err("時間が不正です".into());
                        }
                        match key {
                            "trim_start" => s.trim_start = n,
                            "trim_end" => s.trim_end = n,
                            _ => s.gap = n,
                        };
                    }
                }
                "move" => {
                    let sid = string(&v, "segment")?;
                    let index = store
                        .project
                        .segments
                        .iter()
                        .position(|s| s.id == sid)
                        .ok_or("発話がありません")?;
                    let target = index as i64 + v["delta"].as_i64().unwrap_or(0);
                    if target >= 0 && (target as usize) < store.project.segments.len() {
                        store.project.segments.swap(index, target as usize);
                    }
                }
                "select" => {
                    let s = segment(&mut store.project, &string(&v, "segment")?)?;
                    let tid = string(&v, "take")?;
                    let t = s
                        .takes
                        .iter()
                        .find(|t| t.id == tid && t.status == "ready")
                        .ok_or("採用できないテイクです")?;
                    if hash_file(&safe(&store.root, &t.file)?)? != t.hash {
                        return Err("音声が変わっています".into());
                    }
                    s.selected = Some(tid);
                    s.needs_review = false;
                    s.confirmed = false;
                    s.trim_start = 0.0;
                    s.trim_end = 0.0;
                }
                "reference" => {
                    let s = segment(&mut store.project, &string(&v, "segment")?)?;
                    s.kana = string(&v, "kana")?;
                    s.mappings =
                        serde_json::from_value(v["mappings"].clone()).map_err(|e| e.to_string())?;
                    s.mappings.sort_by_key(|m| m.start);
                    validate_reference(s)?;
                    s.confirmed = true;
                }
                "mapping" => {
                    let s = segment(&mut store.project, &string(&v, "segment")?)?;
                    let start = v["start"].as_u64().ok_or("開始位置が必要です")? as usize;
                    let end = v["end"].as_u64().ok_or("終了位置が必要です")? as usize;
                    slice(&s.text, start, end)?;
                    let kana = string(&v, "replacement")?;
                    if kana.trim().is_empty()
                        || s.mappings.iter().any(|m| start < m.end && end > m.start)
                    {
                        return Err("読みが空または範囲が重複しています".into());
                    }
                    s.mappings.push(Mapping { start, end, kana });
                    s.mappings.sort_by_key(|m| m.start);
                    s.confirmed = false;
                }
                "clear_mappings" => {
                    let s = segment(&mut store.project, &string(&v, "segment")?)?;
                    s.mappings.clear();
                    s.confirmed = false;
                }
                "correction" => {
                    let s = segment(&mut store.project, &string(&v, "segment")?)?;
                    let start = v["start"].as_u64().ok_or("開始位置が必要です")? as usize;
                    let end = v["end"].as_u64().ok_or("終了位置が必要です")? as usize;
                    let original = slice(&s.text, start, end)?;
                    let replacement = string(&v, "replacement")?;
                    if replacement.trim().is_empty() {
                        return Err("置換する読みが必要です".into());
                    }
                    s.corrections.push(Correction {
                        id: id(),
                        start,
                        end,
                        original,
                        replacement,
                        decision: "pending".into(),
                    });
                }
                "decide" => {
                    let s = segment(&mut store.project, &string(&v, "segment")?)?;
                    let cid = string(&v, "id")?;
                    let decision = string(&v, "decision")?;
                    if !["accepted", "rejected", "pending"].contains(&decision.as_str()) {
                        return Err("判断が不正です".into());
                    }
                    s.corrections
                        .iter_mut()
                        .find(|c| c.id == cid)
                        .ok_or("補正がありません")?
                        .decision = decision;
                    corrected(s)?;
                }
                "transcript" => {
                    let s = segment(&mut store.project, &string(&v, "segment")?)?;
                    let rid = string(&v, "run")?;
                    let vid = string(&v, "variant")?;
                    let run = s
                        .comparisons
                        .iter_mut()
                        .find(|r| r.id == rid)
                        .ok_or("比較がありません")?;
                    run.variants
                        .iter_mut()
                        .find(|x| x.id == vid)
                        .ok_or("音声がありません")?
                        .transcript = string(&v, "text")?;
                }
                "rate" => {
                    let s = segment(&mut store.project, &string(&v, "segment")?)?;
                    let rid = string(&v, "run")?;
                    let vid = string(&v, "variant")?;
                    let rating: Rating =
                        serde_json::from_value(v["rating"].clone()).map_err(|e| e.to_string())?;
                    for n in [
                        rating.accuracy,
                        rating.naturalness,
                        rating.accent,
                        rating.rhythm,
                        rating.emotion,
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if !(1..=5).contains(&n) {
                            return Err("採点は1～5または未採点です".into());
                        }
                    }
                    let run = s
                        .comparisons
                        .iter_mut()
                        .find(|r| r.id == rid)
                        .ok_or("比較がありません")?;
                    let variant = run
                        .variants
                        .iter_mut()
                        .find(|x| x.id == vid && x.status == "succeeded")
                        .ok_or("生成済み音声がありません")?;
                    if hash_file(&safe(&store.root, &variant.file)?)? != variant.hash {
                        return Err("採点対象の音声が変わっています".into());
                    }
                    variant.rating = Some(rating);
                }
                "reveal" => {
                    let s = segment(&mut store.project, &string(&v, "segment")?)?;
                    let rid = string(&v, "run")?;
                    s.comparisons
                        .iter_mut()
                        .find(|r| r.id == rid)
                        .ok_or("比較がありません")?
                        .revealed = true;
                }
                _ => return Err("未対応の操作です".into()),
            }
            for s in &mut store.project.segments {
                let source = hash_bytes(s.text.as_bytes());
                let reference = s
                    .selected_take()
                    .map(|t| hash_bytes(format!("{}:{}", t.hash, s.kana).as_bytes()))
                    .unwrap_or_default();
                let c = corrected(s).unwrap_or_default();
                for r in &mut s.comparisons {
                    r.stale = r.source_hash != source
                        || r.reference_hash != reference
                        || r.variants.iter().any(|v| v.label == "C" && v.text != c)
                        || (!r.correction_hash.is_empty()
                            && r.correction_hash != hash_bytes(c.as_bytes()));
                }
            }
            store.save()
        })();
        if let Err(e) = result {
            store.project = previous;
            return Err(e);
        }
        a.message = "保存しました".into();
        Ok(json!(null))
    }
    fn record(&self, v: Value) -> Result<Value> {
        let mut a = self.0.lock().map_err(|e| e.to_string())?;
        if a.busy {
            return Err("処理中です".into());
        }
        let sid = string(&v, "segment")?;
        let endpoint = string(&v, "endpoint")?;
        let store = a.store.as_mut().ok_or("プロジェクトを開いてください")?;
        let tid = id();
        let file = format!("takes/{tid}/audio.wav");
        let path = safe(&store.root, &file)?;
        let root = store.root.clone();
        segment(&mut store.project, &sid)?.takes.push(Take {
            id: tid.clone(),
            file,
            hash: String::new(),
            status: "recording".into(),
            duration: 0.0,
            peak: 0.0,
            warning: String::new(),
            created: now(),
            endpoint: endpoint.clone(),
        });
        store.save()?;
        let stop = Arc::new(AtomicBool::new(false));
        a.stop = Some(stop.clone());
        a.busy = true;
        a.job = "recording".into();
        a.message = "録音準備中…停止すると保存します".into();
        let app = self.clone();
        std::thread::spawn(move || {
            let request = MicrophoneRecordingRequest::new(endpoint, path.clone(), 5 * 60 * 1000);
            let result =
                record_microphone_to_wav_controlled(&request, &stop, |peak, _, elapsed| {
                    if let Ok(mut a) = app.0.lock() {
                        a.peak = peak;
                        a.elapsed = elapsed;
                        a.message = "録音中".into();
                    }
                })
                .map_err(|e| e.to_string())
                .and_then(|report| {
                    json_new(&safe(&root, &format!("takes/{tid}/capture.json"))?, &report)?;
                    Ok(report)
                });
            let mut a = app.0.lock().unwrap();
            let outcome = (|| -> Result<()> {
                let store = a.store.as_mut().ok_or("保存先がありません")?;
                let t = segment(&mut store.project, &sid)?
                    .takes
                    .iter_mut()
                    .find(|t| t.id == tid)
                    .ok_or("テイクがありません")?;
                match result {
                    Ok(report) => {
                        t.duration = report.wav.audio_duration_ms as f64 / 1000.0;
                        t.peak = report.wav.peak;
                        t.hash = hash_file(&path)?;
                        t.status = "ready".into();
                        t.warning = if report.dropped_samples > 0 {
                            format!("音声欠落: {} samples。再録音を推奨", report.dropped_samples)
                        } else if t.peak >= 0.999 {
                            "クリッピングの可能性".into()
                        } else if t.peak < 0.01 {
                            "音量が小さいため確認してください".into()
                        } else {
                            String::new()
                        };
                    }
                    Err(e) => {
                        t.status = "failed".into();
                        t.warning = e;
                    }
                }
                store.save()
            })();
            a.busy = false;
            a.stop = None;
            a.peak = 0.0;
            a.message = outcome
                .err()
                .unwrap_or("録音処理が終了しました。テイクの状態を確認してください".into());
        });
        Ok(json!(null))
    }
}
