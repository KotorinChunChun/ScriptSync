use crate::{
    app::{segment, string},
    audio,
    domain::*,
    storage::*,
};
use serde_json::{json, Value};
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
    let mut s = Segment::new(text.clone());
    s.needs_review = true;
    let file_mode = sources.iter().any(|source| !source.source_id.is_empty());
    if let Some(source) = sources.first() {
        s.source_id = source.source_id.clone();
        s.source_line = if file_mode { None } else { source.source_line };
        s.kind = source.kind;
        s.hierarchy = source.hierarchy.clone();
        s.source_text = if file_mode {
            text
        } else {
            sources
                .iter()
                .map(|s| {
                    if s.source_text.is_empty() {
                        s.text.as_str()
                    } else {
                        s.source_text.as_str()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
    }
    if !file_mode {
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
    }
    s
}
fn sync_file_draft(p: &mut Project, originals: &[Segment], replacements: &[Segment]) -> Result<()> {
    let source_id = originals
        .first()
        .map(|source| source.source_id.as_str())
        .filter(|source_id| !source_id.is_empty())
        .ok_or("台本ファイル行が必要です")?;
    if originals
        .iter()
        .any(|source| source.source_id != source_id || source.source_line.is_none())
    {
        return Err("台本ファイル行の対応位置が不明です".into());
    }
    let lines: Vec<usize> = originals
        .iter()
        .map(|source| source.source_line.unwrap())
        .collect();
    if lines.windows(2).any(|pair| pair[1] != pair[0] + 1) {
        return Err("隣接していない台本行は結合できません".into());
    }
    let source = p
        .script_sources
        .iter_mut()
        .find(|source| source.id == source_id)
        .ok_or("台本ファイルが見つかりません")?;
    let physical: Vec<String> = source.draft_text.split('\n').map(str::to_owned).collect();
    let start = lines[0];
    let end = *lines.last().unwrap();
    if end >= physical.len() {
        return Err("台本下書きの対応位置が不正です".into());
    }
    let mut rows = if source.draft_lines.is_empty() {
        serde_json::to_value(crate::markdown::parse(&source.draft_text)?)
            .map_err(|e| e.to_string())?
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        source.draft_lines.clone()
    };
    let edited = source.draft_text != source.confirmed_text;
    for original in originals {
        let line = original.source_line.unwrap();
        let row = rows.iter().find(|row| {
            row["previous_id"].as_str() == Some(original.id.as_str())
                || (!edited && row["line"].as_u64() == Some(line as u64))
        });
        if row.is_none() || (edited && row.unwrap()["keep"].as_bool() != Some(true)) {
            return Err("台本下書きに未確定の編集があるため自動同期できません".into());
        }
    }
    let mut updated_physical = physical;
    updated_physical.splice(
        start..=end,
        replacements
            .iter()
            .map(|replacement| replacement.text.clone()),
    );
    let delta = replacements.len() as isize - (end - start + 1) as isize;
    let mut updated_rows = Vec::with_capacity(rows.len() + delta.max(0) as usize);
    for row in rows.drain(..) {
        let line = row["line"].as_u64().ok_or("台本下書きの行番号が不正です")? as usize;
        if (start..=end).contains(&line) {
            continue;
        }
        let mut row = row;
        let shifted = if line > end {
            (line as isize + delta) as usize
        } else {
            line
        };
        row["line"] = shifted.into();
        updated_rows.push(row);
    }
    for (offset, replacement) in replacements.iter().enumerate() {
        let parsed = crate::markdown::parse(&replacement.text)?;
        let mut row = serde_json::to_value(parsed.first().ok_or("派生した台本行を解析できません")?)
            .map_err(|e| e.to_string())?;
        row["line"] = (start + offset).into();
        row["previous_id"] = replacement.id.clone().into();
        row["keep"] = true.into();
        updated_rows.push(row);
    }
    updated_rows.sort_by_key(|row| row["line"].as_u64().unwrap_or(usize::MAX as u64));
    source.draft_text = updated_physical.join("\n");
    source.draft_lines = updated_rows;
    source.blank_rows.retain_mut(|row| {
        if (start..=end).contains(&row.line) {
            return false;
        }
        if row.line > end {
            row.line = (row.line as isize + delta) as usize;
        }
        true
    });
    Ok(())
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
        if !p.script_sources.is_empty() {
            return Err("台本ファイル管理中は台本全体の置換を使用できません".into());
        }
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
        let replacements = [
            derived(left, std::slice::from_ref(&s)),
            derived(right, std::slice::from_ref(&s)),
        ];
        if !s.source_id.is_empty() {
            sync_file_draft(p, std::slice::from_ref(&s), &replacements)?;
        }
        p.segments.splice(i..=i, replacements);
        p.archived_segments.push(s);
    } else if action == "merge" {
        if i + 1 >= p.segments.len() {
            return Err("後ろの発話がありません".into());
        }
        let sources = p.segments[i..i + 2].to_vec();
        let file_ids: Vec<_> = sources
            .iter()
            .map(|source| source.source_id.as_str())
            .collect();
        if file_ids.iter().any(|id| !id.is_empty())
            && (file_ids.iter().any(|id| id.is_empty()) || file_ids[0] != file_ids[1])
        {
            return Err("異なる台本ファイルの発話は結合できません".into());
        }
        let text = format!("{}{}", sources[0].text, sources[1].text);
        let replacements = [derived(text, &sources)];
        if !sources[0].source_id.is_empty() {
            sync_file_draft(p, &sources, &replacements)?;
        }
        p.segments.splice(i..i + 2, replacements);
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
        assert!(edit_script(
            &mut p,
            "revise_script",
            &json!({"text":"変更","generation":1})
        )
        .is_err());
        edit_script(
            &mut p,
            "revise_script",
            &json!({"text":"変更","generation":0}),
        )
        .unwrap();
        assert_eq!(p.archived_segments[0].text, "元");
        assert!(p.segments[0].selected.is_none());
    }

    fn file_segment(text: &str, source_id: &str, source_line: usize) -> Segment {
        let mut segment = Segment::new(text.into());
        segment.source_id = source_id.into();
        segment.source_line = Some(source_line);
        segment.takes.push(Take {
            script_text: text.into(),
            trim_start: 0.0,
            trim_end: 0.0,
            id: id(),
            file: "take.wav".into(),
            hash: "hash".into(),
            status: "ready".into(),
            duration: 1.0,
            peak: 0.0,
            warning: String::new(),
            created: 0,
            endpoint: String::new(),
        });
        segment
    }

    #[test]
    fn file_mode_split_and_merge_keep_source_without_copying_takes() {
        let mut p = Project::new("試験".into());
        p.segments.push(file_segment("甲乙", "source-a", 0));
        p.segments.push(file_segment("丙", "source-b", 0));
        p.script_sources.push(ScriptSource {
            id: "source-a".into(),
            name: "a.md".into(),
            kind: "markdown".into(),
            confirmed_text: "甲乙".into(),
            draft_text: "甲乙".into(),
            lines: crate::markdown::parse("甲乙").unwrap(),
            draft_lines: serde_json::to_value(crate::markdown::parse("甲乙").unwrap())
                .unwrap()
                .as_array()
                .cloned()
                .unwrap(),
            blank_rows: vec![],
            approved_text: None,
        });
        p.script_sources.push(ScriptSource {
            id: "source-b".into(),
            name: "b.md".into(),
            kind: "markdown".into(),
            confirmed_text: "丙".into(),
            draft_text: "丙".into(),
            lines: crate::markdown::parse("丙").unwrap(),
            draft_lines: serde_json::to_value(crate::markdown::parse("丙").unwrap())
                .unwrap()
                .as_array()
                .cloned()
                .unwrap(),
            blank_rows: vec![],
            approved_text: None,
        });
        let sid = p.segments[0].id.clone();
        edit_script(&mut p, "split", &json!({"segment":sid,"position":1})).unwrap();
        assert_eq!(
            p.segments[..2]
                .iter()
                .map(|s| s.source_id.as_str())
                .collect::<Vec<_>>(),
            vec!["source-a", "source-a"]
        );
        assert!(p.segments[..2]
            .iter()
            .all(|s| s.source_line.is_none() && s.takes.is_empty()));
        assert_eq!(p.segments[0].source_text, "甲");
        assert_eq!(p.segments[1].source_text, "乙");
        assert_eq!(p.archived_segments[0].source_line, Some(0));
        assert_eq!(p.script_sources[1].draft_text, "丙");
        let split_ids: Vec<_> = p.segments[..2]
            .iter()
            .map(|segment| segment.id.clone())
            .collect();
        let source = &p.script_sources[0];
        let payload = json!({
            "base_revision": p.script_revision,
            "sources": [{
                "id": source.id,
                "name": source.name,
                "kind": source.kind,
                "text": source.draft_text,
                "lines": source.draft_lines,
                "approved_text": "甲\n乙"
            }, {
                "id": p.script_sources[1].id,
                "name": p.script_sources[1].name,
                "kind": p.script_sources[1].kind,
                "text": p.script_sources[1].draft_text,
                "lines": p.script_sources[1].draft_lines,
                "approved_text": "丙"
            }]
        });
        crate::script_sources::commit(&mut p, &payload).unwrap();
        assert_eq!(
            p.segments[..2]
                .iter()
                .map(|segment| segment.id.clone())
                .collect::<Vec<_>>(),
            split_ids
        );
        assert_eq!(p.segments[0].source_line, Some(0));
        assert_eq!(p.segments[1].source_line, Some(1));
        let sid = p.segments[0].id.clone();
        edit_script(&mut p, "merge", &json!({"segment":sid})).unwrap();
        assert_eq!(p.segments[0].source_id, "source-a");
        assert!(p.segments[0].source_line.is_none());
        assert!(p.segments[0].takes.is_empty());
        assert_eq!(p.script_sources[0].draft_text, "甲乙");
    }

    #[test]
    fn file_mode_revision_keeps_source_and_archives_audio_bearing_original() {
        let mut p = Project::new("試験".into());
        p.segments.push(file_segment("元", "source-a", 2));
        p.script_sources.push(ScriptSource {
            id: "source-a".into(),
            name: "a.md".into(),
            kind: "markdown".into(),
            confirmed_text: "元".into(),
            draft_text: "元".into(),
            lines: crate::markdown::parse("元").unwrap(),
            draft_lines: serde_json::to_value(crate::markdown::parse("元").unwrap())
                .unwrap()
                .as_array()
                .cloned()
                .unwrap(),
            blank_rows: vec![],
            approved_text: None,
        });
        assert!(edit_script(
            &mut p,
            "revise_script",
            &json!({"text":"変更","generation":0}),
        )
        .is_err());
        assert_eq!(p.segments[0].source_id, "source-a");
        assert_eq!(p.segments[0].takes.len(), 1);
    }

    #[test]
    fn file_mode_merge_rejects_different_source_files() {
        let mut p = Project::new("試験".into());
        p.segments.push(file_segment("甲", "source-a", 0));
        p.segments.push(file_segment("乙", "source-b", 0));
        let first = p.segments[0].id.clone();
        assert!(edit_script(&mut p, "merge", &json!({"segment":first})).is_err());
        assert_eq!(p.segments.len(), 2);
        assert!(p.archived_segments.is_empty());
    }
}
