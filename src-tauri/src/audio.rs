use crate::{domain::*, storage::*};
use audio_mixer_backend::audio::resampler::to_internal;
use std::{
    fs::OpenOptions,
    io::{BufWriter, Cursor},
    path::Path,
};
pub fn samples(path: &Path) -> Result<Vec<f32>> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
        return Err("1テイクは128 MB以内に分割してください".into());
    }
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let input: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
        reader
            .samples::<f32>()
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| e.to_string())?
    } else {
        let scale = 2f32.powi(spec.bits_per_sample as i32 - 1);
        reader
            .samples::<i32>()
            .map(|s| s.map(|x| x as f32 / scale))
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| e.to_string())?
    };
    if !(8000..=192000).contains(&spec.sample_rate)
        || !(1..=2).contains(&spec.channels)
        || input.is_empty()
        || input.iter().any(|x| !x.is_finite())
    {
        return Err("音声の形式・長さ・数値が不正です（mono/stereo WAV対応）".into());
    }
    Ok(to_internal(&input, spec.sample_rate, spec.channels).samples)
}
pub fn inspect(path: &Path) -> Result<(f64, f32)> {
    let s = samples(path)?;
    Ok((
        s.len() as f64 / 96000.0,
        s.iter().fold(0.0f32, |a, b| a.max(b.abs())),
    ))
}
fn clean_recorded_samples(input: &[f32], channels: usize, rate: u32) -> Result<Vec<f32>> {
    if channels == 0 || rate == 0 || input.is_empty() || input.len() % channels != 0 {
        return Err("録音音声の形式が不正です".into());
    }
    let mut filtered = input.to_vec();
    // 前後の傾きに対して孤立して大きい1サンプルの変位だけを補間する。
    // 連続する音声波形や発話中の無音区間は変更しない。
    for _ in 0..2 {
        let previous = filtered.clone();
        for index in 1..previous.len() / channels - 1 {
            for channel in 0..channels {
                let at = index * channels + channel;
                let before = previous[at - channels];
                let after = previous[at + channels];
                let baseline = (before + after) * 0.5;
                let deviation = (previous[at] - baseline).abs();
                let surrounding = (after - before).abs();
                if deviation > 0.018 && deviation > surrounding * 6.0 {
                    filtered[at] = baseline;
                }
            }
        }
    }
    let frames = filtered.len() / channels;
    let window = (rate as usize / 100).max(1); // 10 ms
    let peak = filtered
        .iter()
        .fold(0.0f32, |p, sample| p.max(sample.abs()));
    let threshold = (peak * 0.03).clamp(0.0025, 0.01);
    let mut first = None;
    let mut last = None;
    for begin in (0..frames).step_by(window) {
        let end = (begin + window).min(frames);
        let slice = &filtered[begin * channels..end * channels];
        let rms = (slice.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / slice.len() as f64)
            .sqrt() as f32;
        if rms >= threshold {
            first.get_or_insert(begin);
            last = Some(end);
        }
    }
    let (Some(first_window), Some(last_window)) = (first, last) else {
        return Ok(filtered);
    };
    // 10 ms窓で発話を安定して検出した後、その窓の内側だけをフレーム単位で
    // 探し直す。これにより検出窓の端に含まれる余分な無音を残さない。
    let first_window_end = (first_window + window).min(frames);
    let first = (first_window..first_window_end)
        .find(|frame| {
            filtered[frame * channels..(frame + 1) * channels]
                .iter()
                .any(|sample| sample.abs() >= threshold)
        })
        .unwrap_or(first_window);
    let last_window_start = last_window.saturating_sub(window);
    let last = (last_window_start..last_window)
        .rev()
        .find(|frame| {
            filtered[frame * channels..(frame + 1) * channels]
                .iter()
                .any(|sample| sample.abs() >= threshold)
        })
        .map(|frame| frame + 1)
        .unwrap_or(last_window);
    let padding = (u64::from(rate) / 10) as usize; // 発話前後を各0.1秒保持
    let start = first.saturating_sub(padding);
    let end = (last + padding).min(frames);
    let mut output = filtered[start * channels..end * channels].to_vec();
    // ファイル境界を5 msだけゼロへなじませ、再生開始・終了のクリックを防ぐ。
    let fade = ((u64::from(rate) * 5 / 1000) as usize).min(output.len() / channels / 2);
    for frame in 0..fade {
        let gain = frame as f32 / fade.max(1) as f32;
        let tail_gain = (fade - frame - 1) as f32 / fade.max(1) as f32;
        for channel in 0..channels {
            output[frame * channels + channel] *= gain;
            let tail = output.len() - (frame + 1) * channels + channel;
            output[tail] *= tail_gain;
        }
    }
    Ok(output)
}
pub fn clean_recording(path: &Path) -> Result<(f64, f32)> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let wav_spec = reader.spec();
    if wav_spec.sample_format != hound::SampleFormat::Float || wav_spec.bits_per_sample != 32 {
        return Err("録音WAVはFloat32である必要があります".into());
    }
    let input = reader
        .samples::<f32>()
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(reader);
    let output = clean_recorded_samples(&input, wav_spec.channels as usize, wav_spec.sample_rate)?;
    let temporary = path.with_extension(format!("clean-{}.tmp", id()));
    {
        let mut writer =
            hound::WavWriter::create(&temporary, wav_spec).map_err(|e| e.to_string())?;
        for sample in &output {
            writer.write_sample(*sample).map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    let backup = path.with_extension(format!("raw-{}.tmp", id()));
    std::fs::rename(path, &backup).map_err(|e| e.to_string())?;
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::rename(&backup, path);
        let _ = std::fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    let _ = std::fs::remove_file(backup);
    inspect(path)
}
pub fn spec() -> hound::WavSpec {
    hound::WavSpec {
        channels: 2,
        sample_rate: 48000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    }
}
fn pcm(x: f32) -> i16 {
    (x.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

#[cfg(test)]
mod recording_cleanup_tests {
    use super::clean_recorded_samples;

    #[test]
    fn trims_only_edges_keeps_point_one_seconds_and_suppresses_isolated_click() {
        let rate = 1000;
        let mut samples = vec![0.0; 2000];
        for frame in 500..700 {
            samples[frame * 2] = 0.1;
            samples[frame * 2 + 1] = 0.1;
        }
        samples[600 * 2] = 0.9;
        samples[600 * 2 + 1] = 0.9;
        for frame in 620..640 {
            samples[frame * 2] = 0.0;
            samples[frame * 2 + 1] = 0.0;
        }
        let cleaned = clean_recorded_samples(&samples, 2, rate).unwrap();
        assert_eq!(cleaned.len() / 2, 400);
        assert!(cleaned.iter().all(|sample| sample.abs() <= 0.11));
        assert_eq!(cleaned[220 * 2], 0.0, "発話途中の無音は保持する");
    }

    #[test]
    fn trims_window_remainder_and_keeps_exact_point_one_second_padding() {
        let rate = 1000;
        let mut samples = vec![0.0; 2000];
        for frame in 503..697 {
            samples[frame * 2] = 0.1;
            samples[frame * 2 + 1] = 0.1;
        }
        let cleaned = clean_recorded_samples(&samples, 2, rate).unwrap();
        assert_eq!(cleaned.len() / 2, 394);
    }
}
pub fn preview(path: &Path) -> Result<Vec<u8>> {
    let s = samples(path)?;
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut bytes, spec()).map_err(|e| e.to_string())?;
        for v in s {
            w.write_sample(pcm(v)).map_err(|e| e.to_string())?
        }
        w.finalize().map_err(|e| e.to_string())?;
    }
    Ok(bytes.into_inner())
}
pub fn assemble(root: &Path, project: &Project) -> Result<serde_json::Value> {
    let export_id = id();
    let relative = format!("exports/{export_id}/audio.wav");
    let final_path = safe(root, &relative)?;
    let mut entries = Vec::new();
    let mut total = 0u64;
    for s in project.segments.iter().filter(|s| !s.skip) {
        let t = s.selected_take().map_err(|e| format!("{}: {e}", s.text))?;
        let path = safe(root, &t.file)?;
        if hash_file(&path)? != t.hash {
            return Err("採用音声が差し替えられています".into());
        }
        let (duration, _) = inspect(&path)?;
        if !s.trim_start.is_finite()
            || !s.trim_end.is_finite()
            || !s.gap.is_finite()
            || s.trim_start < 0.0
            || s.trim_end < 0.0
            || s.gap < 0.0
            || s.gap > 60.0
            || s.trim_start + s.trim_end >= duration
        {
            return Err("トリム・間の範囲が不正です".into());
        }
        let from = (s.trim_start * 48000.0).round() as usize;
        let end = (duration * 48000.0).round() as usize - (s.trim_end * 48000.0).round() as usize;
        let gap = (s.gap * 48000.0).round() as usize;
        if !s.gain_db.is_finite() || !(-36.0..=24.0).contains(&s.gain_db) {
            return Err("音量が不正です".into());
        }
        entries.push(serde_json::json!({"segment":s.id,"take":t.id,"hash":t.hash,"file":t.file,"gain_db":s.gain_db,"source_start":from,"source_end":end,"gap":gap,"start_frame":total,"end_frame":total+(end-from) as u64}));
        total += (end - from + gap) as u64;
    }
    if entries.is_empty() {
        return Err("出力対象の発話がありません".into());
    }
    if total * 4 > u32::MAX as u64 - 128 {
        return Err("WAVの4 GB上限を超えます。台本を分割してください".into());
    }
    let folder = final_path.parent().unwrap();
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let mut manifest = serde_json::json!({"id":export_id,"project":project.id,"generation":project.generation,"status":"running","sample_rate":48000,"channels":2,"frames":total,"entries":entries});
    json_new(&folder.join("plan.json"), &manifest)?;
    let pending = folder.join("pending.wav");
    let f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .map_err(|e| e.to_string())?;
    let mut writer = hound::WavWriter::new(BufWriter::new(f), spec()).map_err(|e| e.to_string())?;
    let mut clipped = 0u64;
    for e in &entries {
        let path = safe(root, e["file"].as_str().unwrap())?;
        if hash_file(&path)? != e["hash"].as_str().unwrap() {
            return Err("出力中に入力音声が変わりました".into());
        }
        let s = samples(&path)?;
        let from = e["source_start"].as_u64().unwrap() as usize * 2;
        let end = e["source_end"].as_u64().unwrap() as usize * 2;
        for &v in s.get(from..end).ok_or("音声区間が不正です")? {
            let v = v * 10f32.powf(e["gain_db"].as_f64().unwrap_or(0.0) as f32 / 20.0);
            if v.abs() > 1.0 {
                clipped += 1
            }
            writer.write_sample(pcm(v)).map_err(|e| e.to_string())?;
        }
        for _ in 0..e["gap"].as_u64().unwrap() * 2 {
            writer.write_sample(0i16).map_err(|e| e.to_string())?;
        }
    }
    writer.finalize().map_err(|e| e.to_string())?;
    std::fs::rename(pending, &final_path).map_err(|e| e.to_string())?;
    manifest["status"] = "succeeded".into();
    manifest["hash"] = hash_file(&final_path)?.into();
    manifest["file"] = relative.into();
    manifest["clipped_samples"] = clipped.into();
    json_new(&folder.join("manifest.json"), &manifest)?;
    Ok(manifest)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(path: &Path, value: i16) {
        let mut w = hound::WavWriter::create(path, spec()).unwrap();
        for _ in 0..960 {
            w.write_sample(value).unwrap()
        }
        w.finalize().unwrap();
    }
    #[test]
    fn export_order_missing_and_tamper() {
        let root = std::env::temp_dir().join(id());
        std::fs::create_dir_all(&root).unwrap();
        let mut p = Project::new("試験".into());
        for (name, v) in [("a.wav", 1000), ("b.wav", -1000)] {
            fixture(&root.join(name), v);
            let mut s = Segment::new(name.into());
            let t = Take {
                id: id(),
                file: name.into(),
                hash: hash_file(&root.join(name)).unwrap(),
                status: "ready".into(),
                duration: 0.01,
                peak: 0.1,
                warning: String::new(),
                created: now(),
                endpoint: String::new(),
            };
            s.selected = Some(t.id.clone());
            s.takes.push(t);
            p.segments.push(s);
        }
        let result = assemble(&root, &p).unwrap();
        let mut r = hound::WavReader::open(root.join(result["file"].as_str().unwrap())).unwrap();
        let values: Vec<i16> = r.samples().map(|s| s.unwrap()).collect();
        assert!(values[0] > 0 && values[960] < 0);
        p.segments[0].gain_db = 6.0206;
        let adjusted = assemble(&root, &p).unwrap();
        let mut r = hound::WavReader::open(root.join(adjusted["file"].as_str().unwrap())).unwrap();
        assert!((r.samples::<i16>().next().unwrap().unwrap() - 2000).abs() <= 2);
        assert_eq!(
            hash_file(&root.join("a.wav")).unwrap(),
            p.segments[0].takes[0].hash
        );
        p.segments[0].selected = None;
        assert!(assemble(&root, &p).is_err());
        p.segments[0].selected = Some(p.segments[0].takes[0].id.clone());
        fixture(&root.join("a.wav"), 2000);
        assert!(assemble(&root, &p).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
