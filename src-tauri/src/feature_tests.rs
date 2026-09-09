use crate::{
    app::{App, take_from_file},
    domain::*,
    rules,
    storage::*,
};
use serde_json::json;
fn scored_segment(root: &std::path::Path, text: &str) -> Segment {
    let mut s = Segment::new(text.into());
    s.corrections.push(Correction {
        id: id(),
        start: 0,
        end: 2,
        original: "市場".into(),
        replacement: "いちば".into(),
        decision: "accepted".into(),
    });
    let file = format!("{}.wav", id());
    let path = root.join(&file);
    let mut w = hound::WavWriter::create(&path, crate::audio::spec()).unwrap();
    for _ in 0..960 {
        w.write_sample(1000i16).unwrap();
    }
    w.finalize().unwrap();
    let variants:Vec<_>=["A","C"].iter().map(|label|Variant{id:id(),label:(*label).into(),text:if *label=="A"{s.text.clone()}else{corrected(&s).unwrap()},file:file.clone(),hash:hash_file(&path).unwrap(),status:"succeeded".into(),error:String::new(),plan:json!({}),actual:json!({"voice_name":"tanaka-san_ja_JP","voice_version":"2.0.1","global_parameters":{"pitch":0}}),transcript:String::new(),rating:Some(Rating{accuracy:Some(if *label=="A"{2}else{5}),naturalness:Some(4),accent:None,rhythm:None,emotion:None,comment:"自動テスト専用の仮採点".into(),created:now()})}).collect();
    s.comparisons.push(Comparison {
        kind: "reading".into(),
        correction_hash: hash_bytes(corrected(&s).unwrap().as_bytes()),
        stale: false,
        reference_kana: String::new(),
        id: id(),
        source_hash: hash_bytes(text.as_bytes()),
        reference_hash: String::new(),
        order: variants.iter().map(|v| v.id.clone()).collect(),
        variants,
        revealed: true,
        created: now(),
    });
    s
}
#[test]
fn rules_require_unseen_validation_and_detect_tampering() {
    let root = std::env::temp_dir().join(id());
    let mut store = Store::open(root.clone(), true).unwrap();
    let source = scored_segment(&root, "市場へ行く");
    let sid = source.id.clone();
    let rid = source.comparisons[0].id.clone();
    store.project.segments.push(source);
    rules::mutate(
        &mut store,
        "rule_create",
        &json!({"segment":sid,"run":rid,"dictionary":"試験辞書"}),
    )
    .unwrap();
    let rule = store.project.rules[0].id.clone();
    assert!(
        rules::mutate(
            &mut store,
            "rule_decide",
            &json!({"id":rule,"decision":"approved"})
        )
        .is_err()
    );
    assert!(
        rules::mutate(
            &mut store,
            "rule_validate",
            &json!({"id":rule,"run":rid,"dictionary":"試験辞書"})
        )
        .is_err()
    );
    let validation = scored_segment(&root, "市場で買う");
    let vid = validation.comparisons[0].id.clone();
    store.project.segments.push(validation);
    rules::mutate(
        &mut store,
        "rule_validate",
        &json!({"id":rule,"run":vid,"dictionary":"試験辞書"}),
    )
    .unwrap();
    rules::mutate(
        &mut store,
        "rule_decide",
        &json!({"id":rule,"decision":"approved"}),
    )
    .unwrap();
    assert!(rules::export(&store).is_ok());
    std::fs::write(
        root.join(&store.project.rules[0].source.variants[0].file),
        b"tampered",
    )
    .unwrap();
    assert!(rules::export(&store).is_err());
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn punctuation_changes_cannot_change_reading() {
    assert!(crate::speech::validate_punctuation("市場へ行く", "市場へ、行く").is_ok());
    assert!(crate::speech::validate_punctuation("市場へ行く", "いちばへ、行く").is_err());
    assert!(crate::speech::validate_punctuation("市場へ行く", "市場へ行く").is_err());
}
#[test]
fn split_recorded_segment_reopens_with_history_and_distinct_ids() {
    let root = std::env::temp_dir().join(id());
    let mut store = Store::open(root.clone(), true).unwrap();
    let mut s = scored_segment(&root, "市場へ行く");
    let t = take_from_file(
        &root,
        &root.join(&s.comparisons[0].variants[0].file),
        "fixture".into(),
    )
    .unwrap();
    s.takes.push(t);
    let sid = s.id.clone();
    store.project.segments.push(s);
    store.save().unwrap();
    let app = App::default();
    app.0.lock().unwrap().store = Some(store);
    app.dispatch("split", json!({"segment":sid,"position":2}))
        .unwrap();
    drop(app);
    let store = Store::open(root.clone(), false).unwrap();
    assert_eq!(store.project.archived_segments.len(), 1);
    assert_eq!(store.project.segments.len(), 2);
    assert_ne!(
        store.project.segments[0].takes[0].id,
        store.project.segments[1].takes[0].id
    );
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn take_score_persists_validates_and_preserves_memo() {
    let old: TakeNote = serde_json::from_value(json!({"memo":"旧メモ","favorite":false})).unwrap();
    assert_eq!(old.score, None);
    let root = std::env::temp_dir().join(id());
    let mut store = Store::open(root.clone(), true).unwrap();
    let mut s = scored_segment(&root, "市場へ行く");
    let t = take_from_file(
        &root,
        &root.join(&s.comparisons[0].variants[0].file),
        "fixture".into(),
    )
    .unwrap();
    let tid = t.id.clone();
    let sid = s.id.clone();
    s.takes.push(t);
    s.take_notes.insert(tid.clone(), old);
    store.project.segments.push(s);
    store.save().unwrap();
    let app = App::default();
    app.0.lock().unwrap().store = Some(store);
    for score in (0..=100).step_by(10) {
        app.dispatch(
            "take_score",
            json!({"segment":sid,"take":tid,"score":score}),
        )
        .unwrap();
    }
    for score in [json!(-10), json!(101), json!(15), json!(0.5), json!("80")] {
        assert!(
            app.dispatch(
                "take_score",
                json!({"segment":sid,"take":tid,"score":score})
            )
            .is_err()
        );
    }
    app.dispatch(
        "take_note",
        json!({"segment":sid,"take":tid,"memo":"変更メモ","favorite":false}),
    )
    .unwrap();
    drop(app);
    let store = Store::open(root.clone(), false).unwrap();
    let note = &store.project.segments[0].take_notes[&tid];
    assert_eq!(note.score, Some(100));
    assert_eq!(note.memo, "変更メモ");
    let app = App::default();
    app.0.lock().unwrap().store = Some(store);
    app.dispatch("take_score", json!({"segment":sid,"take":tid,"score":null}))
        .unwrap();
    drop(app);
    let store = Store::open(root.clone(), false).unwrap();
    assert_eq!(store.project.segments[0].take_notes[&tid].score, None);
    drop(store);
    std::fs::remove_dir_all(root).unwrap();
}
