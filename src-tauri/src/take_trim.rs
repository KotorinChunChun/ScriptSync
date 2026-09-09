//! テイクごとの非破壊トリミング。範囲だけをプロジェクトへ保存する。
use crate::domain::*;
use std::path::{Path, PathBuf};

pub fn save(
    project: &mut Project,
    segment_id: &str,
    take_id: &str,
    start: f64,
    end: f64,
) -> Result<()> {
    if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start {
        return Err("開始と終了は、空でない正しい順番の秒数にしてください".into());
    }
    let segment = project
        .segments
        .iter_mut()
        .find(|segment| segment.id == segment_id)
        .ok_or("発話が見つかりません")?;
    let take = segment
        .takes
        .iter_mut()
        .find(|take| take.id == take_id && take.status == "ready")
        .ok_or("保存できるテイクがありません")?;
    if end > take.duration {
        return Err("終了位置は音声の長さを超えています".into());
    }
    let changed = take.trimmed_range()? != (start, end);
    take.trim_start = start;
    take.trim_end = end;
    take.trimmed_range()?;
    if changed && segment.selected.as_deref() == Some(take_id) {
        segment.needs_review = true;
        for comparison in &mut segment.comparisons {
            comparison.stale = true;
        }
    }
    Ok(())
}

/// 編集途中の範囲だけを一時WAVへ書き出す。元ファイルは開いて読むだけで変更しない。
pub fn preview_wav(root: &Path, source: &Path, start: f64, end: f64) -> Result<(PathBuf, String)> {
    let samples = crate::audio::samples(source)?;
    let from = (start * 48_000.0).round() as usize * 2;
    let until = (end * 48_000.0).round() as usize * 2;
    let clip = samples
        .get(from..until)
        .ok_or("試聴範囲が音声の長さを超えています")?;
    if clip.is_empty() {
        return Err("試聴する音声がありません".into());
    }
    let preview_id = id();
    let relative = format!("exports/preview-take-{preview_id}.wav");
    let path = crate::storage::safe(root, &relative)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut writer =
        hound::WavWriter::create(&path, crate::audio::spec()).map_err(|e| e.to_string())?;
    for &sample in clip {
        writer
            .write_sample((sample.clamp(-1.0, 1.0) * 32767.0).round() as i16)
            .map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    Ok((path, relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take() -> Take {
        Take {
            script_text: String::new(),
            trim_start: 0.0,
            trim_end: 0.0,
            id: id(),
            file: "a.wav".into(),
            hash: String::new(),
            status: "ready".into(),
            duration: 2.0,
            peak: 0.0,
            warning: String::new(),
            created: 0,
            endpoint: String::new(),
        }
    }
    #[test]
    fn legacy_range_is_the_whole_take_and_save_rejects_empty_or_reversed_ranges() {
        let mut p = Project::new("test".into());
        let mut s = Segment::new("line".into());
        let t = take();
        let sid = s.id.clone();
        let tid = t.id.clone();
        s.takes.push(t);
        s.selected = Some(tid.clone());
        p.segments.push(s);
        assert_eq!(p.segments[0].takes[0].trimmed_range().unwrap(), (0.0, 2.0));
        assert!(save(&mut p, &sid, &tid, 1.0, 1.0).is_err());
        assert!(save(&mut p, &sid, &tid, 1.5, 1.0).is_err());
        assert!(save(&mut p, &sid, &tid, 0.25, 1.75).is_ok());
        assert_eq!(
            p.segments[0].takes[0].trimmed_range().unwrap(),
            (0.25, 1.75)
        );
        assert!(p.segments[0].needs_review);
    }
    #[test]
    fn preview_wav_contains_only_the_requested_range() {
        let root = std::env::temp_dir().join(format!("scriptsync-trim-{}", id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.wav");
        let mut writer = hound::WavWriter::create(&source, crate::audio::spec()).unwrap();
        for frame in 0..480 {
            for _ in 0..2 {
                writer
                    .write_sample(if frame < 240 { 1000i16 } else { 2000i16 })
                    .unwrap();
            }
        }
        writer.finalize().unwrap();
        let (path, _) = preview_wav(&root, &source, 0.005, 0.010).unwrap();
        let result = crate::audio::samples(&path).unwrap();
        assert_eq!(result.len(), 480);
        assert!(
            result
                .iter()
                .all(|sample| (*sample - 2000.0 / 32768.0).abs() < 0.0001)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn persisted_trim_limits_export_without_changing_original_wav() {
        let root = std::env::temp_dir().join(format!("scriptsync-trim-export-{}", id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.wav");
        let mut writer = hound::WavWriter::create(&source, crate::audio::spec()).unwrap();
        for _ in 0..4_800 {
            for _ in 0..2 {
                writer.write_sample(1_000i16).unwrap();
            }
        }
        writer.finalize().unwrap();
        let mut store = crate::storage::Store::open(root.clone(), true).unwrap();
        let take = crate::app::take_from_file(&root, &source, "test".into()).unwrap();
        let original = crate::storage::safe(&root, &take.file).unwrap();
        let original_hash = crate::storage::hash_file(&original).unwrap();
        let mut segment = Segment::new("line".into());
        let sid = segment.id.clone();
        let tid = take.id.clone();
        segment.selected = Some(tid.clone());
        segment.takes.push(take);
        store.project.segments.push(segment);
        save(&mut store.project, &sid, &tid, 0.02, 0.08).unwrap();
        store.save().unwrap();
        drop(store);
        let reopened = crate::storage::Store::open(root.clone(), false).unwrap();
        assert_eq!(
            reopened.project.segments[0].takes[0]
                .trimmed_range()
                .unwrap(),
            (0.02, 0.08)
        );
        let output = crate::export_blocks::export_blocks(
            &root,
            &reopened.project,
            &serde_json::json!({"format":"wav","normalize":false,"global_gaps":{}}),
        )
        .unwrap();
        let file =
            crate::storage::safe(&root, output["files"][0]["file"].as_str().unwrap()).unwrap();
        assert_eq!(crate::audio::samples(&file).unwrap().len(), 5_760);
        assert_eq!(crate::storage::hash_file(&original).unwrap(), original_hash);
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }
}
