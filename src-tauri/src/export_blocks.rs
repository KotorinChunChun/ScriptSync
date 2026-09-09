//! 台本を発話ブロック単位で非破壊に出力するバックエンド。
//!
//! 境界は開始発話ID、階層は部・章・節・項。原音を変更せず別フォルダーへ出力する。
use crate::{audio, domain::*, storage::*};

use hound::{SampleFormat, WavSpec, WavWriter};
use serde_json::{Value, json};
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
fn stem(v: &Value, project: &Project, start: usize, block: usize) -> String {
    v.get("stems")
        .and_then(Value::as_array)
        .and_then(|a| a.get(block))
        .and_then(Value::as_str)
        .or_else(|| {
            project.segments.get(start).and_then(|s| {
                v.get("blocks")
                    .and_then(|b| b.get(&s.id))
                    .and_then(|x| x.get("name"))
                    .and_then(Value::as_str)
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
        .to_string()
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
fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(110)
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
) -> Result<(Vec<f32>, String)> {
    let mut subtitles = String::from("WEBVTT\n\n");
    if start >= end || end > project.segments.len() {
        return Err("出力する原稿がありません".into());
    }
    let settings = &overrides[&project.segments[start].id]["gaps"];
    let seconds = |key: &str| -> Result<f64> {
        let n = if settings[key]["mode"] == "manual" {
            settings[key]["value"]
                .as_f64()
                .ok_or("手入力の間隔が不正です")?
        } else {
            gap_for(gap, key)
        };
        if !n.is_finite() || !(0.0..=60.0).contains(&n) {
            return Err("間隔は0～60秒にしてください".into());
        }
        Ok(n)
    };
    let active: Vec<_> = (start..end)
        .filter(|&i| {
            !project.segments[i].skip && i >= v["_preview_from"].as_u64().unwrap_or(0) as usize
        })
        .collect();
    if active.is_empty() {
        return Err("出力対象の原稿がありません".into());
    }
    let mut out = vec![0.0; (seconds("head")? * RATE as f64).round() as usize * CHANNELS];
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
        let (take_start, take_end) = take.trimmed_range()?;
        let kept = take_end - take_start;
        if !seg.trim_start.is_finite()
            || !seg.trim_end.is_finite()
            || seg.trim_start < 0.0
            || seg.trim_end < 0.0
            || seg.trim_start + seg.trim_end >= kept
        {
            return Err("トリム範囲が不正です".into());
        }
        let from = ((take_start + seg.trim_start) * RATE as f64).round() as usize * CHANNELS;
        let end = ((take_end - seg.trim_end) * RATE as f64).round() as usize * CHANNELS;
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
            // 見出し自体を読まない場合も所属階層の変化から区切りを判定する。
            let nh = &project.segments[next].hierarchy;
            let h = &seg.hierarchy;
            let boundary = (0..3).find(|&k| h.get(k) != nh.get(k));
            let key = boundary
                .map(|k| ["part", "chapter", "section"][k].to_owned())
                .unwrap_or_else(|| kind(project, next));
            out.extend(std::iter::repeat_n(
                0.0,
                (seconds(&key)? * RATE as f64).round() as usize * CHANNELS,
            ));
        }
    }
    out.extend(std::iter::repeat_n(
        0.0,
        (seconds("tail")? * RATE as f64).round() as usize * CHANNELS,
    ));
    Ok((out, subtitles))
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
    for (i, &start) in b.iter().enumerate() {
        let end = b.get(i + 1).copied().unwrap_or(project.segments.len());
        let (data, subtitles) =
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
        crate::storage::write_new(&subtitle_path, subtitles.as_bytes())?;
        let subtitle_file = std::path::Path::new(&rel)
            .with_extension("vtt")
            .to_string_lossy()
            .replace('\\', "/");
        files.push(json!({"subtitle_file":subtitle_file,"subtitle_hash":hash_file(&subtitle_path)?,"block":i,"range":{"start":start,"end":end},"file":rel,"hash":hash,"stem":stem(v,project,start,i),"name":format!("{}.{}",safe_name(&output_name),ext),"frames":data.len()/CHANNELS,"clipped_samples":clipped}));
    }
    let manifest = json!({"id":export_id,"format":format,"project":project.id,"generation":project.generation,"files":files});
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
        assert_eq!(safe_name("0001_日本語:原稿/題名"), "0001_日本語_原稿_題名");
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
        assert!(
            decoded_flac
                .samples
                .iter()
                .zip(&decoded)
                .all(|(a, b)| (a - b).abs() < 0.0001)
        );
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
}
