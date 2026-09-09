//! 実原稿をローカルだけで派生させ、保存・音声参照・マージ結果を実測する。
use crate::{domain::*, markdown, script_merge as merge, storage::*};
use serde_json::json;
use std::{fs, path::PathBuf};

fn request(text: &str) -> merge::Request {
    merge::Request {
        text: text.into(),
        range: None,
        choices: vec![],
    }
}
fn original(text: &str) -> Project {
    let mut p = Project::new("マージ検証".into());
    markdown::commit(
        &mut p,
        &json!({"text":text,"lines":markdown::parse(text).unwrap()}),
    )
    .unwrap();
    p
}

#[test]
fn merge_rejects_ambiguous_duplicate_conflicting_and_stale_choices() {
    let p = original("同文\n同文\n固有文");
    let mut r = request("同文\n固有文\n同文");
    let before = serde_json::to_value(&p).unwrap();
    let preview = merge::preview(&p, &r).unwrap();
    assert_eq!(preview.unresolved, 2);
    assert!(merge::apply(&p, &r, &preview.fingerprint, true).is_err());
    for (line, old) in [(0, 1), (2, 0)] {
        r.choices.push(merge::Choice {
            new_line: line,
            old_id: Some(p.segments[old].id.clone()),
            decision: merge::Decision::Keep,
        });
    }
    // 交差する手動対応をJSON再読込後も維持。
    let restored: merge::Request =
        serde_json::from_value(serde_json::to_value(&r).unwrap()).unwrap();
    let review = merge::preview(&p, &restored).unwrap();
    assert_eq!(review.unresolved, 0);
    let next = merge::apply(&p, &restored, &review.fingerprint, false).unwrap();
    assert_eq!(next.segments[0].id, p.segments[1].id);
    assert_eq!(next.segments[2].id, p.segments[0].id);
    r.choices[1].old_id = r.choices[0].old_id.clone();
    assert!(merge::preview(&p, &r).is_err());
    r.text = "固有文".into();
    assert!(merge::preview(&p, &r).is_err());
    let mut changed = p.clone();
    changed.segments[0].skip = true;
    assert!(merge::apply(&changed, &restored, &review.fingerprint, false).is_err());
    assert_eq!(serde_json::to_value(&p).unwrap(), before);
}

#[test]
fn partial_update_preserves_outside_ids_and_markdown_and_requires_discard_confirmation() {
    let p = original("# 前章\n本文A\n本文B\n# 後章\n本文C");
    let mut r = request("本文B\n本文A");
    r.range = Some((1, 3));
    let preview = merge::preview(&p, &r).unwrap();
    let next = merge::apply(&p, &r, &preview.fingerprint, false).unwrap();
    assert_eq!(next.segments[0].id, p.segments[0].id);
    assert_eq!(next.segments[1].id, p.segments[2].id);
    assert_eq!(next.segments[3].id, p.segments[3].id);
    assert!(next.source_markdown.starts_with("# 前章\n"));
    assert!(next.source_markdown.contains("# 後章"));
    r.text = "本文A".into();
    assert!(merge::apply(&p, &r, &preview.fingerprint, false).is_err());
    let next = merge::apply(&p, &r, &preview.fingerprint, true).unwrap();
    assert_eq!(next.archived_segments[0].id, p.segments[2].id);
}

#[test]
#[ignore = "ユーザー許可済み原稿。SCRIPT_SYNC_MERGE_SAMPLEを指定して明示実行"]
fn presentation_corpus_experiment() {
    let source =
        PathBuf::from(std::env::var_os("SCRIPT_SYNC_MERGE_SAMPLE").expect("原稿フォルダーが必要"));
    let mut paths: Vec<_> = fs::read_dir(&source)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty());
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("output/merge-experiments")
        .join(id());
    fs::create_dir_all(&out).unwrap();
    let mut input = Vec::new();
    let mut chapters = Vec::new();
    for path in &paths {
        let text = fs::read_to_string(path).unwrap().replace("\r\n", "\n");
        input.push(json!({"name":path.file_name().unwrap().to_string_lossy(),"sha256":hash_file(path).unwrap()}));
        chapters.push(text);
    }
    let combined = chapters.join("\n\n");
    fs::write(out.join("00-all-files.md"), &combined).unwrap();
    // 改名・再分割して同じ内容になる入力セットを実際に生成する。
    let split_at = combined
        .char_indices()
        .nth(combined.chars().count() / 2)
        .unwrap()
        .0;
    let renamed = out.join("renamed-parts");
    fs::create_dir_all(&renamed).unwrap();
    fs::write(renamed.join("別名-A.md"), &combined[..split_at]).unwrap();
    fs::write(renamed.join("別名-B.md"), &combined[split_at..]).unwrap();
    assert_eq!(
        fs::read_to_string(renamed.join("別名-A.md")).unwrap()
            + &fs::read_to_string(renamed.join("別名-B.md")).unwrap(),
        combined
    );
    // この原稿だけの明示的な朗読用派生。全ファイル版も必ず残す。
    let reading = chapters
        .iter()
        .filter_map(|c| c.split_once("## 読み上げ原稿").map(|(_, body)| body))
        .flat_map(str::lines)
        .filter(|l| !l.trim().starts_with('【'))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(out.join("01-reading-only.md"), &reading).unwrap();
    let mut p = original(&combined);
    assert!(p.segments.len() > 100);
    let baseline_ids: Vec<_> = p.segments.iter().map(|s| s.id.clone()).collect();
    // 各行に異なる信号を作り、実ファイルのhashと採用参照を検証する。
    let mut store =
        crate::storage::Store::create_project_file(out.join("baseline.scriptsync")).unwrap();
    for (i, s) in p.segments.iter_mut().enumerate() {
        let tid = id();
        let file = format!("takes/{tid}/audio.wav");
        let path = safe(&store.root, &file).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut wav = hound::WavWriter::create(&path, spec).unwrap();
        for n in 0..2400 {
            let v = ((n as f32 * (220.0 + i as f32) * std::f32::consts::TAU / 48000.0).sin()
                * 2000.0) as i16;
            wav.write_sample(v).unwrap();
            wav.write_sample(v).unwrap();
        }
        wav.finalize().unwrap();
        s.takes.push(Take {
            id: tid.clone(),
            script_text: s.text.clone(),
            trim_start: 0.0,
            trim_end: 0.0,
            file,
            hash: hash_file(&path).unwrap(),
            status: "ready".into(),
            duration: 0.05,
            peak: 0.07,
            warning: "検証用信号。朗読ではありません".into(),
            created: now(),
            endpoint: "test-signal".into(),
        });
        s.selected = Some(tid);
        if i % 13 == 0 {
            s.skip = true;
        }
    }
    p.generation = store.project.generation;
    store.project = p.clone();
    store.save().unwrap();
    p = store.project.clone();
    let base_json = serde_json::to_value(&p).unwrap();
    let base_hashes: Vec<_> = p.segments.iter().map(|s| s.takes[0].hash.clone()).collect();

    let insertion = format!("検証用の先頭追加です。\n\n{combined}");
    let mut legacy = p.clone();
    markdown::commit(
        &mut legacy,
        &json!({"text":insertion,"lines":markdown::parse(&insertion).unwrap()}),
    )
    .unwrap();
    let legacy_retained = legacy
        .segments
        .iter()
        .filter(|s| baseline_ids.contains(&s.id))
        .count();

    // 元の行位置をfixture生成時だけ追跡し、独立した正解対応を持たせる。
    // 照合側へファイル名を渡さず、重複文は明示的なユーザー対応を模擬する。
    let lines: Vec<String> = combined.lines().map(str::to_owned).collect();
    let old_rows: Vec<usize> = p.segments.iter().map(|s| s.source_line.unwrap()).collect();
    let base: Vec<(String, Option<usize>, merge::Decision)> = lines
        .iter()
        .enumerate()
        .map(|(line, raw)| {
            (
                raw.clone(),
                old_rows.iter().position(|n| *n == line),
                merge::Decision::Keep,
            )
        })
        .collect();
    let mut cases = Vec::new();
    cases.push(("unchanged", base.clone()));
    let mut inserted = base.clone();
    inserted.insert(
        0,
        ("検証用の先頭追加です。".into(), None, merge::Decision::Add),
    );
    inserted.insert(
        inserted.len() / 2,
        (
            "途中に補足を追加しました。".into(),
            None,
            merge::Decision::Add,
        ),
    );
    inserted.push(("末尾の追加です。".into(), None, merge::Decision::Add));
    cases.push(("insertions", inserted));
    let mut moved = base.clone();
    moved.reverse();
    cases.push(("reversed", moved));
    let mut edited = base.clone();
    let i = *old_rows
        .iter()
        .find(|i| lines[**i].chars().count() > 50)
        .unwrap();
    edited[i].0.push_str("（追記）");
    cases.push(("minor-keep", edited.clone()));
    edited[i].0 = "この部分の説明を新しい内容に全面的に差し替えました。".into();
    edited[i].2 = merge::Decision::Replace;
    cases.push(("major-replace", edited));
    let deleted: Vec<_> = base
        .iter()
        .filter(|(_, old, _)| !old.is_some_and(|i| i % 7 == 0))
        .cloned()
        .collect();
    cases.push(("deletions", deleted));
    let mut results = Vec::new();
    for (name, fixture) in cases {
        let text = fixture
            .iter()
            .map(|r| r.0.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(out.join(format!("case-{name}.md")), &text).unwrap();
        let mut r = request(&text);
        let initial = merge::preview(&p, &r).unwrap();
        for row in &initial.rows {
            let (_, old, choice) = &fixture[row.line.line];
            if row.decision.is_none() || *choice != merge::Decision::Keep {
                r.choices.push(merge::Choice {
                    new_line: row.line.line,
                    old_id: old.map(|i| p.segments[i].id.clone()),
                    decision: choice.clone(),
                });
            }
        }
        let preview = merge::preview(&p, &r).unwrap();
        assert_eq!(preview.unresolved, 0, "{name}");
        let merged = merge::apply(&p, &r, &preview.fingerprint, true).unwrap();
        let mut expected_archive = 0;
        for (row, segment) in preview.rows.iter().zip(&merged.segments) {
            let (_, old, choice) = &fixture[row.line.line];
            if let Some(i) = old.filter(|_| *choice == merge::Decision::Keep) {
                assert_eq!(segment.id, p.segments[i].id, "{name}");
                assert_eq!(segment.selected, p.segments[i].selected);
                assert_eq!(segment.skip, p.segments[i].skip);
                assert_eq!(segment.text, p.segments[i].text);
                assert_eq!(segment.takes[0].hash, base_hashes[i]);
            } else {
                assert!(segment.takes.is_empty());
                assert!(segment.selected.is_none());
            }
        }
        for old in &p.segments {
            if !merged.segments.iter().any(|s| s.id == old.id) {
                expected_archive += 1;
            }
        }
        assert_eq!(merged.archived_segments.len(), expected_archive);
        // 実際の保存経路を通し再読込する。音声は同じデータフォルダーに保持。
        let mut case_store =
            Store::create_project_file(out.join(format!("case-{name}.scriptsync"))).unwrap();
        for s in &p.segments {
            for t in &s.takes {
                let destination = safe(&case_store.root, &t.file).unwrap();
                fs::create_dir_all(destination.parent().unwrap()).unwrap();
                fs::copy(safe(&store.root, &t.file).unwrap(), destination).unwrap();
            }
        }
        let current_generation = case_store.project.generation;
        case_store.project = merged.clone();
        case_store.project.generation = current_generation;
        case_store.save().unwrap();
        let project_file = case_store.project_file.clone().unwrap();
        drop(case_store);
        let case_store = Store::open_project_file(project_file).unwrap();
        assert_eq!(case_store.project.segments.len(), merged.segments.len());
        for s in case_store
            .project
            .segments
            .iter()
            .chain(&case_store.project.archived_segments)
        {
            for t in &s.takes {
                assert_eq!(
                    hash_file(&safe(&case_store.root, &t.file).unwrap()).unwrap(),
                    t.hash
                );
            }
        }
        let export = if merged
            .segments
            .iter()
            .filter(|s| !s.skip)
            .all(|s| s.selected.is_some())
        {
            let export = crate::audio::assemble(&case_store.root, &case_store.project).unwrap();
            let entries = export["entries"].as_array().unwrap();
            let expected: Vec<_> = merged.segments.iter().filter(|s| !s.skip).collect();
            assert_eq!(entries.len(), expected.len());
            for (entry, s) in entries.iter().zip(expected) {
                assert_eq!(entry["segment"], s.id);
                assert_eq!(entry["take"], s.selected.as_ref().unwrap().as_str());
            }
            let samples = crate::audio::samples(
                &safe(&case_store.root, export["file"].as_str().unwrap()).unwrap(),
            )
            .unwrap();
            assert_eq!(samples.len() as u64, export["frames"].as_u64().unwrap() * 2);
            json_new(&out.join(format!("case-{name}-export.json")), &export).unwrap();
            Some(json!({"frames":export["frames"],"hash":export["hash"],"entries":entries.len()}))
        } else {
            None
        };
        results.push(json!({"case":name,"rows":merged.segments.len(),"archived":expected_archive,
            "manual_choices":r.choices.len(),"initial_unresolved":initial.unresolved,"export":export,"passed":true}));
        json_new(&out.join(format!("case-{name}-request.json")), &r).unwrap();
    }
    assert_eq!(serde_json::to_value(&p).unwrap(), base_json);
    for (path, metadata) in paths.iter().zip(&input) {
        assert_eq!(hash_file(path).unwrap(), metadata["sha256"]);
    }
    json_new(&out.join("report.json"), &json!({"inputs":input,"input_files":paths.len(),"baseline_rows":p.segments.len(),
        "legacy_after_prefix_insert_retained":legacy_retained,"cases":results,"audio":"識別用信号。朗読ではない",
        "scope":"Rustマージ・保存・WAV結合の実験。UI・完全消去・ネイティブ録音は未検証"})).unwrap();
    let mut summary = format!(
        "# 登壇原稿マージ実験\n\n入力{}ファイル、全結合の対象{}行。旧処理の先頭追加後のID維持は{}行。\n\n音声は識別用信号で、朗読音声ではありません。基準と各ケースの `.scriptsync` は独立しています。\n\n| ケース | 確定行 | 廃棄行 | 手動指定 | 結果 |\n|---|---:|---:|---:|---|\n",
        paths.len(),
        p.segments.len(),
        legacy_retained
    );
    for case in &results {
        summary.push_str(&format!(
            "| {} | {} | {} | {} | 成功 |\n",
            case["case"].as_str().unwrap(),
            case["rows"],
            case["archived"],
            case["manual_choices"]
        ));
    }
    summary.push_str("\n候補の手動解決は試験コードが指定しています。新マージ処理は試験用で、アプリ画面には未接続です。部分範囲・重複起点・保存競合は通常テストで別途確認します。UI・完全消去・ネイティブ録音は未検証です。\n");
    fs::write(out.join("README.md"), summary).unwrap();
    println!("実験結果: {}", out.display());
}
