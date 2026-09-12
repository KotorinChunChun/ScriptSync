use crate::{app::App, domain::*, storage::*};
use serde_json::json;
#[test]
fn failed_mutation_rolls_back_and_edits_clear_selection() {
    let root = std::env::temp_dir().join(id());
    let mut store = Store::open(root.clone(), true).unwrap();
    let s = Segment::new("市場".into());
    let sid = s.id.clone();
    store.project.segments.push(s);
    store.save().unwrap();
    let app = App::default();
    app.0.lock().unwrap().store = Some(store);
    let old = app.snapshot().unwrap();
    assert!(
        app.dispatch("edit", json!({"segment":sid,"text":"変更","gap":-1}))
            .is_err()
    );
    assert_eq!(app.snapshot().unwrap()["project"], old["project"]);
    app.dispatch("edit", json!({"segment":sid,"text":"変更","gap":0}))
        .unwrap();
    assert_eq!(
        app.snapshot().unwrap()["project"]["segments"][0]["selected"],
        json!(null)
    );
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn deleting_take_removes_audio_and_clears_selection() {
    let root = std::env::temp_dir().join(id());
    let mut store = Store::open(root.clone(), true).unwrap();
    let mut s = Segment::new("削除試験".into());
    let sid = s.id.clone();
    let tid = id();
    let relative = format!("takes/{tid}/audio.wav");
    let path = safe(&root, &relative).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"test").unwrap();
    s.takes.push(Take {
        script_text: String::new(),
        trim_start: 0.0,
        trim_end: 0.0,
        id: tid.clone(),
        file: relative,
        hash: "hash".into(),
        status: "ready".into(),
        duration: 1.0,
        peak: 0.1,
        warning: String::new(),
        created: now(),
        endpoint: String::new(),
    });
    s.selected = Some(tid.clone());
    store.project.segments.push(s);
    store.save().unwrap();
    let app = App::default();
    app.0.lock().unwrap().store = Some(store);
    app.dispatch("delete_take", json!({"segment":sid,"take":tid}))
        .unwrap();
    assert!(
        app.snapshot().unwrap()["project"]["segments"][0]["takes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        app.snapshot().unwrap()["project"]["segments"][0]["selected"],
        json!(null)
    );
    assert!(!path.exists());
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn source_candidates_preserve_occurrence_ranges() {
    let root = std::env::temp_dir().join(id());
    let mut store = Store::open(root.clone(), true).unwrap();
    let mut s = Segment::new("人気のない場所に人気の商品".into());
    let sid = s.id.clone();
    s.mappings = vec![
        Mapping {
            start: 0,
            end: 2,
            kana: "ひとけ".into(),
        },
        Mapping {
            start: 8,
            end: 10,
            kana: "にんき".into(),
        },
    ];
    store.project.segments.push(s);
    let app = App::default();
    app.0.lock().unwrap().store = Some(store);
    let result=app.dispatch("diff",json!({"segment":sid,"expected":"ひとけのないばしょににんきのしょうひん","observed":"にんきのないばしょににんきのしょうひん"})).unwrap();
    assert!(result.as_array().unwrap().iter().all(|d| {
        d["source_candidates"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["start"] == 0)
    }));
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn media_uses_project_audio_and_byte_ranges() {
    let root = std::env::temp_dir().join(id());
    let store = Store::open(root.clone(), true).unwrap();
    let path = root.join("sound.wav");
    let mut w = hound::WavWriter::create(&path, crate::audio::spec()).unwrap();
    for _ in 0..960 {
        w.write_sample(1000i16).unwrap();
    }
    w.finalize().unwrap();
    let hash = hash_file(&path).unwrap();
    let app = App::default();
    app.0.lock().unwrap().store = Some(store);
    let url = app
        .dispatch("audio", json!({"file":"sound.wav","hash":hash}))
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    let request = tauri::http::Request::builder()
        .uri(url)
        .header("Range", "bytes=0-43")
        .body(vec![])
        .unwrap();
    let response = crate::media::response(&app, request);
    assert_eq!(response.status(), 206);
    assert_eq!(&response.body()[..4], b"RIFF");
    assert_eq!(response.body().len(), 44);
    assert!(
        app.dispatch("audio", json!({"file":"../outside.wav","hash":hash}))
            .is_err()
    );
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn delayed_stop_is_scoped_to_the_current_capture_and_cancel_is_immediate() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let app = App::default();
    let first = Arc::new(AtomicBool::new(false));
    {
        let mut state = app.0.lock().unwrap();
        state.busy = true;
        state.job = "recording".into();
        state.stop = Some(first.clone());
        state.cancel_recording = Some(Arc::new(AtomicBool::new(false)));
    }
    app.dispatch("stop", json!({"delay_seconds":0.05})).unwrap();
    assert!(!first.load(Ordering::Acquire));
    assert_eq!(app.snapshot().unwrap()["stop_scheduled"], true);
    app.dispatch("cancel_record", json!({})).unwrap();
    assert!(first.load(Ordering::Acquire));
    let second = Arc::new(AtomicBool::new(false));
    app.0.lock().unwrap().stop = Some(second.clone());
    std::thread::sleep(std::time::Duration::from_millis(80));
    assert!(!second.load(Ordering::Acquire));
    app.dispatch("stop", json!({})).unwrap();
    assert!(second.load(Ordering::Acquire));
    assert!(app.dispatch("stop", json!({"delay_seconds":-1})).is_err());
}

#[test]
fn recording_draft_is_saved_while_recording_without_changing_confirmed_content() {
    let root=std::env::temp_dir().join(id());let mut store=Store::open(root.clone(),true).unwrap();
    let mut s=Segment::new("確定原稿".into());let sid=s.id.clone();s.kana="よみ".into();
    store.project.segments.push(s);store.save().unwrap();
    let app=App::default();{let mut a=app.0.lock().unwrap();a.store=Some(store);a.busy=true;a.job="recording".into();}
    app.dispatch("recording_draft",json!({"segment":sid,"text":"修正全文"})).unwrap();
    let state=app.snapshot().unwrap();assert_eq!(state["project"]["segments"][0]["text"],"確定原稿");
    assert_eq!(state["project"]["segments"][0]["recording_draft"],"修正全文");
    assert_eq!(state["project"]["segments"][0]["kana"],"よみ");
    assert!(app.dispatch("edit",json!({"segment":sid,"text":"直接変更"})).is_err());
    drop(app);let reopened=Store::open(root.clone(),false).unwrap();
    assert_eq!(reopened.project.segments[0].recording_draft.as_deref(),Some("修正全文"));
    drop(reopened);std::fs::remove_dir_all(root).unwrap();
}
