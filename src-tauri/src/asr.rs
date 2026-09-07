use crate::{
    app::{App, segment, string},
    domain::*,
    storage::*,
};
use serde_json::{Value, json};
use std::{
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
impl App {
    pub fn transcribe(&self, payload: Value) -> Result<Value> {
        let mut a = self.0.lock().map_err(|e| e.to_string())?;
        if a.busy {
            return Err("処理中です".into());
        }
        let sid = string(&payload, "segment")?;
        let file = string(&payload, "file")?;
        let expected = string(&payload, "hash")?;
        let store = a.store.as_mut().ok_or("プロジェクトを開いてください")?;
        let s = segment(&mut store.project, &sid)?;
        let owned = s.takes.iter().any(|t| t.file == file && t.hash == expected)
            || s.comparisons.iter().any(|r| {
                r.variants
                    .iter()
                    .any(|v| v.file == file && v.hash == expected)
            });
        if !owned {
            return Err("この発話の音声ではありません".into());
        }
        let input = safe(&store.root, &file)?;
        if hash_file(&input)? != expected {
            return Err("音声が変わっています".into());
        }
        let run_id = id();
        let folder = safe(&store.root, &format!("asr/{run_id}"))?;
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let tools = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tools/asr");
        let python = tools.join("venv/Scripts/python.exe");
        if !python.exists() {
            return Err(
                "ローカル認識の準備が必要です。dev/scripts/setup-asr.ps1を実行してください".into(),
            );
        }
        let stop = Arc::new(AtomicBool::new(false));
        a.stop = Some(stop.clone());
        a.busy = true;
        a.job = "asr".into();
        a.message = "ローカルで音声を認識しています。音声は外部送信しません".into();
        let app = self.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<Value> {
                let out =
                    std::fs::File::create(folder.join("result.json")).map_err(|e| e.to_string())?;
                let err =
                    std::fs::File::create(folder.join("stderr.txt")).map_err(|e| e.to_string())?;
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
                    if stop.load(Ordering::Acquire) || start.elapsed() > Duration::from_secs(600) {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err("音声認識を中断しました".into());
                    }
                    if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                        if !status.success() {
                            return Err(std::fs::read_to_string(folder.join("stderr.txt"))
                                .unwrap_or("音声認識に失敗しました".into()));
                        }
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                let mut result: Value = serde_json::from_slice(
                    &std::fs::read(folder.join("result.json")).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if hash_file(&input)? != expected {
                    return Err("認識中に音声が変わりました".into());
                }
                result["audio_hash"] = expected.clone().into();
                result["file"] = file.clone().into();
                result["id"] = run_id.clone().into();
                Ok(result)
            })();
            let mut a = app.0.lock().unwrap();
            let mut message = String::new();
            if let Some(store) = a.store.as_mut() {
                match result {
                    Ok(result) => {
                        if let Ok(s) = segment(&mut store.project, &sid) {
                            s.transcriptions.push(result);
                        }
                        message =
                            "認識候補を保存しました。実音声を聴いて読みを確認してください".into();
                    }
                    Err(e) => {
                        if let Ok(s) = segment(&mut store.project, &sid) {
                            s.transcriptions.push(json!({"id":run_id,"file":file,"audio_hash":expected,"error":e,"needs_review":true}));
                        }
                        message = format!("認識失敗: {e}");
                    }
                }
                if let Err(e) = store.save() {
                    message = format!("保存失敗: {e}");
                }
            }
            a.message = message;
            a.busy = false;
            a.stop = None;
        });
        Ok(json!(null))
    }
}
