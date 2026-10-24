use crate::{
    app::{App, segment, string},
    audio,
    domain::*,
    storage::*,
};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
// CLI互換の内部引数をHTTPサービスへ変換する。外部プログラムは起動しない。
fn service(settings:&crate::machine_settings::Settings,args:&[String],_folder:&Path,stop:&AtomicBool)->Result<Value>{
 if stop.load(Ordering::Acquire){return Err("キャンセルしました".into())}
 let arg=|name:&str|args.iter().position(|x|x==name).and_then(|n|args.get(n+1)).ok_or_else(||format!("引数がありません：{name}"));
 if args.first().is_some_and(|x|x=="propose") { let text=std::fs::read_to_string(arg("--text-file")?).map_err(|e|e.to_string())?;return Ok(json!({"format":"ttscli.speech-plan","version":1,"text":text,"voice":null,"proposal":{"reading":text,"tone_parameters":{"speed":1}}})); }
 let plan:Value=serde_json::from_slice(&std::fs::read(arg("--plan")?).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 let bytes=crate::services::request(settings,"/v1/audio/speech",Some(json!({"model":settings.tts_model,"voice":settings.tts_voice,"input":plan["proposal"]["reading"].as_str().or(plan["text"].as_str()).ok_or("合成本文がありません")?,"speed":1})))?;
 if stop.load(Ordering::Acquire){return Err("キャンセルしました".into())}
 let output=arg("--output")?;write_new(Path::new(output),&bytes)?;audio::inspect(Path::new(output))?;
 Ok(json!({"actual":{"voice_name":settings.tts_voice,"voice_version":"server-unverified","model":settings.tts_model},"warning":"サーバーの実話者・版は別途確認が必要です"}))
}
pub fn execution_plan(original: &Value, text: &str, reading: &str) -> Result<Value> {
    if text.chars().count() > 500 || reading.chars().count() > 500 || reading.trim().is_empty() {
        return Err("合成原稿とかなは各500文字以内に分割してください".into());
    }
    let mut plan = original.clone();
    plan["version"] = 1.into();
    plan["text"] = text.into();
    let p = plan["proposal"].as_object_mut().ok_or("提案が不正です")?;
    p.insert("reading".into(), reading.into());
    for key in [
        "reading_replacements",
        "full_kana_reading",
        "reading_policy",
    ] {
        p.remove(key);
    }
    Ok(plan)
}
impl App {
    pub fn compare(&self, v: Value) -> Result<Value> {
        let mut a = self.0.lock().map_err(|e| e.to_string())?;
        if a.busy {
            return Err("処理中です".into());
        }
        let sid = string(&v, "segment")?;
        let store = a.store.as_mut().ok_or("プロジェクトを開いてください")?;
        let s = segment(&mut store.project, &sid)?;
        if !s.confirmed {
            return Err("正解読みを確定してください".into());
        }
        validate_reference(s)?;
        let take = s.selected_take()?;
        if hash_file(&safe(&store.root, &take.file)?)? != take.hash {
            return Err("正解音声が差し替えられています。再採用・再確認してください".into());
        }
        let reference_hash =
            hash_bytes(format!("{}:{}", s.selected_take()?.hash, s.kana).as_bytes());
        let text = s.text.clone();
        let base = corrected(s)?;
        let punctuation = v["punctuation"].as_str().map(str::to_owned);
        let (labels, texts) = if let Some(changed) = &punctuation {
            validate_punctuation(&base, changed)?;
            (vec!["P0", "P1"], vec![base.clone(), changed.clone()])
        } else {
            (
                vec!["A", "B", "C"],
                vec![s.text.clone(), s.kana.clone(), base.clone()],
            )
        };
        for t in &texts {
            if t.chars().count() > 500 {
                return Err("各候補は500文字以内に分割してください".into());
            }
        }
        let source_hash = hash_bytes(text.as_bytes());
        let seed = if let Some(base_run) = v["base_run"].as_str() {
            let old = s
                .comparisons
                .iter()
                .find(|r| r.id == base_run && !r.stale)
                .ok_or("現在の条件に合う比較がありません")?;
            let c = old
                .variants
                .iter()
                .find(|v| v.label == "C" && v.status == "succeeded" && v.text == base)
                .ok_or("局所補正音声がありません")?;
            if !c.actual["voice_name"].is_string() || !c.actual["voice_version"].is_string() {
                return Err("実話者が不明です".into());
            }
            let mut plan = c.plan.clone();
            plan["voice"] = json!({"voice_name":c.actual["voice_name"],"voice_version":c.actual["voice_version"]});
            Some(plan)
        } else if punctuation.is_some() && v["retry"].is_null() {
            return Err("基準となる比較を選択してください".into());
        } else if let Some(retry) = v["retry"].as_str() {
            let old = s
                .comparisons
                .iter()
                .find(|r| r.id == retry)
                .ok_or("再試行する比較がありません")?;
            if old.source_hash != source_hash
                || old.reference_hash != reference_hash
                || old.variants.iter().zip(&texts).any(|(a, b)| a.text != *b)
            {
                return Err(
                    "原稿・正解・補正が変わっています。新しい比較を作成してください".into(),
                );
            }
            let mut plan = old
                .variants
                .iter()
                .find(|x| x.plan.is_object())
                .ok_or("声条件が未作成です。新しく生成してください")?
                .plan
                .clone();
            if let Some(actual) = old
                .variants
                .iter()
                .find(|x| x.actual["voice_name"].is_string())
                .map(|x| &x.actual)
            {
                plan["voice"] = json!({"voice_name":actual["voice_name"],"voice_version":actual["voice_version"]});
            }
            Some(plan)
        } else if v["fixed"].as_bool() == Some(true) {
            Some(
                json!({"format":"ttscli.speech-plan","version":1,"text":text,"voice":null,"proposal":{"id":"speech","reading":text,"model_expression":"neutral","voice_expression":"neutral","reason":"利用者が選択した標準の固定声条件","tone_parameters":{"alp":0,"huskiness":0,"intonation":1,"pitch":0,"speed":1,"volume":0,"style":"","style_intensity":0}}}),
            )
        } else {
            None
        };
        let rid = id();
        let variants: Vec<_> = labels
            .iter()
            .zip(texts)
            .map(|(label, text)| Variant {
                id: id(),
                label: label.to_string(),
                text,
                file: String::new(),
                hash: String::new(),
                status: "pending".into(),
                error: String::new(),
                plan: Value::Null,
                actual: Value::Null,
                transcript: String::new(),
                rating: None,
            })
            .collect();
        let mut order: Vec<_> = variants.iter().map(|v| v.id.clone()).collect();
        order.sort();
        s.comparisons.push(Comparison {
            kind: if punctuation.is_some() {
                "punctuation"
            } else {
                "reading"
            }
            .into(),
            correction_hash: hash_bytes(base.as_bytes()),
            stale: false,
            reference_kana: s.kana.clone(),
            id: rid.clone(),
            source_hash,
            reference_hash,
            variants,
            order,
            revealed: false,
            created: now(),
        });
        store.save()?;
        let root = store.root.clone();
        let stop = Arc::new(AtomicBool::new(false));
        a.stop = Some(stop.clone());
        a.busy = true;
        a.job = "synthesis".into();
        a.message = "音声合成サーバーへ接続しています".into();
        let app = self.clone();
        let result_id = rid.clone();
        std::thread::spawn(move || {
            let result = run_comparison(&app, &root, &sid, &rid, &text, &stop, seed);
            let mut a = app.0.lock().unwrap();
            a.busy = false;
            a.stop = None;
            if let Err(e) = &result {
                if let Some(store) = a.store.as_mut() {
                    if let Ok(s) = segment(&mut store.project, &sid) {
                        if let Some(run) = s.comparisons.iter_mut().find(|r| r.id == rid) {
                            for variant in &mut run.variants {
                                if variant.status == "pending" || variant.status == "running" {
                                    variant.status = "failed".into();
                                    variant.error = e.clone();
                                }
                            }
                        }
                    }
                    let _ = store.save();
                }
            }
            a.message = result
                .err()
                .unwrap_or("比較音声を保存しました。試聴して採点できます".into());
        });
        Ok(json!(result_id))
    }
}
fn update(app: &App, sid: &str, rid: &str, vid: &str, f: impl FnOnce(&mut Variant)) -> Result<()> {
    let mut a = app.0.lock().map_err(|e| e.to_string())?;
    let store = a.store.as_mut().ok_or("プロジェクトがありません")?;
    let s = segment(&mut store.project, sid)?;
    let run = s
        .comparisons
        .iter_mut()
        .find(|r| r.id == rid)
        .ok_or("比較がありません")?;
    f(run
        .variants
        .iter_mut()
        .find(|v| v.id == vid)
        .ok_or("候補がありません")?);
    store.save()
}
fn run_comparison(
    app: &App,
    root: &Path,
    sid: &str,
    rid: &str,
    text: &str,
    stop: &AtomicBool,
    seed: Option<Value>,
) -> Result<()> {
    let settings=app.0.lock().map_err(|e|e.to_string())?.machine_settings.settings.clone();
    let folder = safe(root, &format!("runs/{rid}"))?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let input = folder.join("input.txt");
    write_new(&input, text.as_bytes())?;
    // original方針でも提案検証は必要。失敗は明示して手動固定plan経路で再試行可能にする。
    let proposal = if let Some(plan) = seed {
        plan
    } else {
        service(&settings,
            &[
                "propose".into(),
                "--text-file".into(),
                input.to_string_lossy().into(),
            ],
            &folder.join("proposal"),
            stop,
        )?
    };
    json_new(&folder.join("proposal.json"), &proposal)?;
    let variants = {
        let a = app.0.lock().unwrap();
        a.store
            .as_ref()
            .unwrap()
            .project
            .segments
            .iter()
            .find(|s| s.id == sid)
            .unwrap()
            .comparisons
            .iter()
            .find(|r| r.id == rid)
            .unwrap()
            .variants
            .clone()
    };
    let mut fixed = proposal.clone();
    let mut reference_actual: Option<Value> = None;
    for variant in variants {
        if stop.load(Ordering::Acquire) {
            return Err("キャンセルしました".into());
        }
        let plan = execution_plan(&fixed, text, &variant.text)?;
        let attempt = folder.join(&variant.label);
        std::fs::create_dir_all(&attempt).map_err(|e| e.to_string())?;
        let pp = attempt.join("plan.json");
        json_new(&pp, &plan)?;
        update(app, sid, rid, &variant.id, |v| {
            v.status = "running".into();
            v.plan = plan.clone();
        })?;
        {
            let mut a = app.0.lock().unwrap();
            a.message = format!("比較音声 {} を生成中", variant.label);
        }
        let wav = attempt.join("audio.wav");
        let result = service(&settings,
            &[
                "synthesize".into(),
                "--plan".into(),
                pp.to_string_lossy().into(),
                "--output".into(),
                wav.to_string_lossy().into(),
                "--result-json".into(),
                attempt.join("result.json").to_string_lossy().into(),
            ],
            &attempt.join("process"),
            stop,
        );
        match result {
            Ok(r) => {
                audio::inspect(&wav)?;
                let hash = hash_file(&wav)?;
                let actual = r["actual"].clone();
                if reference_actual.is_none() {
                    if actual["voice_name"].is_string() && actual["voice_version"].is_string() {
                        fixed["voice"] = json!({"voice_name":actual["voice_name"],"voice_version":actual["voice_version"]});
                    }
                    reference_actual = Some(actual.clone());
                }
                let condition_warning = if let Some(first) = &reference_actual {
                    if !same_conditions(first, &actual) {
                        "実際の声条件が不一致または不明です。厳密な比較は未確認"
                    } else {
                        ""
                    }
                } else {
                    ""
                };
                update(app, sid, rid, &variant.id, |v| {
                    v.status = "succeeded".into();
                    v.file = format!("runs/{rid}/{}/audio.wav", variant.label);
                    v.hash = hash;
                    v.actual = actual;
                    v.error = condition_warning.into();
                })?;
            }
            Err(e) => {
                update(app, sid, rid, &variant.id, |v| {
                    v.status = "failed".into();
                    v.error = e.clone();
                })?;
                if reference_actual.is_none() {
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}
pub fn validate_punctuation(base: &str, changed: &str) -> Result<()> {
    let without = |s: &str| s.chars().filter(|c| *c != '、').collect::<String>();
    if base == changed || without(base) != without(changed) {
        return Err("読点「、」の追加・削除・移動だけを指定してください".into());
    }
    Ok(())
}
pub fn same_conditions(a: &Value, b: &Value) -> bool {
    if !a["voice_name"].is_string()
        || !a["voice_version"].is_string()
        || a["voice_name"] != b["voice_name"]
        || a["voice_version"] != b["voice_version"]
    {
        return false;
    }
    fn near(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => {
                (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() <= 0.002
            }
            (Value::Array(x), Value::Array(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(a, b)| near(a, b))
            }
            (Value::Object(x), Value::Object(y)) => {
                x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|b| near(v, b)))
            }
            _ => a == b,
        }
    }
    a["global_parameters"].is_object() && near(&a["global_parameters"], &b["global_parameters"])
}
pub fn report(store: &Store) -> Result<Value> {
    let relative = format!("exports/report-{}/report.html", id());
    let path = safe(&store.root, &relative)?;
    let parent = path.parent().unwrap();
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    json_new(&parent.join("project.json"), &store.project)?;
    fn esc(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
    let mut html = String::from(
        "<!doctype html><html lang=ja><meta charset=utf-8><title>読みあわせ 評価</title><style>body{font:16px sans-serif;max-width:1000px;margin:40px auto}td,th{border:1px solid #bbb;padding:8px}table{border-collapse:collapse}</style><h1>読みあわせ 評価レポート</h1><p>空欄は未採点。音声と条件の実測を確認してください。</p>",
    );
    let mut csv = String::from("発話,比較ID,候補,状態,正確さ,自然さ,コメント\n");
    fn cell(s: &str) -> String {
        let safe = if s.starts_with(['=', '+', '-', '@']) {
            format!("'{s}")
        } else {
            s.into()
        };
        format!("\"{}\"", safe.replace('"', "\"\""))
    }
    for s in &store.project.segments {
        html += &format!("<h2>{}</h2>", esc(&s.text));
        for r in &s.comparisons {
            html += &format!(
                "<p>比較 {}</p><table><tr><th>候補</th><th>状態</th><th>読み</th><th>採点</th><th>注意</th></tr>",
                esc(&r.id)
            );
            for v in &r.variants {
                let rating = v.rating.as_ref();
                html += &format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                    esc(&v.label),
                    esc(&v.status),
                    esc(&v.text),
                    esc(&serde_json::to_string(&rating).unwrap()),
                    esc(&v.error)
                );
                csv += &format!(
                    "{},{},{},{},{},{},{}\n",
                    cell(&s.text),
                    cell(&r.id),
                    cell(&v.label),
                    cell(&v.status),
                    rating
                        .and_then(|x| x.accuracy)
                        .map(|n| n.to_string())
                        .unwrap_or_default(),
                    rating
                        .and_then(|x| x.naturalness)
                        .map(|n| n.to_string())
                        .unwrap_or_default(),
                    cell(rating.map(|x| x.comment.as_str()).unwrap_or(""))
                );
            }
            html += "</table>";
        }
    }
    html += "</html>";
    write_new(&path, html.as_bytes())?;
    write_new(&parent.join("ratings.csv"), csv.as_bytes())?;
    Ok(json!({"path":path}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conditions_tolerate_api_rounding() {
        let a = json!({"voice_name":"v","voice_version":"1","global_parameters":{"pitch":20.0}});
        let b = json!({"voice_name":"v","voice_version":"1","global_parameters":{"pitch":19.999}});
        assert!(same_conditions(&a, &b));
        assert!(!same_conditions(&a, &json!({})));
    }
}
