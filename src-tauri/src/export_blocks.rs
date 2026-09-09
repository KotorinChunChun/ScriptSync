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
    _block: usize,
    v: &Value,
) -> Result<Vec<f32>> {
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
        .filter(|&i| !project.segments[i].skip)
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
        let from = (seg.trim_start * RATE as f64).round() as usize * CHANNELS;
        let tail = (seg.trim_end * RATE as f64).round() as usize * CHANNELS;
        if from >= input.len() || tail >= input.len() - from {
            return Err("トリム範囲が不正です".into());
        }
        let clip = &input[from..input.len() - tail];
        let gain = if v["normalize"].as_bool().unwrap_or(true) {
            normalized_gain(clip)
        } else {
            10f32.powf(seg.gain_db as f32 / 20.0)
        };
        out.extend(clip.iter().map(|x| x * gain));
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
    Ok(out)
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
        let data = collect_block(root, project, start, end, gap, overrides, i, v)?;
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
        files.push(json!({"block":i,"range":{"start":start,"end":end},"file":rel,"hash":hash,"stem":stem(v,project,start,i),"name":format!("{}.{}",safe_name(&output_name),ext),"frames":data.len()/CHANNELS,"clipped_samples":clipped}));
    }
    let manifest = json!({"id":export_id,"format":format,"project":project.id,"generation":project.generation,"files":files});
    json_new(&folder.join("manifest.json"), &manifest)?;
    Ok(manifest)
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
        if i < selected {
            continue;
        }
        let end = b.get(i + 1).copied().unwrap_or(project.segments.len());
        let data = collect_block(root, project, start, end, gap, overrides, i, v)?;
        let duration = data.len() as f64 / (RATE as f64 * CHANNELS as f64);
        all.extend_from_slice(&data);
        timeline.push(json!({"block":i,"start_segment":start,"end_segment":end,"offset":offset,"duration":duration}));
        offset += duration;
        if i == selected {
            for (mode, norm) in [("raw", false), ("normalize", true)] {
                let p = folder.join(format!("{mode}.wav"));
                let mut variant = v.clone();
                variant["normalize"] = json!(norm);
                let variant_data =
                    collect_block(root, project, start, end, gap, overrides, i, &variant)?;
                let (h, _) = write_wav(&p, &variant_data, false)?;
                result.push(json!({"mode":mode,"file":format!("exports/preview-{id}/{mode}.wav"),"hash":h,"frames":data.len()/CHANNELS}));
            }
        }
    }
    let p = folder.join("combined.wav");
    let (h, _) = write_wav(&p, &all, false)?;
    result.push(json!({"mode":"combined","file":format!("exports/preview-{id}/combined.wav"),"hash":h,"frames":all.len()/CHANNELS}));
    Ok(json!({"selected_block":selected,"files":result,"timeline":timeline}))
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
        assert_eq!(decoded.len(), 21120); // 0.22秒 × 48000 × stereo
        assert!((decoded[5760] - decoded[12480]).abs() < 0.0001);
        assert!((decoded[5760] - 0.1).abs() < 0.0001);
        settings["format"] = json!("flac");
        let flac = export_blocks(&root, &p, &settings).unwrap();
        let flac_path = safe(&root, flac["files"][0]["file"].as_str().unwrap()).unwrap();
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
}
