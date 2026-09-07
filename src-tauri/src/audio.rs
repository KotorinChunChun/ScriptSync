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
