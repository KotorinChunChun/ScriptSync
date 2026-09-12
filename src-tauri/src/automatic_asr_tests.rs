use super::{apply_result, complete_job, Job};
use crate::app::App;
use crate::domain::{BlankScriptRow, Project, ScriptSource, Segment, Take};
use crate::storage::Store;
use serde_json::json;
use std::path::PathBuf;

fn fixture(segment_text: &str, source_id: &str) -> (Project, Job) {
    let mut project = Project::new("ASR test".into());
    let mut segment = Segment::new(segment_text.into());
    segment.id = "segment-1".into();
    segment.source_id = source_id.into();
    segment.source_line = Some(0);
    segment.revision = 7;
    segment.takes.push(Take {
        script_text: String::new(),
        trim_start: 0.0,
        trim_end: 0.0,
        id: "take-1".into(),
        file: "audio/take-1.wav".into(),
        hash: "hash-1".into(),
        status: "ready".into(),
        duration: 2.0,
        peak: 0.0,
        warning: String::new(),
        created: 0,
        endpoint: String::new(),
    });
    project.segments.push(segment);
    project.script_sources.push(ScriptSource {
        id: source_id.into(),
        name: "script.md".into(),
        kind: "markdown".into(),
        confirmed_text: "確定済みの基準本文".into(),
        draft_text: segment_text.into(),
        lines: vec![],
        draft_lines: vec![],
        blank_rows: vec![BlankScriptRow {
            segment_id: "segment-1".into(),
            line: 0,
        }],
        approved_text: None,
    });
    let job = Job {
        project: project.id.clone(),
        root: PathBuf::from("C:/scriptsync-asr-test"),
        segment: "segment-1".into(),
        take: "take-1".into(),
        file: "audio/take-1.wav".into(),
        hash: "hash-1".into(),
        revision: 7,
        run: "run-1".into(),
    };
    (project, job)
}

#[test]
fn result_is_rejected_when_project_take_or_hash_no_longer_matches() {
    for mismatch in ["project", "take", "hash"] {
        let (mut project, mut job) = fixture("本文", "source-1");
        match mismatch {
            "project" => job.project = "other-project".into(),
            "take" => job.take = "other-take".into(),
            "hash" => job.hash = "other-hash".into(),
            _ => unreachable!(),
        }
        assert!(apply_result(&mut project, &job, Ok(json!({"raw":"認識結果"}))).is_err());
        assert!(project.segments[0].transcriptions.is_empty(), "{mismatch}");
    }
}

#[test]
fn failed_recognition_is_retained_with_job_identity() {
    let (mut project, job) = fixture("本文", "source-1");
    apply_result(&mut project, &job, Err("認識エンジンが失敗しました".into())).unwrap();
    let record = &project.segments[0].transcriptions[0];
    assert_eq!(record["status"], "failed");
    assert_eq!(record["error"], "認識エンジンが失敗しました");
    assert_eq!(record["take_id"], "take-1");
    assert_eq!(record["audio_hash"], "hash-1");
    assert_eq!(record["needs_review"], true);
}

#[test]
fn nonempty_reading_is_never_overwritten() {
    let (mut project, job) = fixture("既存の読み上げ本文", "source-1");
    project.script_sources[0].draft_text = "手入力した下書き".into();
    apply_result(&mut project, &job, Ok(json!({"raw":"ASR結果"}))).unwrap();
    assert_eq!(project.segments[0].text, "既存の読み上げ本文");
    assert_eq!(project.script_sources[0].draft_text, "手入力した下書き");
    assert_eq!(project.segments[0].transcriptions[0]["auto_filled"], false);
}

#[test]
fn successful_blank_fill_keeps_confirmed_baseline() {
    let (mut project, job) = fixture("", "source-1");
    project.script_sources[0].confirmed_text = "確定済みの基準本文".into();
    apply_result(&mut project, &job, Ok(json!({"raw":"認識結果"}))).unwrap();
    assert_eq!(project.script_sources[0].draft_text, "認識結果");
    assert_eq!(
        project.script_sources[0].confirmed_text,
        "確定済みの基準本文"
    );
    assert_eq!(project.script_sources[0].approved_text, None);
    assert_eq!(project.segments[0].transcriptions[0]["auto_filled"], true);
}

#[test]
fn manual_revision_prevents_blank_fill() {
    let (mut project, job) = fixture("", "source-1");
    project.segments[0].revision = 8;
    project.script_sources[0].draft_text = "手入力した下書き".into();
    apply_result(&mut project, &job, Ok(json!({"raw":"認識結果"}))).unwrap();
    assert_eq!(project.script_sources[0].draft_text, "手入力した下書き");
    assert_eq!(project.segments[0].transcriptions[0]["auto_filled"], false);
}

#[test]
fn manual_blank_row_draft_is_not_overwritten() {
    let (mut project, job) = fixture("", "source-1");
    project.script_sources[0].draft_text = "手入力した下書き".into();
    apply_result(&mut project, &job, Ok(json!({"raw":"認識結果"}))).unwrap();
    assert_eq!(project.script_sources[0].draft_text, "手入力した下書き");
    assert_eq!(project.segments[0].text, "");
    assert_eq!(project.segments[0].transcriptions[0]["auto_filled"], false);
}

#[test]
fn blank_fill_updates_the_matching_script_source_after_helper_ready() {
    let (mut project, job) = fixture("", "source-1");
    apply_result(&mut project, &job, Ok(json!({"raw":"認識結果"}))).unwrap();
    assert_eq!(project.script_sources[0].draft_text, "認識結果");
    assert_eq!(
        project.script_sources[0].confirmed_text,
        "確定済みの基準本文"
    );
    assert_eq!(project.segments[0].transcriptions[0]["auto_filled"], true);
}

#[test]
fn stale_result_changes_running_record_to_stale_after_take_hash_change() {
    let path = std::env::temp_dir().join(format!(
        "scriptsync-asr-stale-{}.scriptsync",
        crate::domain::id()
    ));
    let mut store = Store::create_project_file(path.clone()).unwrap();
    let root = store.root.clone();
    let (mut project, mut job) = fixture("本文", "source-1");
    job.root = root.clone();
    project.segments[0].transcriptions.push(json!({
        "id": job.run,
        "take_id": job.take,
        "audio_hash": "hash-1",
        "status": "running"
    }));
    project.generation = store.project.generation;
    store.project = project;
    store.save().unwrap();
    let app = App::default();
    app.0.lock().unwrap().store = Some(store);
    app.0
        .lock()
        .unwrap()
        .store
        .as_mut()
        .unwrap()
        .project
        .segments[0]
        .takes[0]
        .hash = "changed".into();
    complete_job(&app, &job, Ok(json!({"raw":"遅れて届いた結果"})));
    let snapshot = app.snapshot().unwrap();
    let record = &snapshot["project"]["segments"][0]["transcriptions"][0];
    assert_eq!(record["status"], "stale");
    assert!(record["raw"].is_null());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_file(path).unwrap();
}

/// 実ワーカーと保存を通す検証。非機密の音声を環境変数で明示した場合だけ実行する。
#[test]
#[ignore = "ローカルfaster-whisperとSCRIPTSYNC_ASR_TEST_WAVが必要"]
fn real_worker_saves_transcript_and_preserves_baseline() {
    use crate::{
        app::{take_from_file, App},
        script_sources,
        storage::Store,
    };
    use std::time::{Duration, Instant};
    let input = std::env::var("SCRIPTSYNC_ASR_TEST_WAV").expect("試験音声を指定してください");
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("output/v0.0.2-validation")
        .join(format!("asr-{}", crate::domain::id()));
    std::fs::create_dir_all(&output).unwrap();
    let manifest = output.join("動作確認.scriptsync");
    let mut store = Store::create_project_file(manifest.clone()).unwrap();
    for name in ["01-説明.md", "02-確認.md", "03-録音台本"] {
        let revision = store.project.script_revision;
        script_sources::insert_empty(
            &mut store.project,
            &json!({"base_revision":revision,"name":name}),
        )
        .unwrap();
    }
    let sources:Vec<_>=store.project.script_sources.iter().map(|s|{
        let text=match s.name.as_str(){"01-説明.md"=>"はじめに。\nファイル順を確認します。","02-確認.md"=>"この行は録音を保持します。",_=>""};
        json!({"id":s.id,"name":s.name,"kind":s.kind,"text":text,"lines":crate::markdown::parse(text).unwrap(),"approved_text":text})
    }).collect();
    let revision = store.project.script_revision;
    script_sources::commit(
        &mut store.project,
        &json!({"base_revision":revision,"sources":sources}),
    )
    .unwrap();
    let source_id = store.project.script_sources.last().unwrap().id.clone();
    let revision = store.project.script_revision;
    script_sources::insert_empty(
        &mut store.project,
        &json!({"source_id":source_id,"base_revision":revision}),
    )
    .unwrap();
    let sid = store.project.segments.last().unwrap().id.clone();
    let take = take_from_file(
        &store.root,
        std::path::Path::new(&input),
        "ASR検証用合成音声".into(),
    )
    .unwrap();
    let tid = take.id.clone();
    store.project.segments.last_mut().unwrap().takes.push(take);
    store.save().unwrap();
    let app = App::default();
    app.0.lock().unwrap().machine_settings.runtime_root = std::env::var("SCRIPTSYNC_ASR_RUNTIME_ROOT").map(std::path::PathBuf::from).expect("設定画面で導入したASRのパスをSCRIPTSYNC_ASR_RUNTIME_ROOTに指定してください");
    app.0.lock().unwrap().store = Some(store);
    app.enqueue_recording_asr(&sid, &tid).unwrap();
    let start = Instant::now();
    loop {
        let state = app.snapshot().unwrap();
        let row = state["project"]["segments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == sid)
            .unwrap();
        let result = row["transcriptions"].as_array().unwrap().last().unwrap();
        if result["status"] == "succeeded" {
            assert!(!result["raw"].as_str().unwrap().trim().is_empty());
            assert_eq!(result["auto_filled"], true);
            assert!(!row["text"].as_str().unwrap().trim().is_empty());
            assert_eq!(state["project"]["script_revision"], revision);
            let source = state["project"]["script_sources"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["id"] == source_id)
                .unwrap();
            assert_eq!(source["confirmed_text"], "");
            println!(
                "検証プロジェクト: {}\n認識結果: {}",
                manifest.display(),
                result["raw"]
            );
            break;
        }
        assert_ne!(result["status"], "failed", "{result}");
        assert!(
            start.elapsed() < Duration::from_secs(180),
            "認識待ちが時間切れ: {result}"
        );
        std::thread::sleep(Duration::from_millis(150));
    }
}
