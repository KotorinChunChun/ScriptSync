use crate::{
    alignment,
    app::{App, take_from_file},
    audio,
    domain::*,
    storage::*,
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

#[test]
fn markdown_project_and_imported_take_round_trip() {
    let root = std::env::temp_dir().join(format!("scriptsync-test-{}", id()));
    fs::create_dir(&root).unwrap();
    let project_path = root.join("検証.scriptsync");
    let markdown_path = root.join("検証原稿.md");
    let wav_path = root.join("検証音声.wav");
    let first_line = "最初の検証用原稿です。";
    let second_line = "次の検証用原稿です。";
    let markdown = format!("{first_line}\n{second_line}\n");
    fs::write(&markdown_path, &markdown).unwrap();
    let mut wav = hound::WavWriter::create(
        &wav_path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..4_800 {
        wav.write_sample(0_i16).unwrap();
    }
    wav.finalize().unwrap();

    let location_path = root.join("settings/project-location.json");
    let isolated_app = || {
        let app = App::default();
        {
            let mut inner = app.0.lock().unwrap();
            inner.project_location =
                crate::project_location::ProjectLocation::load(location_path.clone());
            inner.machine_settings = crate::machine_settings::MachineSettings::load(
                root.join("settings/machine-settings.json"),
                root.join("asr"),
            );
        }
        app
    };
    let app = isolated_app();
    let project_argument = json!({"path": project_path});
    app.dispatch("new", project_argument.clone()).unwrap();
    let before = app.snapshot().unwrap()["project"].clone();
    let imported = app
        .dispatch("markdown_drop", json!({"paths": [markdown_path]}))
        .unwrap();
    let file = &imported["files"][0];
    assert_eq!(file["name"], "検証原稿.md");
    assert_eq!(file["text"], markdown);
    assert_eq!(app.snapshot().unwrap()["project"], before);

    let source_id = id();
    app.dispatch(
        "script_commit",
        json!({
            "project_id": before["id"],
            "base_revision": before["script_revision"],
            "base_generation": before["generation"],
            "sources": [{
                "id": source_id,
                "name": file["name"],
                "kind": "markdown",
                "text": file["text"],
                "approved_text": file["text"],
                "lines": [{"line": 0, "enabled": true}, {"line": 1, "enabled": true}]
            }],
            "deleted_source_ids": []
        }),
    )
    .unwrap();
    let confirmed = app.snapshot().unwrap()["project"].clone();
    app.dispatch("save_project", json!({})).unwrap();
    app.dispatch("close_project", json!({})).unwrap();
    drop(app);

    assert_eq!(
        serde_json::from_slice::<PathBuf>(&fs::read(&location_path).unwrap()).unwrap(),
        root
    );
    let app = isolated_app();
    app.dispatch("open", project_argument.clone()).unwrap();
    let reopened = app.snapshot().unwrap()["project"].clone();
    assert_eq!(reopened["id"], before["id"]);
    assert_eq!(
        reopened["script_revision"].as_u64().unwrap(),
        before["script_revision"].as_u64().unwrap() + 1
    );
    assert_eq!(reopened["script_sources"], confirmed["script_sources"]);
    assert_eq!(reopened["script_sources"][0]["confirmed_text"], markdown);
    let segments = reopened["segments"].as_array().unwrap();
    assert_eq!(segments.len(), 2);
    for (line, expected_text) in [first_line, second_line].into_iter().enumerate() {
        let segment = &segments[line];
        assert_eq!(segment["id"], confirmed["segments"][line]["id"]);
        assert_eq!(segment["text"], expected_text);
        assert_eq!(segment["source_id"], source_id);
        assert_eq!(segment["source_line"], line);
    }

    let segment_id = &segments[0]["id"];
    app.dispatch("import_audio", json!({"segment": segment_id, "path": wav_path}))
        .unwrap();
    let with_take = app.snapshot().unwrap()["project"].clone();
    let take_id = &with_take["segments"][0]["takes"][0]["id"];
    app.dispatch("select", json!({"segment": segment_id, "take": take_id}))
        .unwrap();
    app.dispatch("save_project", json!({})).unwrap();
    app.dispatch("close_project", json!({})).unwrap();
    drop(app);

    let app = isolated_app();
    app.dispatch("open", project_argument).unwrap();
    let final_state = app.snapshot().unwrap();
    let project = &final_state["project"];
    assert_eq!(project["script_sources"], reopened["script_sources"]);
    assert_eq!(project["segments"][1], segments[1]);
    let segment = &project["segments"][0];
    assert_eq!(&segment["id"], segment_id);
    assert_eq!(&segment["selected"], take_id);
    let takes = segment["takes"].as_array().unwrap();
    assert_eq!(takes.len(), 1);
    let take = &takes[0];
    assert_eq!(&take["id"], take_id);
    assert_eq!(take["script_text"], first_line);
    assert_eq!(take["status"], "ready");
    let stored_wav = safe(
        &PathBuf::from(final_state["root"].as_str().unwrap()),
        take["file"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(take["hash"], hash_file(&wav_path).unwrap());
    assert_eq!(fs::read(stored_wav).unwrap(), fs::read(wav_path).unwrap());
    app.dispatch("close_project", json!({})).unwrap();
    fs::remove_dir_all(root).unwrap();
}

fn wait(app: &App) {
    let start = Instant::now();
    while app.snapshot().unwrap()["busy"] == true {
        assert!(
            start.elapsed() < Duration::from_secs(360),
            "処理タイムアウト"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}
#[test]
#[ignore = "VoiSona Talkの起動と合成fixtureが必要。SCRIPT_SYNC_FIXTURESを指定"]
fn synthetic_end_to_end() {
    let fixtures =
        PathBuf::from(std::env::var("SCRIPT_SYNC_FIXTURES").expect("合成fixtureのパスが必要"));
    let root = fixtures
        .parent()
        .unwrap()
        .join(format!("verified-project-{}", id()));
    let mut store = Store::open(root.clone(), true).unwrap();
    store.project.title = "合成音声による動作検証（人間音声ではありません）".into();
    let app = App::default();
    app.0.lock().unwrap().store = Some(store);
    app.dispatch(
        "append",
        json!({"text":"市場へ行きます。\n人気のない道で人気の商品を見ました。"}),
    )
    .unwrap();
    let snap = app.snapshot().unwrap();
    let sid = snap["project"]["segments"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let second = snap["project"]["segments"][1]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for (target, file) in [
        (&sid, "market-reference"),
        (&sid, "market-wrong"),
        (&second, "repeat-reference"),
    ] {
        let mut a = app.0.lock().unwrap();
        let store = a.store.as_mut().unwrap();
        let take = take_from_file(
            &root,
            &fixtures.join(file).join("audio.wav"),
            "synthetic fixture".into(),
        )
        .unwrap();
        store
            .project
            .segments
            .iter_mut()
            .find(|s| s.id == *target)
            .unwrap()
            .takes
            .push(take);
        store.save().unwrap();
    }
    for target in [&sid, &second] {
        let snap = app.snapshot().unwrap();
        let tid = snap["project"]["segments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == *target)
            .unwrap()["takes"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        app.dispatch("select", json!({"segment":target,"take":tid}))
            .unwrap();
    }
    app.dispatch("reference",json!({"segment":sid,"kana":"いちばへいきます。","mappings":[{"start":0,"end":2,"kana":"いちば"}]})).unwrap();
    assert!(
        !alignment::differences("いちばへいきます", "しじょうへいきます")
            .unwrap()
            .is_empty()
    );
    app.dispatch(
        "correction",
        json!({"segment":sid,"start":0,"end":2,"replacement":"いちば"}),
    )
    .unwrap();
    let cid = app.snapshot().unwrap()["project"]["segments"][0]["corrections"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    app.dispatch(
        "decide",
        json!({"segment":sid,"id":cid,"decision":"accepted"}),
    )
    .unwrap();
    app.dispatch("compare", json!({"segment":sid,"fixed":true}))
        .unwrap();
    wait(&app);
    let snap = app.snapshot().unwrap();
    let run = &snap["project"]["segments"][0]["comparisons"][0];
    assert!(
        run["variants"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["status"] == "succeeded"),
        "{}",
        snap["message"]
    );
    let rid = run["id"].as_str().unwrap().to_owned();
    let vid = run["variants"][0]["id"].as_str().unwrap().to_owned();
    app.dispatch("rate",json!({"segment":sid,"run":rid,"variant":vid,"rating":{"accuracy":null,"naturalness":null,"accent":null,"rhythm":null,"emotion":null,"comment":"自動試験。聴感は未採点。","created":now()}})).unwrap();
    let export = app.dispatch("export", json!({})).unwrap();
    assert!(export["frames"].as_u64().unwrap() > 0);
    assert_eq!(export["entries"].as_array().unwrap().len(), 2);
    app.dispatch("report", json!({})).unwrap();
    app.dispatch("reference",json!({"segment":second,"kana":"ひとけのないみちでにんきのしょうひんをみました。","mappings":[]})).unwrap();
    app.dispatch(
        "batch",
        json!({"kind":"compare","segments":[sid,second],"fixed":true}),
    )
    .unwrap();
    wait(&app);
    let snap = app.snapshot().unwrap();
    assert!(
        snap["project"]["batches"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["status"] == "succeeded"),
        "{}",
        snap["project"]["batches"]
    );
    let run = snap["project"]["segments"][0]["comparisons"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    let base = run["id"].as_str().unwrap();
    let c = run["variants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["label"] == "C")
        .unwrap();
    let talk = app
        .dispatch(
            "talk_export",
            json!({"segment":sid,"run":base,"variant":c["id"]}),
        )
        .unwrap();
    let inspected =
        crate::talk_project::inspect_file(std::path::Path::new(talk["path"].as_str().unwrap()))
            .unwrap();
    assert_eq!(inspected["utterances"][0]["text"], "いちばへ行きます。");
    app.dispatch(
        "compare",
        json!({"segment":sid,"base_run":base,"punctuation":"いちばへ、行きます。"}),
    )
    .unwrap();
    wait(&app);
    let snap = app.snapshot().unwrap();
    let punctuation = snap["project"]["segments"][0]["comparisons"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert_eq!(punctuation["variants"].as_array().unwrap().len(), 2);
    assert!(
        punctuation["variants"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["status"] == "succeeded"),
        "{}",
        punctuation
    );
    app.dispatch("batch", json!({"kind":"asr","segments":[sid,second]}))
        .unwrap();
    wait(&app);
    let snap = app.snapshot().unwrap();
    assert!(
        snap["project"]["batches"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["status"] == "succeeded"),
        "{}",
        snap["project"]["batches"]
    );
    let t = &snap["project"]["segments"][0]["takes"][0];
    let wave = app
        .dispatch("waveform", json!({"file":t["file"],"hash":t["hash"]}))
        .unwrap();
    assert!(wave["peaks"].as_array().unwrap().len() > 0);
    let preview = app
        .dispatch(
            "preview_edit",
            json!({"segment":sid,"seam":true,"gain_db":-3.0}),
        )
        .unwrap();
    assert_eq!(preview["entries"].as_array().unwrap().len(), 2);
    json_new(&root.join("feature-verification.json"),&json!({"batch_synthesis":true,"batch_asr":true,"punctuation_synthesis":true,"talk_binary_roundtrip":talk,"talk_gui_verified":false,"waveform":true,"seam_preview":preview})).unwrap();

    {
        let mut a = app.0.lock().unwrap();
        a.store.take();
    }
    let reopened = Store::open(root.clone(), false).unwrap();
    assert_eq!(reopened.project.segments[0].takes.len(), 2);
    assert!(reopened.project.segments[0].confirmed);
    assert!(
        reopened.project.segments[0].comparisons[0].variants[0]
            .rating
            .is_some()
    );
    let repeated = audio::assemble(&root, &reopened.project).unwrap();
    assert_eq!(export["hash"], repeated["hash"]);
    json_new(&root.join("verification.json"),&json!({"synthetic":true,"human_quality_evaluated":false,"test":"import/select/reference/correct/synthesize/rate/export/reopen","success":true,"export":export})).unwrap();
    println!("検証プロジェクト: {}", root.display());
}

#[test]
#[ignore = "ローカルASRモデルとSCRIPT_SYNC_FIXTURESが必要"]
fn synthetic_asr_measurement() {
    let fixtures = PathBuf::from(std::env::var("SCRIPT_SYNC_FIXTURES").unwrap());
    let root = fixtures
        .parent()
        .unwrap()
        .join(format!("asr-measurement-{}", id()));
    let mut store = Store::open(root.clone(), true).unwrap();
    store
        .project
        .segments
        .push(Segment::new("ASR実測。期待原稿は認識器に渡さない。".into()));
    let sid = store.project.segments[0].id.clone();
    let app = App::default();
    app.0.lock().unwrap().store = Some(store);
    let cases = [
        ("market-reference", "いちばへいきます。"),
        ("market-wrong", "しじょうへいきます。"),
        (
            "repeat-reference",
            "ひとけのないみちでにんきのしょうひんをみました。",
        ),
        (
            "repeat-wrong",
            "にんきのないみちでにんきのしょうひんをみました。",
        ),
    ];
    let mut measurements = Vec::new();
    for (name, expected) in cases {
        let take = {
            let mut a = app.0.lock().unwrap();
            let store = a.store.as_mut().unwrap();
            let t = take_from_file(
                &root,
                &fixtures.join(name).join("audio.wav"),
                "synthetic".into(),
            )
            .unwrap();
            store.project.segments[0].takes.push(t.clone());
            store.save().unwrap();
            t
        };
        app.dispatch(
            "asr",
            json!({"segment":sid,"file":take.file,"hash":take.hash}),
        )
        .unwrap();
        wait(&app);
        let snapshot = app.snapshot().unwrap();
        let result = snapshot["project"]["segments"][0]["transcriptions"]
            .as_array()
            .unwrap()
            .last()
            .unwrap();
        assert!(result["error"].is_null(), "{result}");
        assert_eq!(result["audio_hash"], take.hash);
        assert_eq!(snapshot["project"]["segments"][0]["confirmed"], false);
        let actual = result["kana"].as_str().unwrap();
        measurements.push(json!({"case":name,"expected_from_synthesis_plan":expected,"asr":result,"kana_match":normalize(expected)==normalize(actual)}));
    }
    json_new(&root.join("measurement.json"),&json!({"synthetic":true,"measurements":measurements,"decision":"手動確認を必須とする認識支援。自動正解判定には使用しない"})).unwrap();
    println!("ASR実測: {}", root.display());
}
