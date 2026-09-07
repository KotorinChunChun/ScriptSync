use crate::{
    app::{App, string},
    domain::*,
    storage::*,
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
impl App {
    pub fn batch(&self, v: Value) -> Result<Value> {
        let mut a = self.0.lock().map_err(|e| e.to_string())?;
        if a.busy || a.queue_stop.is_some() {
            return Err("処理中です".into());
        }
        let store = a.store.as_mut().ok_or("プロジェクトがありません")?;
        let items = if let Some(retry) = v["retry"].as_str() {
            let old = store
                .project
                .batches
                .iter()
                .find(|b| b.id == retry)
                .ok_or("一括履歴がありません")?;
            old.items
                .iter()
                .filter(|i| i.status != "succeeded")
                .map(|i| {
                    let mut i = i.clone();
                    i.status = "pending".into();
                    i.error.clear();
                    if i.action == "compare" {
                        if let Some(r) = &i.result_id {
                            if store
                                .project
                                .segments
                                .iter()
                                .flat_map(|s| &s.comparisons)
                                .any(|run| {
                                    run.id == *r && run.variants.iter().any(|v| v.plan.is_object())
                                })
                            {
                                i.payload["retry"] = json!(r);
                            } else if let Some(obj) = i.payload.as_object_mut() {
                                obj.remove("retry");
                            }
                        }
                    }
                    i
                })
                .collect::<Vec<_>>()
        } else {
            let action = string(&v, "kind")?;
            if action != "asr" && action != "compare" {
                return Err("一括処理の種類が不正です".into());
            }
            let ids = v["segments"].as_array().ok_or("発話を選択してください")?;
            let mut seen = std::collections::HashSet::new();
            let mut items = vec![];
            for sid in ids {
                let sid = sid.as_str().ok_or("発話IDが不正です")?;
                if !seen.insert(sid) {
                    continue;
                }
                let s = store
                    .project
                    .segments
                    .iter()
                    .find(|s| s.id == sid)
                    .ok_or("発話がありません")?;
                let mut payload =
                    json!({"segment":sid,"fixed":v["fixed"].as_bool().unwrap_or(true)});
                if action == "asr" {
                    let t = s.selected_take()?;
                    payload["file"] = json!(t.file);
                    payload["hash"] = json!(t.hash);
                }
                items.push(BatchItem {
                    action: action.clone(),
                    payload,
                    status: "pending".into(),
                    error: String::new(),
                    result_id: None,
                    source_hash: hash_bytes(s.text.as_bytes()),
                });
            }
            items
        };
        if items.is_empty() {
            return Err("処理対象がありません".into());
        }
        let bid = id();
        store.project.batches.push(Batch {
            id: bid.clone(),
            items,
        });
        store.save()?;
        let stop = Arc::new(AtomicBool::new(false));
        a.queue_stop = Some(stop.clone());
        a.message = "一括処理を開始します".into();
        let app = self.clone();
        let result = bid.clone();
        std::thread::spawn(move || {
            let outcome = app.run_batch(&bid, &stop);
            let mut a = app.0.lock().unwrap();
            a.queue_stop = None;
            a.message = outcome
                .err()
                .unwrap_or("一括処理が終了しました。各発話の結果を確認してください".into());
        });
        Ok(json!(result))
    }
    fn run_batch(&self, bid: &str, stop: &AtomicBool) -> Result<()> {
        let count = {
            let a = self.0.lock().unwrap();
            a.store
                .as_ref()
                .unwrap()
                .project
                .batches
                .iter()
                .find(|b| b.id == bid)
                .unwrap()
                .items
                .len()
        };
        for index in 0..count {
            let item = {
                let mut a = self.0.lock().unwrap();
                let store = a.store.as_mut().unwrap();
                let item = &mut store
                    .project
                    .batches
                    .iter_mut()
                    .find(|b| b.id == bid)
                    .unwrap()
                    .items[index];
                item.status = if stop.load(Ordering::Acquire) {
                    "cancelled"
                } else {
                    "running"
                }
                .into();
                let item = item.clone();
                store.save()?;
                item
            };
            if item.status == "cancelled" {
                continue;
            }
            let source_valid = {
                let a = self.0.lock().unwrap();
                a.store.as_ref().unwrap().project.segments.iter().any(|s| {
                    Some(s.id.as_str()) == item.payload["segment"].as_str()
                        && hash_bytes(s.text.as_bytes()) == item.source_hash
                })
            };
            let launched = if !source_valid {
                Err("原稿が変更されています。新しい一括処理を作成してください".into())
            } else if item.action == "compare" {
                self.compare(item.payload.clone())
            } else {
                self.transcribe(item.payload.clone())
            };
            let mut error = launched.as_ref().err().cloned().unwrap_or_default();
            let rid = launched.ok().and_then(|v| v.as_str().map(str::to_owned));
            if error.is_empty() {
                loop {
                    {
                        let a = self.0.lock().unwrap();
                        if stop.load(Ordering::Acquire) {
                            if let Some(s) = &a.stop {
                                s.store(true, Ordering::Release);
                            }
                        }
                        if !a.busy {
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            let mut a = self.0.lock().unwrap();
            let store = a.store.as_mut().unwrap();
            if error.is_empty() {
                let sid = string(&item.payload, "segment")?;
                let s = store
                    .project
                    .segments
                    .iter()
                    .find(|s| s.id == sid)
                    .ok_or("発話がありません")?;
                if item.action == "compare" {
                    if let Some(run) = s.comparisons.iter().find(|r| Some(&r.id) == rid.as_ref()) {
                        error = run
                            .variants
                            .iter()
                            .filter(|v| v.status != "succeeded")
                            .map(|v| v.error.as_str())
                            .collect::<Vec<_>>()
                            .join(" ／ ");
                        if run.variants.iter().any(|v| v.status != "succeeded") && error.is_empty()
                        {
                            error = "未完了の比較音声があります".into();
                        }
                    } else {
                        error = "比較結果がありません".into();
                    }
                } else {
                    if let Some(r) = s.transcriptions.last() {
                        if r["status"] != "succeeded" {
                            error = r["error"].as_str().unwrap_or("認識失敗").into();
                        }
                    } else {
                        error = "認識結果がありません".into();
                    }
                }
            }
            let item = &mut store
                .project
                .batches
                .iter_mut()
                .find(|b| b.id == bid)
                .unwrap()
                .items[index];
            item.status = if error.is_empty() {
                "succeeded"
            } else if stop.load(Ordering::Acquire) {
                "cancelled"
            } else {
                "failed"
            }
            .into();
            item.error = error;
            item.result_id = rid;
            store.save()?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_records_failures_and_rejects_edits() {
        let root = std::env::temp_dir().join(id());
        let mut store = Store::open(root.clone(), true).unwrap();
        store.project.segments.push(Segment::new("未確定".into()));
        let sid = store.project.segments[0].id.clone();
        let app = App::default();
        app.0.lock().unwrap().store = Some(store);
        app.batch(json!({"kind":"compare","segments":[sid]}))
            .unwrap();
        for _ in 0..100 {
            if app.0.lock().unwrap().queue_stop.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            app.snapshot().unwrap()["project"]["batches"][0]["items"][0]["status"],
            "failed"
        );
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }
}
