use crate::{
    alignment,
    app::{App, take_from_file},
    audio,
    domain::*,
    storage::*,
};
use serde_json::json;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
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
