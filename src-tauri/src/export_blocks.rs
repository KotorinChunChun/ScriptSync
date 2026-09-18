//! 台本を発話ブロック単位で非破壊に出力するバックエンド。
//!
//! 境界は開始発話ID、階層は部・章・節・項。原音を変更せず別フォルダーへ出力する。
use crate::{audio, domain::*, storage::*};

use hound::{SampleFormat, WavSpec, WavWriter};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const RATE: u32 = 48_000;
const CHANNELS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Gap {
    normal: f64,
    part: f64,
    chapter: f64,
    section: f64,
    head: f64,
    tail: f64,
}

#[derive(Default)]
struct BlockTiming {
    active: Vec<usize>,
    audio_seconds: f64,
    normal_seconds: f64,
    part_seconds: f64,
    chapter_seconds: f64,
    section_seconds: f64,
    head_seconds: f64,
    tail_seconds: f64,
    unselected_count: usize,
}

impl BlockTiming {
    fn add_gap(&mut self, key: &str, seconds: f64) {
        match key {
            "part" => self.part_seconds += seconds,
            "chapter" => self.chapter_seconds += seconds,
            "section" => self.section_seconds += seconds,
            "head" => self.head_seconds += seconds,
            "tail" => self.tail_seconds += seconds,
            _ => self.normal_seconds += seconds,
        }
    }
    fn wait_seconds(&self) -> f64 {
        self.normal_seconds + self.part_seconds + self.chapter_seconds + self.section_seconds
            + self.head_seconds + self.tail_seconds
    }
}

fn number(v: &Value, key: &str, default: f64) -> Result<f64> {
    let n = v.get(key).and_then(Value::as_f64).unwrap_or(default);
    if !n.is_finite() || !(0.0..=60.0).contains(&n) {
        return Err(format!("{key} の間隔が不正です"));
    }
    Ok(n)
}
fn gaps(v: &Value) -> Result<Gap> {
    Ok(Gap {
        normal: number(v, "normal", 0.0)?,
        part: number(v, "part", 0.0)?,
        chapter: number(v, "chapter", 0.0)?,
        section: number(v, "section", 0.0)?,
        head: number(v, "head", 0.0)?,
        tail: number(v, "tail", 0.0)?,
    })
}
fn gap_for(g: Gap, kind: &str) -> f64 {
    match kind {
        "part" => g.part,
        "chapter" => g.chapter,
        "section" => g.section,
        "head" => g.head,
        "tail" => g.tail,
        _ => g.normal,
    }
}
fn boundaries(project: &Project, v: &Value) -> Result<Vec<usize>> {
    let unit = v.get("unit").and_then(Value::as_str).unwrap_or("all");
    let ids = v
        .get("boundaries")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if unit == "custom" || !ids.is_empty() {
        let mut b = vec![0];
        for id in ids {
            if let Some(i) = project.segments.iter().position(|s| s.id == id) {
                if i > 0 {
                    b.push(i)
                }
            }
        }
        b.sort_unstable();
        b.dedup();
        return Ok(b);
    }
    if unit == "utterance" {
        return Ok((0..project.segments.len()).collect());
    }
    if unit != "all" {
        let wanted = match unit {
            "part" => 1,
            "chapter" => 2,
            "section" => 3,
            _ => 0,
        };
        let mut b = vec![0];
        for (i, s) in project.segments.iter().enumerate().skip(1) {
            if (s.kind > 0 && s.kind <= wanted)
                || (0..wanted as usize).any(|level| {
                    s.hierarchy.get(level) != project.segments[i - 1].hierarchy.get(level)
                })
            {
                b.push(i)
            }
        }
        return Ok(b);
    }
    let mut b = v
        .get("boundaries")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_u64)
                .map(|n| n as usize)
                .collect()
        })
        .unwrap_or_else(|| vec![0]);
    if !b.contains(&0) {
        b.push(0);
    }
    b.sort_unstable();
    b.dedup();
    if b.iter().any(|&n| n >= project.segments.len()) {
        return Err("ブロック境界が発話数の範囲外です".into());
    }
    Ok(b)
}
fn wav_spec() -> WavSpec {
    WavSpec {
        channels: 2,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    }
}
fn pcm(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * 32767.0).round() as i16
}
fn kind(project: &Project, index: usize) -> String {
    match project.segments.get(index).map(|s| s.kind).unwrap_or(0) {
        1 => "part",
        2 => "chapter",
        3 => "section",
        _ => "normal",
    }
    .into()
}
fn gap_key(project: &Project, current: usize, next: usize) -> String {
    // 見出し自体を読まない場合も所属階層の変化から区切りを判定する。
    let hierarchy = &project.segments[current].hierarchy;
    let next_hierarchy = &project.segments[next].hierarchy;
    (0..3)
        .find(|&level| hierarchy.get(level) != next_hierarchy.get(level))
        .map(|level| ["part", "chapter", "section"][level].to_owned())
        .unwrap_or_else(|| kind(project, next))
}
fn resolved_gap(gap: Gap, settings: &Value, key: &str) -> Result<f64> {
    let seconds = if settings[key]["mode"] == "manual" {
        settings[key]["value"]
            .as_f64()
            .ok_or("手入力の間隔が不正です")?
    } else {
        gap_for(gap, key)
    };
    if !seconds.is_finite() || !(0.0..=60.0).contains(&seconds) {
        return Err("間隔は0～60秒にしてください".into());
    }
    Ok(seconds)
}
fn effective_range(segment: &Segment, take: &Take) -> Result<(f64, f64)> {
    let (take_start, take_end) = take.trimmed_range()?;
    let kept = take_end - take_start;
    if !segment.trim_start.is_finite()
        || !segment.trim_end.is_finite()
        || segment.trim_start < 0.0
        || segment.trim_end < 0.0
        || segment.trim_start + segment.trim_end >= kept
    {
        return Err("トリム範囲が不正です".into());
    }
    Ok((take_start + segment.trim_start, take_end - segment.trim_end))
}
fn block_timing(
    project: &Project,
    start: usize,
    end: usize,
    from: usize,
    gap: Gap,
    overrides: &Value,
) -> Result<BlockTiming> {
    if start >= end || end > project.segments.len() {
        return Err("出力する原稿がありません".into());
    }
    let settings = &overrides[&project.segments[start].id]["gaps"];
    let mut timing = BlockTiming::default();
    for index in start.max(from)..end {
        let segment = &project.segments[index];
        if segment.skip {
            continue;
        }
        if segment.selected.is_none() {
            timing.unselected_count += 1;
            continue;
        }
        let take = segment.selected_take().map_err(|error| format!("{}: {error}", segment.text))?;
        let (from, to) = effective_range(segment, take)?;
        timing.audio_seconds += to - from;
        timing.active.push(index);
    }
    if timing.active.is_empty() {
        return Ok(timing);
    }
    timing.add_gap("head", resolved_gap(gap, settings, "head")?);
    let active = timing.active.clone();
    for pair in active.windows(2) {
        let key = gap_key(project, pair[0], pair[1]);
        timing.add_gap(&key, resolved_gap(gap, settings, &key)?);
    }
    timing.add_gap("tail", resolved_gap(gap, settings, "tail")?);
    Ok(timing)
}
/// 出力と同じ境界・間隔・トリム規則で、採用済み音声の見込み時間を返す。
/// 未採用の発話は書き出し対象にはできないため、音声・待ち時間には含めず件数で返す。
pub fn duration_summary(project: &Project, input: &Value) -> Result<Value> {
    let settings = if input.as_object().map(|value| value.is_empty()).unwrap_or(true) {
        if project.export_settings.is_object() { project.export_settings.clone() } else { crate::export_settings::defaults() }
    } else {
        input.clone()
    };
    let starts = boundaries(project, &settings)?;
    let gap = gaps(settings.get("global_gaps").or_else(|| settings.get("gaps")).unwrap_or(&Value::Null))?;
    let overrides = settings.get("blocks").unwrap_or(&Value::Null);
    let mut total = BlockTiming::default();
    for (block, &start) in starts.iter().enumerate() {
        let end = starts.get(block + 1).copied().unwrap_or(project.segments.len());
        let timing = block_timing(project, start, end, start, gap, overrides)?;
        total.audio_seconds += timing.audio_seconds;
        total.normal_seconds += timing.normal_seconds;
        total.part_seconds += timing.part_seconds;
        total.chapter_seconds += timing.chapter_seconds;
        total.section_seconds += timing.section_seconds;
        total.head_seconds += timing.head_seconds;
        total.tail_seconds += timing.tail_seconds;
        total.unselected_count += timing.unselected_count;
    }
    let wait_seconds = total.wait_seconds();
    Ok(json!({
        "audio_seconds": total.audio_seconds,
        "wait_seconds": {
            "normal": total.normal_seconds, "part": total.part_seconds,
            "chapter": total.chapter_seconds, "section": total.section_seconds,
            "head": total.head_seconds, "tail": total.tail_seconds
        },
        "wait_total_seconds": wait_seconds,
        "total_seconds": total.audio_seconds + wait_seconds,
        "unselected_count": total.unselected_count
    }))
}
fn stem(v: &Value, project: &Project, start: usize, block: usize) -> String {
    let raw = v
        .get("stems")
        .and_then(Value::as_array)
        .and_then(|a| a.get(block))
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            project.segments.get(start).and_then(|s| {
                v.get("blocks")
                    .and_then(|b| b.get(&s.id))
                    .and_then(|x| x.get("name"))
                    .and_then(Value::as_str)
                    .filter(|name| !name.trim().is_empty())
            })
        })
        .or_else(|| {
            project.segments.get(start).map(|s| {
                if s.source_text.trim().is_empty() {
                    s.text.as_str()
                } else {
                    s.source_text.as_str()
                }
            })
        })
        .unwrap_or("script")
        .trim()
        .to_string();
    filename_text(&raw)
}
fn prefix(v: &Value, project: &Project, start: usize, block: usize) -> String {
    let style = v["naming"].as_str().unwrap_or("jp");
    let mut result = format!("{:04}_", block + 1);
    if style == "none" {
        return result;
    }
    if let Some(s) = project.segments.get(start) {
        for (i, &n) in s.hierarchy.iter().take(4).enumerate() {
            if n == 0 {
                continue;
            }
            if style == "number" {
                result.push_str(&format!("{n:02}_"));
            } else if i == 3 {
                result.push_str(&format!("{n:02}項_"));
            } else {
                result.push_str(&format!("第{n}{}_", ["部", "章", "節"][i]));
            }
        }
    }
    result
}
fn filename_text(name: &str) -> String {
    let text = safe_name(name)
        .chars()
        .take(20)
        .collect::<String>()
        .trim_end_matches([' ', '.'])
        .to_owned();
    if text.is_empty() {
        "script".into()
    } else {
        text
    }
}
fn safe_name(name: &str) -> String {
    name.trim()
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .collect::<String>()
        .trim_end_matches([' ', '.'])
        .to_owned()
}
fn block_file(root: &Path, export_id: &str, name: &str, ext: &str) -> Result<(PathBuf, String)> {
    let rel = format!("exports/{export_id}/{}.{}", safe_name(name), ext);
    Ok((safe(root, &rel)?, rel))
}
fn normalize_gain(samples: &[f32]) -> f32 {
    let peak = samples.iter().fold(0.0f32, |p, x| p.max(x.abs()));
    if peak > 0.0 {
        (0.98 / peak).min(16.0)
    } else {
        1.0
    }
}
fn write_wav(path: &Path, samples: &[f32], normalize: bool) -> Result<(String, u64)> {
    let gain = if normalize {
        normalize_gain(samples)
    } else {
        1.0
    };
    let mut clipped = 0u64;
    let mut writer = WavWriter::create(path, wav_spec()).map_err(|e| e.to_string())?;
    for &x in samples {
        let y = x * gain;
        if y.abs() > 1.0 {
            clipped += 1;
        }
        writer.write_sample(pcm(y)).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    Ok((hash_file(path)?, clipped))
}
fn write_flac(path: &Path, samples: &[f32], normalize: bool) -> Result<(String, u64)> {
    let gain = if normalize {
        normalize_gain(samples)
    } else {
        1.0
    };
    let mut pcm = Vec::with_capacity(samples.len());
    let mut clipped = 0u64;
    for &x in samples {
        let y = x * gain;
        if y.abs() > 1.0 {
            clipped += 1
        };
        pcm.push(y.clamp(-1.0, 1.0));
    }

    audio_mixer_backend::recording::codecs::encode_audio_file(
        path,
        audio_mixer_backend::audio::graph::RecordingFormat::Flac,
        &pcm,
    )
    .map_err(|e| e.to_string())?;
    Ok((hash_file(path)?, clipped))
}
fn collect_block(
    root: &Path,
    project: &Project,
    start: usize,
    end: usize,
    gap: Gap,
    overrides: &Value,
    block: usize,
    v: &Value,
) -> Result<Vec<f32>> {
    Ok(collect_block_with_subtitles(root, project, start, end, gap, overrides, block, v)?.0)
}
fn vtt_time(frames: usize) -> String {
    let ms = (frames as u64 * 1000 + RATE as u64 / 2) / RATE as u64;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        ms / 3600000,
        ms / 60000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}
fn collect_block_with_subtitles(
    root: &Path,
    project: &Project,
    start: usize,
    end: usize,
    gap: Gap,
    overrides: &Value,
    _block: usize,
    v: &Value,
) -> Result<(Vec<f32>, String, Vec<Value>)> {
    let mut subtitles = String::from("WEBVTT\n\n");
    let mut utterances = Vec::new();
    if start >= end || end > project.segments.len() {
        return Err("出力する原稿がありません".into());
    }
    let from = v["_preview_from"].as_u64().unwrap_or(0) as usize;
    let timing = block_timing(project, start, end, from, gap, overrides)?;
    let active = timing.active;
    if timing.unselected_count > 0 {
        return Err("採用テイクが存在しません".into());
    }
    if active.is_empty() {
        return Err("出力対象の原稿がありません".into());
    }
    let settings = &overrides[&project.segments[start].id]["gaps"];
    let mut out = vec![0.0; (resolved_gap(gap, settings, "head")? * RATE as f64).round() as usize * CHANNELS];
    for (offset, &i) in active.iter().enumerate() {
        let seg = &project.segments[i];
        let take = seg
            .selected_take()
            .map_err(|e| format!("{}: {e}", seg.text))?;
        let path = safe(root, &take.file)?;
        if hash_file(&path)? != take.hash {
            return Err(format!("採用音声が差し替えられています: {}", seg.text));
        }
        let input = audio::samples(&path)?;
        let (take_from, take_to) = effective_range(seg, take)?;
        let from = (take_from * RATE as f64).round() as usize * CHANNELS;
        let end = (take_to * RATE as f64).round() as usize * CHANNELS;
        if from >= end || end > input.len() {
            return Err("トリム範囲が不正です".into());
        }
        let clip = &input[from..end];
        let gain = if v["normalize"].as_bool().unwrap_or(true) {
            normalized_gain(clip)
        } else {
            10f32.powf(seg.gain_db as f32 / 20.0)
        };
        let begin = out.len() / CHANNELS;
        out.extend(clip.iter().map(|x| x * gain));
        let finish = out.len() / CHANNELS;
        utterances.push(json!({"segment_id":seg.id,"take_id":take.id,"text":seg.text,"hierarchy":seg.hierarchy,"kind":seg.kind,"start_frame":begin,"end_frame":finish,"start_seconds":begin as f64/RATE as f64,"end_seconds":finish as f64/RATE as f64}));
        let text = seg
            .text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('\0', "\u{FFFD}")
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if vtt_time(begin) == vtt_time(finish) {
            return Err("字幕の表示区間が1ミリ秒未満です".into());
        }
        subtitles.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            offset + 1,
            vtt_time(begin),
            vtt_time(finish),
            text
        ));
        if let Some(&next) = active.get(offset + 1) {
            let key = gap_key(project, i, next);
            out.extend(std::iter::repeat_n(
                0.0,
                (resolved_gap(gap, settings, &key)? * RATE as f64).round() as usize * CHANNELS,
            ));
        }
    }
    out.extend(std::iter::repeat_n(
        0.0,
        (resolved_gap(gap, settings, "tail")? * RATE as f64).round() as usize * CHANNELS,
    ));
    Ok((out, subtitles, utterances))
}
// 有声区間のRMSを共通目標へ合わせ、ピークによるクリップも抑える。
fn normalized_gain(samples: &[f32]) -> f32 {
    let peak = samples.iter().fold(0.0f32, |a, x| a.max(x.abs()));
    let voiced: Vec<_> = samples
        .iter()
        .filter(|x| x.abs() > (peak * 0.03).max(0.001))
        .collect();
    if voiced.is_empty() {
        return 1.0;
    }
    let rms = (voiced.iter().map(|x| (**x as f64).powi(2)).sum::<f64>() / voiced.len() as f64)
        .sqrt() as f32;
    (0.1 / rms).min(0.98 / peak).clamp(0.015, 64.0)
}
/// 採用済み音声を原稿順に結合し、FLACまたはWAVとmanifestを保存する。
pub fn export_blocks(root: &Path, project: &Project, v: &Value) -> Result<Value> {
    let v = if v.as_object().map(|o| o.is_empty()).unwrap_or(true)
        && project.export_settings.is_object()
    {
        &project.export_settings
    } else {
        v
    };
    let format = v
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("flac")
        .to_ascii_lowercase();
    if format != "wav" && format != "flac" {
        return Err("出力形式はFLACまたはWAVです".into());
    }
    let subtitle_format = match v.get("subtitle_format") {
        None => "vtt",
        Some(Value::String(value)) => value.as_str(),
        _ => return Err("字幕ファイル出力形式が不正です".into()),
    };
    if subtitle_format != "vtt" && subtitle_format != "none" {
        return Err("字幕ファイル出力形式はVTTまたはなしです".into());
    }
    let b = boundaries(project, v)?;
    let gap = gaps(
        v.get("global_gaps")
            .or_else(|| v.get("gaps"))
            .unwrap_or(&Value::Null),
    )?;
    let overrides = v.get("blocks").unwrap_or(&Value::Null);
    let export_id = id();
    let folder = safe(root, &format!("exports/{export_id}"))?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    let mut blocks = Vec::new();
    let mut cumulative_frames = 0usize;
    for (i, &start) in b.iter().enumerate() {
        let end = b.get(i + 1).copied().unwrap_or(project.segments.len());
        let (data, subtitles, utterances) =
            collect_block_with_subtitles(root, project, start, end, gap, overrides, i, v)?;
        if data.is_empty() {
            return Err("出力対象の発話がありません".into());
        };
        let output_name = format!(
            "{}{}",
            prefix(v, project, start, i),
            stem(v, project, start, i)
        );
        let ext = if format == "flac" { "flac" } else { "wav" };
        let (path, rel) = block_file(root, &export_id, &output_name, ext)?;
        let normalize = false;
        let (hash, clipped) = if format == "flac" {
            write_flac(&path, &data, normalize)?
        } else {
            write_wav(&path, &data, normalize)?
        };
        let subtitle_path = path.with_extension("vtt");
        let (subtitle_file, subtitle_name, subtitle_hash) = if subtitle_format == "vtt" {
            crate::storage::write_new(&subtitle_path, subtitles.as_bytes())?;
            (
                Some(
                    std::path::Path::new(&rel)
                        .with_extension("vtt")
                        .to_string_lossy()
                        .replace('\\', "/"),
                ),
                Some(
                    subtitle_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                ),
                Some(hash_file(&subtitle_path)?),
            )
        } else {
            (None, None, None)
        };
        let frames = data.len() / CHANNELS;
        let full_text = utterances
            .iter()
            .filter_map(|u| u["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        blocks.push(json!({"block_index":i,"slide_number":i+1,"audio_file":path.file_name().unwrap().to_string_lossy(),"subtitle_file":subtitle_name,"audio_hash":hash,"duration_frames":frames,"duration_seconds":frames as f64/RATE as f64,"sequence_start_frame":cumulative_frames,"sequence_end_frame":cumulative_frames+frames,"sequence_start_seconds":cumulative_frames as f64/RATE as f64,"sequence_end_seconds":(cumulative_frames+frames) as f64/RATE as f64,"utterance_start_seconds":utterances.first().map(|u|u["start_seconds"].clone()),"utterance_end_seconds":utterances.last().map(|u|u["end_seconds"].clone()),"text":full_text,"utterances":utterances}));
        cumulative_frames += frames;
        files.push(json!({"subtitle_file":subtitle_file,"subtitle_hash":subtitle_hash,"block":i,"range":{"start":start,"end":end},"file":rel,"hash":hash,"stem":stem(v,project,start,i),"name":format!("{}.{}",safe_name(&output_name),ext),"frames":data.len()/CHANNELS,"clipped_samples":clipped}));
    }
    let block_map = json!({"schema":"scriptsync.block-map","schema_version":1,"export_id":export_id,"project_id":project.id,"generation":project.generation,"split_unit":v["unit"].as_str().unwrap_or("all"),"sample_rate":RATE,"channels":CHANNELS,"total_duration_frames":cumulative_frames,"total_duration_seconds":cumulative_frames as f64/RATE as f64,"blocks":blocks});
    json_new(&folder.join("block-map.json"), &block_map)?;
    let manifest = json!({"block_map":"block-map.json","id":export_id,"format":format,"subtitle_format":subtitle_format,"project":project.id,"generation":project.generation,"files":files});
    json_new(&folder.join("manifest.json"), &manifest)?;
    Ok(manifest)
}

pub fn explorer_folder(root: &Path, relative: &str) -> Result<std::path::PathBuf> {
    if relative != "exports" && !relative.starts_with("exports/") {
        return Err("出力フォルダーを指定してください".into());
    }
    let requested = safe(root, relative)?;
    if relative == "exports" {
        std::fs::create_dir_all(&requested).map_err(|e| e.to_string())?;
    }
    let path = requested.canonicalize().map_err(|e| e.to_string())?;
    if !path.is_dir() {
        return Err("出力フォルダーがありません".into());
    }
    // Explorer は canonicalize の拡張長パス接頭辞や混在区切りを解釈できない。
    let value = path.to_string_lossy().replace('/', "\\");
    let value = if let Some(unc) = value.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{unc}")
    } else {
        value.strip_prefix("\\\\?\\").unwrap_or(&value).to_owned()
    };
    Ok(value.into())
}

pub fn preview_blocks(root: &Path, project: &Project, v: &Value) -> Result<Value> {
    let v = if v.as_object().map(|o| o.is_empty()).unwrap_or(true)
        && project.export_settings.is_object()
    {
        &project.export_settings
    } else {
        v
    };
    let b = boundaries(project, v)?;
    let selected = v.get("selected_block").and_then(Value::as_u64).unwrap_or(0) as usize;
    if selected >= b.len() {
        return Err("試聴ブロックが不正です".into());
    };
    let scope = v["preview_scope"].as_str().unwrap_or("following");
    if !["all", "block", "following"].contains(&scope) {
        return Err("試聴範囲が不正です".into());
    }
    let selected = if scope == "all" { 0 } else { selected };
    let last_block = if scope == "block" {
        selected + 1
    } else {
        b.len()
    };
    // 採用音声と加工条件だけをキャッシュの依存先とする。メモや評価の変更では破棄しない。
    let mut dependencies = Vec::new();
    for block in selected..last_block {
        let start = b[block];
        let end = b.get(block + 1).copied().unwrap_or(project.segments.len());
        let from = if block == selected && v["from_last_item"].as_bool().unwrap_or(false) {
            (start..end)
                .rev()
                .find(|&i| !project.segments[i].skip)
                .ok_or("試聴対象がありません")?
        } else {
            start
        };
        for seg in &project.segments[from..end] {
            if seg.skip {
                continue;
            }
            let take = seg.selected_take()?;
            if hash_file(&safe(root, &take.file)?)? != take.hash {
                return Err("採用音声が差し替えられています".into());
            }
            dependencies.push(json!({"id":seg.id,"text":seg.text,"hierarchy":seg.hierarchy,"kind":seg.kind,"gain":seg.gain_db,"trim_start":seg.trim_start,"trim_end":seg.trim_end,"take":take}));
        }
    }
    let cache_key = hash_bytes(
        &serde_json::to_vec(
            &json!({"version":1,"settings":v,"boundaries":b,"dependencies":dependencies}),
        )
        .map_err(|e| e.to_string())?,
    );
    let cache_path = safe(root, &format!("exports/preview-cache-{cache_key}.json"))?;
    if let Ok(bytes) = std::fs::read(&cache_path) {
        if let Ok(mut cached) = serde_json::from_slice::<Value>(&bytes) {
            let valid = cached["files"].as_array().is_some_and(|files| {
                files.len() == 1
                    && files.iter().all(|f| {
                        f["file"]
                            .as_str()
                            .zip(f["hash"].as_str())
                            .is_some_and(|(file, hash)| {
                                safe(root, file)
                                    .and_then(|p| hash_file(&p))
                                    .is_ok_and(|actual| actual == hash)
                            })
                    })
            });
            if valid {
                cached["cached"] = json!(true);
                return Ok(cached);
            }
        }
    }
    let gap = gaps(v.get("global_gaps").unwrap_or(&Value::Null))?;
    let overrides = v.get("blocks").unwrap_or(&Value::Null);
    let id = id();
    let folder = safe(root, &format!("exports/preview-{id}"))?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mut all = Vec::new();
    let mut result = Vec::new();
    let mut timeline = Vec::new();
    let mut offset = 0.0f64;
    for (i, &start) in b.iter().enumerate() {
        if i < selected || i >= last_block {
            continue;
        }
        let end = b.get(i + 1).copied().unwrap_or(project.segments.len());
        let mut selected_settings = v.clone();
        let from = if i == selected && v["from_last_item"].as_bool().unwrap_or(false) {
            (start..end)
                .rev()
                .find(|&n| !project.segments[n].skip)
                .ok_or("試聴対象がありません")?
        } else {
            start
        };
        selected_settings["_preview_from"] = json!(from);
        let data = collect_block(
            root,
            project,
            start,
            end,
            gap,
            overrides,
            i,
            &selected_settings,
        )?;
        let duration = data.len() as f64 / (RATE as f64 * CHANNELS as f64);
        all.extend_from_slice(&data);
        timeline.push(json!({"block":i,"start_segment":from,"end_segment":end,"offset":offset,"duration":duration}));
        offset += duration;
    }
    let p = folder.join("combined.wav");
    let (h, _) = write_wav(&p, &all, false)?;
    result.push(json!({"mode":"combined","file":format!("exports/preview-{id}/combined.wav"),"hash":h,"frames":all.len()/CHANNELS}));
    let result =
        json!({"selected_block":selected,"files":result,"timeline":timeline,"cached":false});
    // キャッシュ索引の破損・書き込み失敗は再生成に戻せる。完成音声のみ登録する。
    if let Ok(bytes) = serde_json::to_vec(&result) {
        let _ = std::fs::write(cache_path, bytes);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timing_take(name: &str, duration: f64) -> Take {
        Take {
            script_text: String::new(), trim_start: 0.0, trim_end: 0.0,
            id: name.into(), file: format!("takes/{name}.wav"), hash: String::new(),
            status: "ready".into(), duration, peak: 0.0, warning: String::new(),
            created: 0, endpoint: String::new(),
        }
    }

    #[test]
    fn duration_summary_uses_export_gap_boundaries_trims_and_skips() {
        let mut project = Project::new("時間集計".into());
        for (index, hierarchy) in [[1, 1, 1], [1, 1, 1], [2, 1, 1], [2, 2, 1], [2, 2, 2]]
            .into_iter()
            .enumerate()
        {
            let mut segment = Segment::new(format!("発話{index}"));
            segment.hierarchy = hierarchy.to_vec();
            let take = timing_take(&format!("take{index}"), if index == 0 { 2.0 } else { 1.0 });
            segment.selected = Some(take.id.clone());
            segment.takes.push(take);
            if index == 0 { segment.trim_start = 0.2; segment.trim_end = 0.1; }
            project.segments.push(segment);
        }
        project.segments.push(Segment::new("未採用".into()));
        let mut skipped = Segment::new("対象外".into());
        skipped.skip = true;
        project.segments.push(skipped);
        let first = project.segments[0].id.clone();
        let summary = duration_summary(&project, &json!({
            "unit":"all",
            "global_gaps":{"normal":0.3,"part":1.2,"chapter":0.8,"section":0.4,"head":0.1,"tail":0.3},
            "blocks":{first:{"gaps":{
                "normal":{"mode":"manual","value":0.15},
                "chapter":{"mode":"manual","value":0.25},
                "head":{"mode":"manual","value":0.7},
                "tail":{"mode":"manual","value":0.8}
            }}}
        })).unwrap();
        assert_eq!(summary["unselected_count"], 1);
        assert_eq!(summary["audio_seconds"], json!(5.7));
        assert_eq!(summary["wait_seconds"], json!({
            "normal":0.15,"part":1.2,"chapter":0.25,"section":0.4,"head":0.7,"tail":0.8
        }));
        assert_eq!(summary["wait_total_seconds"], json!(3.5));
        assert_eq!(summary["total_seconds"], json!(9.2));
    }
    #[test]
    fn naming_is_fixed_four_digits_by_default() {
        let p = Project::new("x".into());
        let v = json!({"stem":"録音","naming":"number"});
        assert_eq!(
            format!("{}{}", prefix(&v, &p, 0, 0), stem(&v, &p, 0, 0)),
            "0001_script"
        );
        assert_eq!(prefix(&json!({"naming":"none"}), &p, 0, 8), "0009_");
    }

    #[test]
    fn subtitle_none_writes_only_audio_and_keeps_timeline_metadata() {
        let root = std::env::temp_dir().join(format!("scriptsync-no-subtitle-{}", id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("input.wav");
        write_wav(&path, &vec![0.1; 9600], false).unwrap();
        let take = crate::app::take_from_file(&root, &path, "test".into()).unwrap();
        let mut project = Project::new("字幕なし".into());
        let mut segment = Segment::new("字幕なしの音声".into());
        segment.selected = Some(take.id.clone());
        segment.takes.push(take);
        project.segments.push(segment);
        for format in ["wav", "flac"] {
            let options = json!({"format":format,"subtitle_format":"none"});
            let manifest = export_blocks(&root, &project, &options).unwrap();
            let output = safe(&root, manifest["files"][0]["file"].as_str().unwrap()).unwrap();
            assert!(output.is_file());
            assert!(!output.with_extension("vtt").exists());
            assert_eq!(manifest["subtitle_format"], "none");
            assert!(manifest["files"][0]["subtitle_file"].is_null());
            assert!(manifest["files"][0]["subtitle_hash"].is_null());
            let map: Value = serde_json::from_slice(
                &std::fs::read(output.parent().unwrap().join("block-map.json")).unwrap(),
            )
            .unwrap();
            assert!(map["blocks"][0]["subtitle_file"].is_null());
            assert_eq!(map["blocks"][0]["text"], "字幕なしの音声");
            assert_eq!(
                map["blocks"][0]["utterances"][0]["segment_id"],
                project.segments[0].id
            );
        }
        assert!(export_blocks(&root, &project, &json!({"subtitle_format":"srt"})).is_err());
        assert!(export_blocks(&root, &project, &json!({"subtitle_format":null})).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn flac_writer_creates_a_valid_stream_header() {
        let path = std::env::temp_dir().join(format!("scriptsync-{}.flac", id()));
        let samples = vec![0.0f32; RATE as usize / 100 * CHANNELS];
        let (hash, _) = write_flac(&path, &samples, false).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..4], b"fLaC");
        assert_eq!(hash_file(&path).unwrap(), hash);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn hierarchy_names_and_upper_heading_boundaries() {
        let mut p = Project::new("test".into());
        for kind in [1, 2, 3, 4, 1] {
            let mut s = Segment::new("日本語の見出し".into());
            s.kind = kind;
            s.hierarchy = vec![1, 2, 3, 4];
            p.segments.push(s);
        }
        assert_eq!(
            prefix(&json!({"naming":"jp"}), &p, 0, 0),
            "0001_第1部_第2章_第3節_04項_"
        );
        assert_eq!(
            prefix(&json!({"naming":"number"}), &p, 0, 1),
            "0002_01_02_03_04_"
        );
        assert_eq!(prefix(&json!({"naming":"none"}), &p, 0, 0), "0001_");
        assert_eq!(
            boundaries(&p, &json!({"unit":"chapter"})).unwrap(),
            vec![0, 1, 4]
        );
        assert_eq!(safe_name("0001_日本語:原稿/題名"), "0001_日本語原稿題名");
    }

    #[test]
    fn wav_and_flac_decode_with_block_gaps_and_uniform_levels() {
        let root = std::env::temp_dir().join(format!("scriptsync-output-{}", id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut p = Project::new("結合試験".into());
        for (i, amplitude) in [0.2, 0.8].into_iter().enumerate() {
            let path = root.join(format!("input{i}.wav"));
            write_wav(&path, &vec![amplitude; 1920], false).unwrap();
            let t = crate::app::take_from_file(&root, &path, "test".into()).unwrap();
            let mut s = Segment::new(format!("原稿{i}"));
            s.kind = (i + 1) as u8;
            s.hierarchy = vec![1, i as u32, 0, 0];
            s.selected = Some(t.id.clone());
            s.takes.push(t);
            p.segments.push(s);
        }
        let key = p.segments[0].id.clone();
        let mut settings = json!({"unit":"all","format":"wav","naming":"jp","normalize":true,"global_gaps":{"normal":0.01,"part":0.04,"chapter":0.03,"section":0.02,"head":0.01,"tail":0.02},"blocks":{key:{"gaps":{"head":{"mode":"manual","value":0.06},"tail":{"mode":"manual","value":0.07},"chapter":{"mode":"manual","value":0.05}}}}});
        let wav = export_blocks(&root, &p, &settings).unwrap();
        let wav_path = safe(&root, wav["files"][0]["file"].as_str().unwrap()).unwrap();
        let decoded = audio::samples(&wav_path).unwrap();
        let vtt_path = wav_path.with_extension("vtt");
        let vtt = std::fs::read_to_string(&vtt_path).unwrap();
        assert_eq!(
            vtt,
            "WEBVTT\n\n1\n00:00:00.060 --> 00:00:00.080\n原稿0\n\n2\n00:00:00.130 --> 00:00:00.150\n原稿1\n\n"
        );
        assert_eq!(
            safe(&root, wav["files"][0]["subtitle_file"].as_str().unwrap()).unwrap(),
            vtt_path
        );
        assert_eq!(decoded.len(), 21120); // 0.22秒 × 48000 × stereo
        assert!((decoded[5760] - decoded[12480]).abs() < 0.0001);
        assert!((decoded[5760] - 0.1).abs() < 0.0001);
        settings["format"] = json!("flac");
        let flac = export_blocks(&root, &p, &settings).unwrap();
        let flac_path = safe(&root, flac["files"][0]["file"].as_str().unwrap()).unwrap();
        assert_eq!(
            std::fs::read_to_string(flac_path.with_extension("vtt")).unwrap(),
            vtt
        );
        let decoded_flac =
            audio_mixer_backend::recording::codecs::decode_audio_file(&flac_path).unwrap();
        assert_eq!(decoded_flac.samples.len(), decoded.len());
        assert!(decoded_flac
            .samples
            .iter()
            .zip(&decoded)
            .all(|(a, b)| (a - b).abs() < 0.0001));
        let preview = preview_blocks(&root, &p, &settings).unwrap();
        assert_eq!(preview["files"].as_array().unwrap().len(), 1);
        let folder =
            explorer_folder(&root, &format!("exports/{}", wav["id"].as_str().unwrap())).unwrap();
        assert!(!folder.to_string_lossy().starts_with("\\\\?\\"));
        assert!(!folder.to_string_lossy().contains('/'));
        assert!(folder.is_dir());
        assert!(explorer_folder(&root, "exports/not-found").is_err());
        assert!(explorer_folder(&root, "exports/../../outside").is_err());
        let mut last_settings = settings.clone();
        last_settings["from_last_item"] = json!(true);
        let last = preview_blocks(&root, &p, &last_settings).unwrap();
        assert_eq!(last["timeline"][0]["start_segment"], 1);
        // 同じブロックの先頭/末尾設定を保ち、最終発話だけを試聴する。
        assert_eq!(
            last["files"].as_array().unwrap().last().unwrap()["frames"],
            7200
        );
        let mut raw_settings = settings.clone();
        raw_settings["normalize"] = json!(false);
        let preview = preview_blocks(&root, &p, &raw_settings).unwrap();
        let raw =
            audio::samples(&safe(&root, preview["files"][0]["file"].as_str().unwrap()).unwrap())
                .unwrap();
        assert!((raw[5760] - 0.2).abs() < 0.0001);
        assert!((raw[12480] - 0.8).abs() < 0.0001);
        assert_eq!(
            p.segments[0].takes[0].hash,
            hash_file(&safe(&root, &p.segments[0].takes[0].file).unwrap()).unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn opens_export_root_before_export_and_subtitles_follow_trim_and_split() {
        let root = std::env::temp_dir().join(id());
        std::fs::create_dir_all(&root).unwrap();
        assert!(explorer_folder(&root, "exports").unwrap().is_dir());
        assert!(explorer_folder(&root, "exports/../outside").is_err());
        let mut p = Project::new("字幕".into());
        for text in ["<b>原稿</b> & -->\n\n次の行", "別ファイル"] {
            let path = root.join(format!("{}.wav", id()));
            write_wav(&path, &vec![0.1; 96000], false).unwrap();
            let mut t = crate::app::take_from_file(&root, &path, "test".into()).unwrap();
            t.trim_start = 0.1;
            t.trim_end = 0.9;
            let mut s = Segment::new(text.into());
            s.trim_start = 0.1;
            s.trim_end = 0.2;
            s.selected = Some(t.id.clone());
            s.takes.push(t);
            p.segments.push(s);
        }
        let settings =
            json!({"unit":"utterance","format":"wav","global_gaps":{"head":0.1,"tail":0.2}});
        let output = export_blocks(&root, &p, &settings).unwrap();
        assert_eq!(output["files"].as_array().unwrap().len(), 2);
        for file in output["files"].as_array().unwrap() {
            let path = safe(&root, file["file"].as_str().unwrap()).unwrap();
            let subtitle = safe(&root, file["subtitle_file"].as_str().unwrap()).unwrap();
            assert_eq!(path.with_extension("vtt"), subtitle);
            let text = std::fs::read_to_string(subtitle).unwrap();
            assert!(text.contains("00:00:00.100 --> 00:00:00.600"));
        }
        let map_path = safe(
            &root,
            &format!("exports/{}/block-map.json", output["id"].as_str().unwrap()),
        )
        .unwrap();
        let map: Value = serde_json::from_slice(&std::fs::read(map_path).unwrap()).unwrap();
        assert_eq!(map["schema"], "scriptsync.block-map");
        assert_eq!(map["schema_version"], 1);
        assert_eq!(map["blocks"].as_array().unwrap().len(), 2);
        assert_eq!(map["blocks"][0]["duration_frames"], 38400);
        assert_eq!(map["blocks"][1]["sequence_start_frame"], 38400);
        assert_eq!(map["total_duration_frames"], 76800);
        assert_eq!(map["blocks"][0]["utterances"][0]["start_frame"], 4800);
        assert_eq!(map["blocks"][0]["utterances"][0]["end_frame"], 28800);
        assert_eq!(map["blocks"][0]["text"], p.segments[0].text);
        for (i, block) in map["blocks"].as_array().unwrap().iter().enumerate() {
            assert_eq!(block["slide_number"], i + 1);
            let audio = root
                .join("exports")
                .join(output["id"].as_str().unwrap())
                .join(block["audio_file"].as_str().unwrap());
            assert_eq!(
                audio::samples(&audio).unwrap().len() / CHANNELS,
                block["duration_frames"].as_u64().unwrap() as usize
            );
            assert!(audio.with_extension("vtt").exists());
        }
        let first = std::fs::read_to_string(
            safe(&root, output["files"][0]["subtitle_file"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        assert!(first.contains("&lt;b&gt;原稿&lt;/b&gt; &amp; --&gt;\n次の行"));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn selected_preview_is_isolated_and_full_preview_reuses_valid_cache() {
        let root = std::env::temp_dir().join(id());
        std::fs::create_dir_all(&root).unwrap();
        let mut p = Project::new("キャッシュ".into());
        for _ in 0..2 {
            let path = root.join(format!("{}.wav", id()));
            write_wav(&path, &vec![0.1; 9600], false).unwrap();
            let t = crate::app::take_from_file(&root, &path, "test".into()).unwrap();
            let mut s = Segment::new("本文".into());
            s.selected = Some(t.id.clone());
            s.takes.push(t);
            p.segments.push(s);
        }
        let mut options = json!({"unit":"utterance","preview_scope":"block","selected_block":0});
        let other = p.segments[1].selected.take();
        let selected = preview_blocks(&root, &p, &options).unwrap();
        assert_eq!(selected["timeline"].as_array().unwrap().len(), 1);
        assert_eq!(selected["files"].as_array().unwrap().len(), 1);
        p.segments[1].selected = other;
        options["preview_scope"] = json!("all");
        let first = preview_blocks(&root, &p, &options).unwrap();
        assert_eq!(first["cached"], false);
        assert_eq!(first["timeline"].as_array().unwrap().len(), 2);
        let second = preview_blocks(&root, &p, &options).unwrap();
        assert_eq!(second["cached"], true);
        assert_eq!(first["files"], second["files"]);
        p.segments[0]
            .take_notes
            .insert("memo".into(), crate::domain::TakeNote::default());
        assert_eq!(preview_blocks(&root, &p, &options).unwrap()["cached"], true);
        options["global_gaps"] = json!({"normal":0.5});
        assert_eq!(
            preview_blocks(&root, &p, &options).unwrap()["cached"],
            false
        );
        let cached = preview_blocks(&root, &p, &options).unwrap();
        let file = safe(&root, cached["files"][0]["file"].as_str().unwrap()).unwrap();
        std::fs::write(file, b"broken").unwrap();
        assert_eq!(
            preview_blocks(&root, &p, &options).unwrap()["cached"],
            false
        );
        p.segments[0].gain_db = 1.0;
        assert_eq!(
            preview_blocks(&root, &p, &options).unwrap()["cached"],
            false
        );
        let input = safe(&root, &p.segments[0].takes[0].file).unwrap();
        std::fs::write(input, b"changed").unwrap();
        assert!(preview_blocks(&root, &p, &options).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn file_reorder_keeps_segment_state_and_id_keyed_output_overrides() {
        let root = std::env::temp_dir().join(format!("scriptsync-reorder-{}", id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut project = Project::new("ファイル順試験".into());
        let source_a = id();
        let source_b = id();
        project.script_sources = vec![
            ScriptSource {
                id: source_a.clone(),
                name: "a.md".into(),
                kind: "markdown".into(),
                confirmed_text: "A".into(),
                draft_text: "A".into(),
                lines: crate::markdown::parse("A").unwrap(),
                draft_lines: vec![],
                blank_rows: vec![],
                approved_text: None,
            },
            ScriptSource {
                id: source_b.clone(),
                name: "b.md".into(),
                kind: "markdown".into(),
                confirmed_text: "B".into(),
                draft_text: "B".into(),
                lines: crate::markdown::parse("B").unwrap(),
                draft_lines: vec![],
                blank_rows: vec![],
                approved_text: None,
            },
        ];
        project.script_revision = 1;
        let mut segments = Vec::new();
        for (source_id, text, amplitude) in [(&source_a, "A", 800i16), (&source_b, "B", 1200i16)] {
            let file = format!("{text}.wav");
            let path = root.join(&file);
            let mut writer = hound::WavWriter::create(&path, audio::spec()).unwrap();
            for _ in 0..960 {
                writer.write_sample(amplitude).unwrap();
            }
            writer.finalize().unwrap();
            let mut segment = Segment::new(text.into());
            segment.source_id = source_id.clone();
            segment.source_line = Some(0);
            segment.source_text = text.into();
            let take = crate::app::take_from_file(&root, &path, text.into()).unwrap();
            segment.selected = Some(take.id.clone());
            segment.takes.push(take);
            segment.corrections.push(Correction {
                id: id(),
                start: 0,
                end: 1,
                original: text.into(),
                replacement: format!("{text}読み"),
                decision: "accepted".into(),
            });
            segment.comparisons.push(Comparison {
                kind: String::new(),
                correction_hash: String::new(),
                stale: false,
                reference_kana: String::new(),
                id: format!("comparison-{text}"),
                source_hash: String::new(),
                reference_hash: String::new(),
                variants: vec![],
                order: vec![],
                revealed: false,
                created: now(),
            });
            segments.push(segment);
        }
        let first_id = segments[0].id.clone();
        let second_id = segments[1].id.clone();
        project.segments = segments;
        let mut a_line = json!(crate::markdown::parse("A").unwrap()[0]);
        a_line["previous_id"] = first_id.clone().into();
        a_line["keep"] = true.into();
        let mut b_line = json!(crate::markdown::parse("B").unwrap()[0]);
        b_line["previous_id"] = second_id.clone().into();
        b_line["keep"] = true.into();
        crate::script_sources::commit(&mut project, &json!({"base_revision":1,"sources":[{"id":source_a,"name":"a.md","kind":"markdown","text":"A","lines":[a_line],"approved_text":"A"},{"id":source_b,"name":"b.md","kind":"markdown","text":"B","lines":[b_line],"approved_text":"B"}]})).unwrap();
        let mut b_line = json!(crate::markdown::parse("B").unwrap()[0]);
        b_line["previous_id"] = second_id.clone().into();
        b_line["keep"] = true.into();
        let mut a_line = json!(crate::markdown::parse("A").unwrap()[0]);
        a_line["previous_id"] = first_id.clone().into();
        a_line["keep"] = true.into();
        let revision = project.script_revision;
        crate::script_sources::commit(&mut project, &json!({"base_revision":revision,"sources":[{"id":source_b,"name":"b.md","kind":"markdown","text":"B","lines":[b_line],"approved_text":"B"},{"id":source_a,"name":"a.md","kind":"markdown","text":"A","lines":[a_line],"approved_text":"A"}]})).unwrap();
        assert_eq!(
            project
                .segments
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            vec![second_id.as_str(), first_id.as_str()]
        );
        assert_eq!(project.segments[0].corrections[0].replacement, "B読み");
        assert_eq!(project.segments[1].corrections[0].replacement, "A読み");
        assert_eq!(project.segments[0].comparisons[0].id, "comparison-B");
        assert_eq!(project.segments[1].comparisons[0].id, "comparison-A");
        let mut options = json!({"unit":"utterance","format":"wav","normalize":false,"global_gaps":{"normal":0.0,"head":0.0,"tail":0.0,"part":0.0,"chapter":0.0,"section":0.0},"blocks":{}});
        options["blocks"][&second_id] = json!({"gaps":{"normal":{"mode":"auto"},"head":{"mode":"manual","value":0.01},"tail":{"mode":"auto"}}});
        options["blocks"][&first_id] = json!({"gaps":{"normal":{"mode":"auto"},"head":{"mode":"manual","value":0.02},"tail":{"mode":"auto"}}});
        let manifest = export_blocks(&root, &project, &options).unwrap();
        let output_file = safe(&root, manifest["files"][0]["file"].as_str().unwrap()).unwrap();
        let map_path = output_file.parent().unwrap().join("block-map.json");
        let map: Value = serde_json::from_slice(&std::fs::read(map_path).unwrap()).unwrap();
        let blocks = map["blocks"].as_array().unwrap();
        assert_eq!(blocks[0]["utterances"][0]["segment_id"], second_id);
        assert_eq!(blocks[1]["utterances"][0]["segment_id"], first_id);
        assert!(
            manifest["files"][0]["frames"].as_u64().unwrap()
                < manifest["files"][1]["frames"].as_u64().unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn filename_text_removes_invalid_characters_and_limits_unicode_characters() {
        assert_eq!(filename_text(" <>:\"/\\|?*\n原稿. "), "原稿");
        assert_eq!(filename_text("<>::\n"), "script");
        assert_eq!(filename_text(&"あ😀".repeat(20)), "あ😀".repeat(10));
        assert_eq!(
            filename_text("1234567890123456789.残り"),
            "1234567890123456789"
        );
    }
}
