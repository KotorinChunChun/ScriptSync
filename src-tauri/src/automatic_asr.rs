//! 録音保存後のASR。録音操作のbusyとは独立した逐次キュー。
use crate::{app::App, domain::*, storage::*};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[cfg(test)]
#[path = "automatic_asr_tests.rs"]
mod tests;

#[derive(Clone)]
pub struct Job {
    project: String,
    root: PathBuf,
    segment: String,
    take: String,
    file: String,
    hash: String,
    revision: u64,
    run: String,
}
impl App {
    pub fn enqueue_recording_asr(&self, sid: &str, tid: &str) -> Result<()> {
        let mut a = self.0.lock().map_err(|e| e.to_string())?;
        let store = a.store.as_mut().ok_or("プロジェクトを開いてください")?;
        let s = store
            .project
            .segments
            .iter()
            .find(|s| s.id == sid)
            .ok_or("発話がありません")?;
        let t = s
            .takes
            .iter()
            .find(|t| t.id == tid && t.status == "ready")
            .ok_or("保存済みテイクがありません")?;
        if s.transcriptions.iter().any(|r| {
            r["take_id"] == tid
                && r["audio_hash"] == t.hash
                && matches!(r["status"].as_str(), Some("queued" | "running"))
        }) {
            return Ok(());
        }
        let job = Job {
            project: store.project.id.clone(),
            root: store.root.clone(),
            segment: sid.into(),
            take: tid.into(),
            file: t.file.clone(),
            hash: t.hash.clone(),
            revision: s.revision,
            run: id(),
        };
        let previous = store.project.clone();
        store.project.segments.iter_mut().find(|s|s.id==sid).unwrap().transcriptions.push(json!({"id":job.run,"take_id":tid,"file":job.file,"audio_hash":job.hash,"status":"queued","needs_review":true}));
        if let Err(e) = store.save() {
            store.project = previous;
            return Err(e);
        }
        a.auto_asr_queue.push_back(job);
        if !a.auto_asr_running {
            a.auto_asr_running = true;
            let app = self.clone();
            std::thread::spawn(move || worker(app));
        }
        Ok(())
    }
}
fn matches_job(p: &Project, job: &Job) -> bool {
    p.id == job.project
        && p.segments.iter().any(|s| {
            s.id == job.segment
                && s.takes
                    .iter()
                    .any(|t| t.id == job.take && t.hash == job.hash && t.status == "ready")
        })
}
fn recording(app: &App) -> bool {
    app.0
        .lock()
        .map(|a| a.busy || a.queue_stop.is_some())
        .unwrap_or(true)
}
fn current(app: &App, job: &Job) -> bool {
    app.0
        .lock()
        .map(|a| {
            a.store
                .as_ref()
                .is_some_and(|s| s.root == job.root && matches_job(&s.project, job))
        })
        .unwrap_or(false)
}
fn set_status(app: &App, job: &Job, status: &str) {
    if let Ok(mut a) = app.0.lock() {
        if let Some(store) = a
            .store
            .as_mut()
            .filter(|s| s.project.id == job.project && s.root == job.root)
        {
            let previous = store.project.clone();
            if let Some(record) = store
                .project
                .segments
                .iter_mut()
                .find(|s| s.id == job.segment)
                .and_then(|s| s.transcriptions.iter_mut().find(|r| r["id"] == job.run))
            {
                record["status"] = status.into();
                if store.save().is_err() {
                    store.project = previous;
                }
            }
        }
    }
}
enum Outcome {
    Done(Result<Value>),
    Paused,
}
fn recognize(app: &App, job: &Job) -> Outcome {
    let work = (|| -> Result<Option<Value>> {
        let tools = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tools/asr");
        let python = tools.join("venv/Scripts/python.exe");
        if !python.exists() {
            return Err("文字起こしの準備が必要です（dev/scripts/setup-asr.ps1）".into());
        }
        let input = safe(&job.root, &job.file)?;
        if hash_file(&input)? != job.hash {
            return Err("音声が変更されています".into());
        }
        let folder = safe(&job.root, &format!("asr/{}", job.run))?;
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let out = std::fs::File::create(folder.join("result.json")).map_err(|e| e.to_string())?;
        let err = std::fs::File::create(folder.join("stderr.txt")).map_err(|e| e.to_string())?;
        let mut command = Command::new(python);
        command
            .arg(tools.join("transcribe.py"))
            .arg("--audio")
            .arg(&input)
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let start = Instant::now();
        loop {
            if !current(app, job) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("対象のテイクまたはプロジェクトが変更されました".into());
            }
            if recording(app) {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(None);
            }
            if start.elapsed() > Duration::from_secs(600) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("文字起こしが時間切れになりました".into());
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                if !status.success() {
                    return Err(std::fs::read_to_string(folder.join("stderr.txt"))
                        .unwrap_or_else(|_| "文字起こしに失敗しました".into()));
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if hash_file(&input)? != job.hash {
            return Err("認識中に音声が変更されました".into());
        }
        let result: Value = serde_json::from_slice(
            &std::fs::read(folder.join("result.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(Some(result))
    })();
    match work {
        Ok(None) => Outcome::Paused,
        Ok(Some(r)) => Outcome::Done(Ok(r)),
        Err(e) => Outcome::Done(Err(e)),
    }
}
fn apply_result(p: &mut Project, job: &Job, result: Result<Value>) -> Result<()> {
    if !matches_job(p, job) {
        return Err("対象の音声が変更されています".into());
    }
    let mut record = match result {
        Ok(mut r) => {
            r["status"] = "succeeded".into();
            r
        }
        Err(e) => json!({"status":"failed","error":e}),
    };
    record["id"] = job.run.clone().into();
    record["take_id"] = job.take.clone().into();
    record["file"] = job.file.clone().into();
    record["audio_hash"] = job.hash.clone().into();
    record["needs_review"] = true.into();
    let text = record["raw"].as_str().unwrap_or("").trim().to_owned();
    if record["status"] == "succeeded" && !text.is_empty() {
        let eligible = p
            .segments
            .iter()
            .any(|s| s.id == job.segment && s.revision == job.revision && s.text.trim().is_empty());
        if eligible {
            // 確定本文は変更しない。下書きが競合していれば候補だけ保存。
            record["auto_filled"] =
                crate::script_sources::update_blank_draft(p, &job.segment, &text, job.revision)
                    .is_ok()
                    .into();
        } else {
            record["auto_filled"] = false.into();
        }
    }
    let s = p
        .segments
        .iter_mut()
        .find(|s| s.id == job.segment)
        .ok_or("発話がありません")?;
    if let Some(existing) = s.transcriptions.iter_mut().find(|r| r["id"] == job.run) {
        *existing = record;
    } else {
        s.transcriptions.push(record);
    }
    Ok(())
}
fn complete_job(app: &App, job: &Job, result: Result<Value>) {
    let mut status = None;
    if let Ok(mut a) = app.0.lock() {
        if let Some(store) = a
            .store
            .as_mut()
            .filter(|s| s.root == job.root && s.project.id == job.project)
        {
            let previous = store.project.clone();
            match apply_result(&mut store.project, job, result).and_then(|_| store.save()) {
                Ok(()) => {}
                Err(error) => {
                    let stale = store.project.segments.iter().any(|s| {
                        s.id == job.segment
                            && s.takes.iter().all(|t| {
                                t.id != job.take || t.hash != job.hash || t.status != "ready"
                            })
                    });
                    store.project = previous;
                    a.message = format!("文字起こし結果を保存できません: {error}");
                    status = Some(if stale { "stale" } else { "failed" });
                }
            }
        }
    }
    if let Some(status) = status {
        set_status(app, job, status);
    }
}
fn worker(app: App) {
    loop {
        let job = {
            let Ok(mut a) = app.0.lock() else { return };
            let Some(job) = a.auto_asr_queue.pop_front() else {
                a.auto_asr_running = false;
                return;
            };
            job
        };
        while recording(&app) && current(&app, &job) {
            std::thread::sleep(Duration::from_millis(150));
        }
        if !current(&app, &job) {
            set_status(&app, &job, "stale");
            continue;
        }
        set_status(&app, &job, "running");
        match recognize(&app, &job) {
            Outcome::Paused => {
                set_status(&app, &job, "queued");
                if let Ok(mut a) = app.0.lock() {
                    a.auto_asr_queue.push_front(job);
                }
            }
            Outcome::Done(result) => {
                complete_job(&app, &job, result);
            }
        }
    }
}
