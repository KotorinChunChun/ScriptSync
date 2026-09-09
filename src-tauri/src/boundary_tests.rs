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
