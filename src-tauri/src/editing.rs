use crate::{
    app::{segment, string},
    audio,
    domain::*,
    storage::*,
};
use serde_json::{Value, json};
use std::path::Path;

pub fn waveform(path: &Path) -> Result<Value> {
    let samples = audio::samples(path)?;
    let peaks: Vec<f32> = samples
        .chunks((samples.len() / 800).max(2))
        .map(|c| c.iter().fold(0.0f32, |p, x| p.max(x.abs())))
        .collect();
    let rms =
        (samples.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / samples.len() as f64).sqrt();
    let peak = samples.iter().fold(0.0f32, |p, x| p.max(x.abs()));
    let rms_db = 20.0 * rms.max(0.000001).log10();
    let suggested = (-20.0 - rms_db)
        .min(-1.0 - 20.0 * (peak as f64).max(0.000001).log10())
        .clamp(-36.0, 24.0);
    Ok(
        json!({"peaks":peaks,"duration":samples.len() as f64/96000.0,"rms_db":rms_db,"peak_db":20.0*(peak as f64).max(0.000001).log10(),"suggested_gain":if rms<0.000001{0.0}else{suggested}}),
    )
}
fn lines(text: &str) -> Result<Vec<String>> {
    let result: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if result.is_empty() || result.len() > 1000 {
        return Err("台本は1～1000発話にしてください".into());
    }
    Ok(result)
}
fn derived(text: String, sources: &[Segment]) -> Segment {
    let mut s = Segment::new(text);
    s.needs_review = true;
    for source in sources {
        for old in &source.takes {
            let mut t = old.clone();
            t.id = id();
            if let Some(n) = source.take_notes.get(&old.id) {
                s.take_notes.insert(t.id.clone(), n.clone());
            }
            s.takes.push(t);
        }
    }
    s
}
pub fn revision_preview(p: &Project, text: &str) -> Result<Value> {
    let proposed = lines(text)?;
    // 同一位置・同一原稿のみ保持し、それ以外は明示的な再確認対象にする。
    let rows:Vec<Value>=(0..proposed.len().max(p.segments.len())).map(|i|{
        let old=p.segments.get(i);let new=proposed.get(i);
        json!({"index":i,"before":old.map(|s|&s.text),"after":new,"status":if old.map(|s|&s.text)==new{"unchanged"}else if old.is_none(){"added"}else if new.is_none(){"removed"}else{"changed"}})
    }).collect();
    Ok(json!({"generation":p.generation,"rows":rows}))
}
pub fn edit_script(p: &mut Project, action: &str, v: &Value) -> Result<()> {
    if action == "revise_script" {
        if v["generation"].as_u64() != Some(p.generation) {
            return Err("台本が更新されました。差分を再確認してください".into());
        }
        let proposed = lines(&string(v, "text")?)?;
        let old = std::mem::take(&mut p.segments);
        for (i, text) in proposed.into_iter().enumerate() {
            if let Some(s) = old.get(i) {
                if s.text == text {
                    p.segments.push(s.clone());
                } else {
                    p.segments.push(derived(text, std::slice::from_ref(s)));
                    p.archived_segments.push(s.clone());
                }
            } else {
                p.segments.push(Segment::new(text));
            }
        }
        p.archived_segments
            .extend(old.into_iter().skip(p.segments.len()));
        return Ok(());
    }
    let sid = string(v, "segment")?;
    if action == "restore_segment" {
        let i = p
            .archived_segments
            .iter()
            .position(|s| s.id == sid)
            .ok_or("履歴がありません")?;
        if p.segments.len() >= 1000 {
            return Err("発話数の上限です".into());
        }
        p.segments.push(p.archived_segments.remove(i));
        return Ok(());
    }
    let i = p
        .segments
        .iter()
        .position(|s| s.id == sid)
        .ok_or("発話がありません")?;
    if action == "split" {
        if p.segments.len() >= 1000 {
            return Err("発話数の上限です".into());
        }
        let s = p.segments[i].clone();
        let at = v["position"].as_u64().ok_or("分割位置が必要です")? as usize;
        let left = slice(&s.text, 0, at)?;
        let right = slice(&s.text, at, s.text.chars().count())?;
        if left.trim().is_empty() || right.trim().is_empty() {
            return Err("空の発話には分割できません".into());
        }
        p.segments.splice(
            i..=i,
            [
                derived(left, std::slice::from_ref(&s)),
                derived(right, std::slice::from_ref(&s)),
            ],
        );
        p.archived_segments.push(s);
    } else if action == "merge" {
        if i + 1 >= p.segments.len() {
            return Err("後ろの発話がありません".into());
        }
        let sources = p.segments[i..i + 2].to_vec();
        let text = format!("{}{}", sources[0].text, sources[1].text);
        p.segments.splice(i..i + 2, [derived(text, &sources)]);
        p.archived_segments.extend(sources);
    } else {
        return Err("未対応の台本操作です".into());
    }
    Ok(())
}
pub fn preview(store: &Store, v: &Value) -> Result<Value> {
    let mut p = store.project.clone();
    let sid = string(v, "segment")?;
    let i = p
        .segments
        .iter()
        .position(|s| s.id == sid)
        .ok_or("発話がありません")?;
    let seam = v["seam"].as_bool().unwrap_or(false);
    if seam && i + 1 >= p.segments.len() {
        return Err("次の発話がありません".into());
    }
    p.segments = p.segments[i..if seam { i + 2 } else { i + 1 }].to_vec();
    let s = segment(&mut p, &sid)?;
    for (key, target) in [
        ("trim_start", &mut s.trim_start),
        ("trim_end", &mut s.trim_end),
        ("gap", &mut s.gap),
        ("gain_db", &mut s.gain_db),
    ] {
        if let Some(n) = v[key].as_f64() {
            *target = n;
        }
    }
    if seam {
        let a = &mut p.segments[0];
        let d = a.selected_take()?.duration;
        a.trim_start = a.trim_start.max(d - a.trim_end - 1.0);
        let b = &mut p.segments[1];
        let d = b.selected_take()?.duration;
        b.trim_end = b.trim_end.max(d - b.trim_start - 1.0);
        b.gap = 0.0;
    } else {
        p.segments[0].gap = 0.0;
    }
    // 完成音声と同じ変換・トリム・ゲイン経路を通して試聴する。
    audio::assemble(&store.root, &p)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_and_merge_preserve_originals() {
        let mut p = Project::new("試験".into());
        p.segments.push(Segment::new("😀市場へ".into()));
        let sid = p.segments[0].id.clone();
        edit_script(&mut p, "split", &json!({"segment":sid,"position":1})).unwrap();
        assert_eq!(p.segments[0].text, "😀");
        assert_eq!(p.archived_segments[0].text, "😀市場へ");
        let sid = p.segments[0].id.clone();
        edit_script(&mut p, "merge", &json!({"segment":sid})).unwrap();
        assert_eq!(p.segments[0].text, "😀市場へ");
        assert!(p.segments[0].needs_review);
    }
    #[test]
    fn revision_requires_reviewed_generation() {
        let mut p = Project::new("試験".into());
        p.segments.push(Segment::new("元".into()));
        assert!(
            edit_script(
                &mut p,
                "revise_script",
                &json!({"text":"変更","generation":1})
            )
            .is_err()
        );
        edit_script(
            &mut p,
            "revise_script",
            &json!({"text":"変更","generation":0}),
        )
        .unwrap();
        assert_eq!(p.archived_segments[0].text, "元");
        assert!(p.segments[0].selected.is_none());
    }
}
