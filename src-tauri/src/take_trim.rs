//! テイク編集の保存時に、指定範囲だけを新しいWAVへ確定する。
use crate::domain::*;
use std::{
    fs::OpenOptions,
    io::BufWriter,
    path::{Path, PathBuf},
};

fn source_take<'a>(
    project: &'a Project,
    segment_id: &str,
    take_id: &str,
    start: f64,
    end: f64,
) -> Result<&'a Take> {
    if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start {
        return Err("開始と終了は、空でない正しい順番の秒数にしてください".into());
    }
    let take = project
        .segments
        .iter()
        .find(|segment| segment.id == segment_id)
        .ok_or("発話が見つかりません")?
        .takes
        .iter()
        .find(|take| take.id == take_id && take.status == "ready")
        .ok_or("保存できるテイクがありません")?;
    if end > take.duration {
        return Err("終了位置は音声の長さを超えています".into());
    }
    Ok(take)
}

/// 編集画面が持つ範囲。`cut` は残す範囲の内側から取り除く一組だけを表す。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditRange {
    pub start: f64,
    pub end: f64,
    pub cut: Option<(f64, f64)>,
}

pub fn edit_range(
    start: f64,
    end: f64,
    cut_start: Option<f64>,
    cut_end: Option<f64>,
) -> Result<EditRange> {
    if cut_start.is_some() != cut_end.is_some() {
        return Err("途中カットの開始と終了は両方指定してください".into());
    }
    let cut = cut_start.zip(cut_end);
    if let Some((cut_start, cut_end)) = cut {
        if !cut_start.is_finite()
            || !cut_end.is_finite()
            || cut_start <= start
            || cut_end >= end
            || cut_end <= cut_start
        {
            return Err("途中カットは残す範囲の内側に、空でない範囲で指定してください".into());
        }
    }
    Ok(EditRange { start, end, cut })
}

fn kept_sample_ranges(edit: EditRange) -> Vec<(f64, f64)> {
    match edit.cut {
        Some((cut_start, cut_end)) => vec![(edit.start, cut_start), (cut_end, edit.end)],
        None => vec![(edit.start, edit.end)],
    }
}

pub fn validate(
    project: &Project,
    segment_id: &str,
    take_id: &str,
    start: f64,
    end: f64,
) -> Result<()> {
    source_take(project, segment_id, take_id, start, end)?;
    Ok(())
}

pub fn validate_edit(
    project: &Project,
    segment_id: &str,
    take_id: &str,
    edit: EditRange,
) -> Result<()> {
    source_take(project, segment_id, take_id, edit.start, edit.end)?;
    // `edit_range` is deliberately separate so app.rs can turn an RPC payload
    // into this value before it reaches the file-writing path.
    if let Some((cut_start, cut_end)) = edit.cut {
        edit_range(edit.start, edit.end, Some(cut_start), Some(cut_end))?;
    }
    Ok(())
}

pub fn save(
    root: &Path,
    project: &mut Project,
    segment_id: &str,
    take_id: &str,
    start: f64,
    end: f64,
) -> Result<()> {
    save_edit(
        root,
        project,
        segment_id,
        take_id,
        EditRange {
            start,
            end,
            cut: None,
        },
    )
}

pub fn save_edit(
    root: &Path,
    project: &mut Project,
    segment_id: &str,
    take_id: &str,
    edit: EditRange,
) -> Result<()> {
    validate_edit(project, segment_id, take_id, edit)?;
    let source = source_take(project, segment_id, take_id, edit.start, edit.end)?.clone();
    let input = crate::storage::safe(root, &source.file)?;
    if crate::storage::hash_file(&input)? != source.hash {
        return Err("音声が変わっています".into());
    }
    let (path, file) = write_edited_wav(root, &source.id, &input, edit)?;
    let hash = crate::storage::hash_file(&path)?;
    let (duration, peak) = crate::audio::inspect(&path)?;
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
    take.file = file;
    take.hash = hash;
    take.duration = duration;
    take.peak = peak;
    take.trim_start = 0.0;
    take.trim_end = 0.0;
    take.warning = if source.warning.starts_with("要確認：") {
        source.warning.clone()
    } else if peak >= 0.999 {
        "クリッピングの可能性があります".into()
    } else if peak < 0.01 {
        "音量が小さいため試聴してください".into()
    } else {
        String::new()
    };
    if segment.selected.as_deref() == Some(take_id) {
        segment.needs_review = true;
        for comparison in &mut segment.comparisons {
            comparison.stale = true;
        }
    }
    Ok(())
}

/// 未公開の一意な一時ファイルへ完成させてから、テイク専用の新WAVとして確定する。
/// 旧WAVはrevisionsから参照され得るため残す。
fn write_edited_wav(
    root: &Path,
    take_id: &str,
    source: &Path,
    edit: EditRange,
) -> Result<(PathBuf, String)> {
    let samples = crate::audio::samples(source)?;
    let mut clip = Vec::new();
    for (start, end) in kept_sample_ranges(edit) {
        let from = (start * 48_000.0).round() as usize * 2;
        let until = (end * 48_000.0).round() as usize * 2;
        clip.extend_from_slice(
            samples
                .get(from..until)
                .ok_or("終了位置は音声の長さを超えています")?,
        );
    }
    if clip.is_empty() {
        return Err("保存する音声がありません".into());
    }
    let relative = format!("takes/{take_id}/trim-{}.wav", id());
    let path = crate::storage::safe(root, &relative)?;
    let pending = path.with_extension(format!("{}.pending", id()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let result = (|| -> Result<()> {
        let pending_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)
            .map_err(|e| e.to_string())?;
        let mut writer = hound::WavWriter::new(BufWriter::new(pending_file), crate::audio::spec())
            .map_err(|e| e.to_string())?;
        for sample in clip {
            writer
                .write_sample((sample.clamp(-1.0, 1.0) * 32767.0).round() as i16)
                .map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
        std::fs::rename(&pending, &path).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    result?;
    Ok((path, relative))
}

/// 編集途中の範囲だけを一時WAVへ書き出す。元ファイルは開いて読むだけで変更しない。
pub fn preview_wav(root: &Path, source: &Path, start: f64, end: f64) -> Result<(PathBuf, String)> {
    preview_edit_wav(
        root,
        source,
        EditRange {
            start,
            end,
            cut: None,
        },
    )
}

/// 編集後に残る範囲だけを一時WAVへ書き出す。途中カットは二つの残存区間を連結する。
pub fn preview_edit_wav(root: &Path, source: &Path, edit: EditRange) -> Result<(PathBuf, String)> {
    let samples = crate::audio::samples(source)?;
    let mut clip = Vec::new();
    for (start, end) in kept_sample_ranges(edit) {
        let from = (start * 48_000.0).round() as usize * 2;
        let until = (end * 48_000.0).round() as usize * 2;
        clip.extend_from_slice(
            samples
                .get(from..until)
                .ok_or("試聴範囲が音声の長さを超えています")?,
        );
    }
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
    for sample in clip {
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
    fn validation_rejects_empty_or_reversed_ranges() {
        let mut p = Project::new("test".into());
        let mut s = Segment::new("line".into());
        let t = take();
        let sid = s.id.clone();
        let tid = t.id.clone();
        s.takes.push(t);
        s.selected = Some(tid.clone());
        p.segments.push(s);
        assert_eq!(p.segments[0].takes[0].trimmed_range().unwrap(), (0.0, 2.0));
        assert!(validate(&p, &sid, &tid, 1.0, 1.0).is_err());
        assert!(validate(&p, &sid, &tid, 1.5, 1.0).is_err());
        assert!(validate(&p, &sid, &tid, 0.25, 1.75).is_ok());
    }
    #[test]
    fn edit_range_accepts_one_internal_cut_only() {
        assert_eq!(
            edit_range(0.0, 2.0, Some(0.5), Some(1.5)).unwrap(),
            EditRange { start: 0.0, end: 2.0, cut: Some((0.5, 1.5)) }
        );
        assert!(edit_range(0.0, 2.0, Some(0.5), None).is_err());
        assert!(edit_range(0.0, 2.0, Some(0.0), Some(1.0)).is_err());
        assert!(edit_range(0.0, 2.0, Some(1.0), Some(2.0)).is_err());
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
    fn save_creates_trimmed_wav_resets_range_and_keeps_old_file() {
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
        let original_file = take.file.clone();
        let mut segment = Segment::new("line".into());
        let sid = segment.id.clone();
        let tid = take.id.clone();
        segment.selected = Some(tid.clone());
        segment.takes.push(take);
        store.project.segments.push(segment);
        save(&root, &mut store.project, &sid, &tid, 0.02, 0.08).unwrap();
        store.save().unwrap();
        drop(store);
        let reopened = crate::storage::Store::open(root.clone(), false).unwrap();
        let trimmed = &reopened.project.segments[0].takes[0];
        let (trim_start, trim_end) = trimmed.trimmed_range().unwrap();
        assert_eq!(trim_start, 0.0);
        assert!((trim_end - 0.06).abs() < f64::EPSILON);
        assert_eq!(
            crate::audio::samples(&crate::storage::safe(&root, &trimmed.file).unwrap())
                .unwrap()
                .len(),
            5_760
        );
        assert_ne!(trimmed.file, original_file);
        assert!(original.exists());
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
    #[test]
    fn save_edit_joins_the_two_remaining_ranges_and_resets_the_edit() {
        let root = std::env::temp_dir().join(format!("scriptsync-cut-save-{}", id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.wav");
        let mut writer = hound::WavWriter::create(&source, crate::audio::spec()).unwrap();
        for frame in 0..480 {
            let sample = if frame < 160 { 1000i16 } else if frame < 320 { 2000i16 } else { 3000i16 };
            for _ in 0..2 { writer.write_sample(sample).unwrap(); }
        }
        writer.finalize().unwrap();
        let mut store = crate::storage::Store::open(root.clone(), true).unwrap();
        let take = crate::app::take_from_file(&root, &source, "test".into()).unwrap();
        let original = crate::storage::safe(&root, &take.file).unwrap();
        let original_file = take.file.clone();
        let mut segment = Segment::new("line".into());
        let sid = segment.id.clone();
        let tid = take.id.clone();
        segment.takes.push(take);
        store.project.segments.push(segment);
        save_edit(
            &root,
            &mut store.project,
            &sid,
            &tid,
            edit_range(0.0, 0.01, Some(0.003_333_333), Some(0.006_666_667)).unwrap(),
        ).unwrap();
        let edited = &store.project.segments[0].takes[0];
        let samples = crate::audio::samples(&crate::storage::safe(&root, &edited.file).unwrap()).unwrap();
        assert_eq!(samples.len(), 640);
        assert!(samples[..320].iter().all(|sample| (*sample - 1000.0 / 32768.0).abs() < 0.0001));
        assert!(samples[320..].iter().all(|sample| (*sample - 3000.0 / 32768.0).abs() < 0.0001));
        assert_eq!(edited.trimmed_range().unwrap(), (0.0, edited.duration));
        assert_ne!(edited.file, original_file);
        assert!(original.exists());
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
}
