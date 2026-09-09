//! 複数ファイル台本の下書きと確定を扱う、UI非依存の保存層。
use crate::{domain::{id, BlankScriptRow, Project, Result, ScriptSource, Segment}, markdown::{self, ScriptLine}};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

const MAX_SOURCE_BYTES: usize = 2_000_000;
const MAX_SEGMENTS: usize = 1_000;

/// `script_lines` は旧画面向けの派生値であり、複数ファイルの正本ではない。
/// 合計2MBを超えても、個別ファイルが有効なら確定を拒否しない。
fn legacy_script_lines(sources: &[ScriptSource]) -> Result<Vec<ScriptLine>> {
    let text = sources.iter().map(|source| source.confirmed_text.as_str()).collect::<Vec<_>>().join("\n\n");
    if text.len() > MAX_SOURCE_BYTES { Ok(vec![]) } else { markdown::parse(&text) }
}

pub fn snapshot(project: &Project) -> Value {
    json!({"script_revision": project.script_revision, "sources": project.script_sources})
}

fn sources_payload(value: &Value) -> Result<&Vec<Value>> {
    value["sources"].as_array().ok_or("台本ファイル一覧が必要です".into())
}
fn base_matches(project: &Project, value: &Value) -> Result<()> {
    let base = value["base_revision"].as_u64().ok_or("台本の基準revisionが必要です")?;
    if base != project.script_revision { return Err("台本が別の保存で更新されています。開き直して比較してください".into()); }
    if let Some(generation) = value["base_generation"].as_u64() {
        if generation != project.generation { return Err("プロジェクトが別の保存で更新されています。下書きを比較し直してください".into()); }
    }
    Ok(())
}
fn source_text(value: &Value) -> Result<String> {
    let text = value["text"].as_str().ok_or("台本文が必要です")?.to_owned();
    if text.len() > MAX_SOURCE_BYTES { return Err("台本ファイルは2MB以内にしてください".into()); }
    Ok(text)
}
fn source_name(value: &Value) -> Result<String> {
    let name = value["name"].as_str().ok_or("台本ファイル名が必要です")?.trim().to_owned();
    if name.is_empty() { return Err("台本ファイル名が空です".into()); }
    Ok(name)
}
fn choices(value: &Value) -> Result<Vec<Value>> {
    value["lines"].as_array().cloned().ok_or("行の選択が必要です".into())
}
fn source_kind(value: &Value) -> String { value["kind"].as_str().unwrap_or("markdown").to_owned() }

/// UIの編集中状態だけを更新する。確定済み本文・発話・revisionは変更しない。
pub fn update_draft(project: &mut Project, value: &Value) -> Result<()> {
    base_matches(project, value)?;
    let input = sources_payload(value)?;
    let mut names = HashSet::new();
    let mut ids = HashSet::new();
    let current: HashMap<_, _> = project.script_sources.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut result = Vec::with_capacity(input.len());
    for item in input {
        let name = source_name(item)?;
        if !names.insert(name.clone()) { return Err("同名の台本ファイルは同時に扱えません".into()); }
        let item_id = item["id"].as_str().unwrap_or("");
        let existing = current.get(item_id).copied();
        if !item_id.is_empty() && existing.is_none() { uuid::Uuid::parse_str(item_id).map_err(|_| "台本ファイルIDの形式が不正です")?; }
        if let Some(old) = existing { if old.name != name { return Err("既存の台本ファイル名は変更できません".into()); } }
        let source_id = if item_id.is_empty() { id() } else { item_id.to_owned() };
        if !ids.insert(source_id.clone()) { return Err("台本ファイルIDが重複しています".into()); }
        let text = source_text(item)?;
        let draft_lines = choices(item)?;
        result.push(ScriptSource {
            id: source_id, name, kind: source_kind(item),
            confirmed_text: existing.map(|s| s.confirmed_text.clone()).unwrap_or_default(),
            lines: existing.map(|s| s.lines.clone()).unwrap_or_default(),
            draft_text: text, draft_lines,
            blank_rows: item["blank_rows"].as_array().map(|rows| serde_json::from_value(Value::Array(rows.clone())).map_err(|e| e.to_string())).transpose()?.unwrap_or_else(|| existing.map(|s| s.blank_rows.clone()).unwrap_or_default()),
            approved_text: item["approved_text"].as_str().map(str::to_owned),
        });
    }
    if project.script_sources.iter().any(|source| !ids.contains(&source.id)) { return Err("既存の台本ファイルは下書きから削除できません".into()); }
    project.script_sources = result;
    Ok(())
}

pub fn insert_empty(project: &mut Project, value: &Value) -> Result<()> {
    base_matches(project, value)?;
    if let Some(source_id) = value["source_id"].as_str() {
        if !project.script_sources.iter().any(|s| s.id == source_id) { return Err("台本ファイルが見つかりません".into()); }
        let insert_at = match value["after_segment"].as_str() {
            Some(after) => project.segments.iter().position(|s| s.id == after && s.source_id == source_id)
                .ok_or("挿入位置の発話が見つかりません")? + 1,
            None => project.segments.iter().rposition(|s| s.source_id == source_id).map(|n| n + 1).unwrap_or(project.segments.len()),
        };
        if project.segments.len() >= MAX_SEGMENTS { return Err("読み上げる行は1000行以内で選んでください".into()); }
        let mut row = Segment::new(String::new());
        row.source_id = source_id.to_owned();
        let source = project.script_sources.iter().find(|s| s.id == source_id).unwrap();
        let line = match value["after_segment"].as_str().and_then(|after| project.segments.iter().find(|s| s.id == after)) {
            Some(after) => source.draft_lines.iter().find(|row| row["previous_id"].as_str() == Some(after.id.as_str())).and_then(|row|row["line"].as_u64()).map(|line|line as usize + 1)
                .or_else(|| source.blank_rows.iter().find(|row| row.segment_id == after.id).map(|row| row.line + 1))
                .or_else(|| if source.draft_text == source.confirmed_text { after.source_line.map(|line| line + 1) } else { None })
                .ok_or("未確定の下書きでは空録音行の挿入位置を特定できません")?,
            None => source.draft_text.split('\n').count(),
        };
        // 確定済み行番号は動かさない。空行の下書き上の位置だけ blank_rows が持つ。
        row.source_line = None;
        let segment_id = row.id.clone();
        project.segments.insert(insert_at, row);
        let source = project.script_sources.iter_mut().find(|s| s.id == source_id).unwrap();
        let mut physical: Vec<String> = source.draft_text.split('\n').map(str::to_owned).collect();
        physical.insert(line.min(physical.len()), String::new());
        source.draft_text = physical.join("\n");
        for blank in &mut source.blank_rows { if blank.line >= line { blank.line += 1; } }
        source.blank_rows.push(BlankScriptRow { segment_id, line });
        if source.draft_lines.is_empty() { source.draft_lines = serde_json::to_value(markdown::parse(&source.confirmed_text)?).map_err(|e| e.to_string())?.as_array().cloned().unwrap_or_default(); }
        for choice in &mut source.draft_lines {
            if let Some(old_line) = choice["line"].as_u64().filter(|old_line| *old_line as usize >= line) { choice["line"] = Value::from(old_line + 1); }
        }
        source.draft_lines.push(json!({"line":line,"enabled":true,"previous_id":source.blank_rows.last().unwrap().segment_id,"keep":true,"blank_segment_id":source.blank_rows.last().unwrap().segment_id}));
        return Ok(());
    }
    let name = value["name"].as_str().unwrap_or("新しい台本").trim().to_owned();
    if name.is_empty() || project.script_sources.iter().any(|s| s.name == name) { return Err("台本ファイル名が空か重複しています".into()); }
    project.script_sources.push(ScriptSource { id: id(), name, kind: source_kind(value), confirmed_text: String::new(), draft_text: String::new(), lines: vec![], draft_lines: vec![], blank_rows: vec![], approved_text: None });
    Ok(())
}

/// ASRの空白行補完用。想定した確定revision以外には下書きを書かない。
pub fn update_blank_draft(project: &mut Project, segment_id: &str, text: &str, expected_segment_revision: u64) -> Result<()> {
    let segment = project.segments.iter_mut().find(|s| s.id == segment_id).ok_or("発話が見つかりません")?;
    if segment.revision != expected_segment_revision || !segment.text.trim().is_empty() { return Err("発話が更新されています。認識結果を比較し直してください".into()); }
    let source_id = segment.source_id.clone();
    let source = project.script_sources.iter_mut().find(|s| s.id == source_id).ok_or("台本ファイルが見つかりません")?;
    let line = source.blank_rows.iter().find(|row| row.segment_id == segment_id).map(|row| row.line).ok_or("空発話の下書き情報が見つかりません")?;
    let mut physical: Vec<String> = source.draft_text.split('\n').map(str::to_owned).collect();
    if line >= physical.len() { return Err("空発話の下書き位置が不正です".into()); }
    if !physical[line].trim().is_empty() { return Err("空発話の下書きが手動で更新されています".into()); }
    physical[line] = text.to_owned();
    let draft = physical.join("\n");
    if draft.len() > MAX_SOURCE_BYTES { return Err("台本ファイルは2MB以内にしてください".into()); }
    source.draft_text = draft;
    if let Some(choice) = source.draft_lines.iter_mut().find(|choice| choice["line"].as_u64() == Some(line as u64) && choice["blank_segment_id"].as_str() == Some(segment_id)) {
        choice["previous_id"] = Value::String(segment_id.to_owned());
        choice["keep"] = Value::Bool(true);
    } else { source.draft_lines.push(json!({"line":line,"enabled":true,"previous_id":segment_id,"keep":true,"blank_segment_id":segment_id})); }
    source.approved_text = None;
    segment.text = text.to_owned();
    // source_line は次回の台本確定で明示対応した行番号へだけ更新する。
    segment.revision = segment.revision.saturating_add(1);
    Ok(())
}

/// 元ファイル名を失った単一の「既存台本」を、利用者が確認したファイル境界へ対応付ける。
/// 本文を推測して分割せず、全発話の明示対応と厳密な連結一致を必須にする。
pub fn rebind_legacy(project: &mut Project, value: &Value) -> Result<()> {
    base_matches(project, value)?;
    if value["base_generation"].as_u64().is_none() { return Err("既存台本対応付けにはプロジェクトの基準generationが必要です".into()); }
    let legacy_id = value["legacy_source_id"].as_str().ok_or("既存台本のsource IDが必要です")?;
    let legacy_index = project.script_sources.iter().position(|source| source.id == legacy_id && source.name == "既存台本")
        .ok_or("既存台本が見つかりません")?;
    let legacy = project.script_sources[legacy_index].clone();
    if legacy.confirmed_text != legacy.draft_text || !legacy.blank_rows.is_empty() {
        return Err("未確定の下書きまたは空録音行がある既存台本は対応付けできません".into());
    }
    let input = sources_payload(value)?;
    if input.is_empty() { return Err("対応付ける元ファイルが必要です".into()); }
    let existing_names: HashSet<_> = project.script_sources.iter().filter(|source| source.id != legacy_id).map(|source| source.name.as_str()).collect();
    let mut names = HashSet::new();
    let mut prepared = Vec::with_capacity(input.len());
    for item in input {
        let name = source_name(item)?;
        if !names.insert(name.clone()) || existing_names.contains(name.as_str()) { return Err("元ファイル名が重複しています".into()); }
        let text = source_text(item)?;
        let lines = markdown::parse(&text)?;
        let choices = choices(item)?;
        prepared.push((name, source_kind(item), text, lines, choices));
    }
    if prepared.iter().map(|(_,_,text,_,_)|text.as_str()).collect::<Vec<_>>().join("\n\n") != legacy.confirmed_text {
        return Err("指定した元ファイル本文の連結が既存台本と完全一致しません".into());
    }
    let old: HashMap<_, _> = project.segments.iter().chain(project.archived_segments.iter())
        .filter(|segment| segment.source_id == legacy_id).map(|segment| (segment.id.as_str(), segment)).collect();
    let mut mapped = HashSet::new();
    let mut locations: HashMap<String, (usize, usize, ScriptLine)> = HashMap::new();
    for (source_index, (_, _, _, lines, selected)) in prepared.iter().enumerate() {
        for selected_line in selected {
            let Some(previous_id) = selected_line["previous_id"].as_str() else { continue; };
            let segment = old.get(previous_id).copied().or_else(|| old.values().copied().find(|segment| segment.line_id == previous_id))
                .ok_or("既存台本にない発話、または別台本の発話が指定されています")?;
            if !mapped.insert(segment.id.clone()) { return Err("同じ既存発話を複数の元ファイルへ対応付けできません".into()); }
            let line_number = selected_line["line"].as_u64().ok_or("対応先の行番号が必要です")? as usize;
            let line = lines.iter().find(|line| line.line == line_number).ok_or("対応先の行が見つかりません")?;
            if line.text != segment.source_text { return Err("対応先の行本文が既存元原稿と一致しません".into()); }
            locations.insert(segment.id.clone(), (source_index, line_number, line.clone()));
        }
    }
    if mapped.len() != old.len() { return Err("全ての現役・アーカイブ発話を明示的に対応付けてください".into()); }
    let mut staged = project.clone();
    let mut rebound = Vec::new();
    for (index, (name, kind, text, lines, selected)) in prepared.into_iter().enumerate() {
        rebound.push(ScriptSource { id: if index == 0 { legacy.id.clone() } else { id() }, name, kind,
            confirmed_text:text.clone(), draft_text:text, lines, draft_lines:selected, blank_rows:vec![], approved_text:None });
    }
    let source_ids: Vec<_> = rebound.iter().map(|source| source.id.clone()).collect();
    for segment in staged.segments.iter_mut().chain(staged.archived_segments.iter_mut()) {
        if let Some((source_index, line_number, line)) = locations.get(&segment.id) {
            segment.source_id = source_ids[*source_index].clone();
            segment.source_line = Some(*line_number);
            segment.kind = line.kind;
            segment.hierarchy = line.hierarchy.clone();
        }
    }
    staged.script_sources.splice(legacy_index..=legacy_index, rebound);
    staged.source_markdown = staged.script_sources.iter().map(|source|source.confirmed_text.as_str()).collect::<Vec<_>>().join("\n\n");
    staged.script_lines = legacy_script_lines(&staged.script_sources)?;
    // 本文の合計値は不変であり、明示対応は基準本文を進めない。
    *project = staged;
    Ok(())
}

fn checked_rows(item: &Value, old: &ScriptSource) -> Result<(String, Vec<ScriptLine>, Vec<Value>)> {
    let text = source_text(item)?;
    let mut rows = markdown::parse(&text)?;
    let selections = choices(item)?;
    let mut previous_ids = HashSet::new();
    let mut previous_lines = HashSet::new();
    for choice in &selections {
        if let Some(previous_id) = choice["previous_id"].as_str() {
            if !previous_ids.insert(previous_id.to_owned()) { return Err("同じ元行を複数回対応付けできません".into()); }
        }
        if let Some(previous_line) = choice["previous_line"].as_u64() {
            if !previous_lines.insert(previous_line) || !old.lines.iter().any(|line| line.line == previous_line as usize) { return Err("対応する元原稿行がありません".into()); }
        }
    }
    for row in &mut rows {
        if let Some(choice) = row_choice(&selections, row.line) {
            row.enabled = choice["enabled"].as_bool().unwrap_or(row.enabled);
        }
    }
    Ok((text, rows, selections))
}
fn row_choice<'a>(choices: &'a [Value], line: usize) -> Option<&'a Value> {
    choices.iter().find(|choice| choice["line"].as_u64() == Some(line as u64))
}

/// 全ファイルを一つの保存世代として確定する。失敗時は project を変更しない。
pub fn commit(project: &mut Project, value: &Value) -> Result<()> {
    base_matches(project, value)?;
    let input = sources_payload(value)?;
    if input.len() != project.script_sources.len() { return Err("確定には全台本ファイルを含めてください".into()); }
    let mut staged = project.clone();
    update_draft(&mut staged, value)?;
    let mut by_id: HashMap<_, _> = project.script_sources.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut parsed = Vec::new();
    for item in input {
        let source_id = item["id"].as_str().ok_or("確定する台本ファイルIDが必要です")?;
        let old = by_id.remove(source_id).ok_or("台本ファイルIDが見つかりません")?;
        let (text, rows, selections) = checked_rows(item, old)?;
        if text != old.confirmed_text && item["approved_text"].as_str() != Some(text.as_str()) {
            return Err(format!("{} の変更が未承認です", old.name));
        }
        parsed.push((source_id.to_owned(), text, rows, selections));
    }
    if !by_id.is_empty() { return Err("確定には全台本ファイルを含めてください".into()); }

    let old_active = std::mem::take(&mut staged.segments);
    let old_archived = staged.archived_segments.clone();
    let mut active = Vec::new();
    let mut used = HashSet::new();
    for (source_id, _text, rows, selections) in &parsed {
        for row in rows.iter().filter(|row| row.enabled && !row.text.is_empty()) {
            let choice = row_choice(selections, row.line);
            let requested_id = choice.and_then(|c| c["previous_id"].as_str());
            let has_previous_line = choice.is_some_and(|c| c.get("previous_line").is_some());
            let requested_line = choice.and_then(|c| c["previous_line"].as_u64()).map(|n| n as usize);
            let candidate = if let Some(previous_id) = requested_id {
                old_active.iter().chain(old_archived.iter()).find(|segment| segment.source_id == source_id.as_str()
                    && !used.contains(&segment.id) && (previous_id == segment.id || previous_id == segment.line_id))
            } else if !has_previous_line {
                // 自動維持は現役行だけに限定する。アーカイブは明示対応でのみ復帰する。
                old_active.iter().find(|segment| segment.source_id == source_id.as_str() && !used.contains(&segment.id)
                    && segment.source_line == Some(row.line) && segment.source_text == row.text)
            } else if let Some(previous_line) = requested_line {
                old_active.iter().find(|segment| segment.source_id == source_id.as_str() && !used.contains(&segment.id)
                    && segment.source_line == Some(previous_line) && segment.source_text == row.text)
            } else { None };
            if requested_id.is_some() && candidate.is_none() {
                return Err("指定した維持元の発話が見つからないか、別の台本ファイルです".into());
            }
            if let Some(found) = candidate {
                if found.source_text != row.text && choice.and_then(|c| c["keep"].as_bool()) != Some(true) {
                    return Err("本文が変更された維持行は keep:true の確認が必要です".into());
                }
            }
            let mut segment = candidate.cloned().unwrap_or_else(|| Segment::new(row.text.clone()));
            if let Some(found) = candidate { used.insert(found.id.clone()); }
            segment.source_id = source_id.clone();
            segment.source_line = Some(row.line);
            segment.source_text = row.text.clone();
            segment.kind = row.kind;
            segment.hierarchy = row.hierarchy.clone();
            active.push(segment);
        }
        // 明示的な空録音行は本文が空でも確定候補から除外せず、位置順に保持する。
        if let Some(source) = staged.script_sources.iter().find(|s| s.id == source_id.as_str()) {
            for blank in &source.blank_rows {
                if active.iter().any(|s| s.id == blank.segment_id) { continue; }
                if let Some(segment) = old_active.iter().chain(old_archived.iter()).find(|s| s.id == blank.segment_id && s.source_id == source_id.as_str()) {
                    if !segment.text.trim().is_empty() || source.draft_text.split('\n').nth(blank.line).is_some_and(|text| !text.trim().is_empty()) { continue; }
                    used.insert(segment.id.clone());
                    let blank_segment = segment.clone();
                    let position = active.iter().rposition(|s| s.source_id == source_id.as_str() && s.source_line.is_some_and(|line| line <= blank.line)).map(|n| n + 1).unwrap_or(active.len());
                    active.insert(position, blank_segment);
                }
            }
        }
    }
    if active.len() > MAX_SEGMENTS { return Err("読み上げる行は1000行以内で選んでください".into()); }
    let mut archived: Vec<Segment> = old_archived.into_iter().filter(|s| !used.contains(&s.id)).collect();
    archived.extend(old_active.into_iter().filter(|s| !used.contains(&s.id)));
    let mut archive_ids = HashSet::new();
    archived.retain(|s| archive_ids.insert(s.id.clone()));
    staged.segments = active;
    staged.archived_segments = archived;
    let still_blank: HashSet<String> = staged.segments.iter().filter(|segment| segment.text.trim().is_empty()).map(|segment| segment.id.clone()).collect();
    for ((source_id, text, rows, selections), source) in parsed.into_iter().zip(staged.script_sources.iter_mut()) {
        if source.id != source_id { return Err("台本ファイルの順序またはIDが不正です".into()); }
        source.confirmed_text = text.clone(); source.draft_text = text;
        source.lines = rows; source.draft_lines = selections; source.blank_rows.retain(|row| still_blank.contains(&row.segment_id)); source.approved_text = None;
    }
    staged.source_markdown = staged.script_sources.iter().map(|s| s.confirmed_text.as_str()).collect::<Vec<_>>().join("\n\n");
    staged.script_lines = legacy_script_lines(&staged.script_sources)?;
    staged.script_revision = staged.script_revision.saturating_add(1);
    *project = staged;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Take;
    use serde_json::json;
    fn source(name: &str, text: &str) -> ScriptSource {
        ScriptSource { id:id(), name:name.into(), kind:"markdown".into(), confirmed_text:text.into(), draft_text:text.into(), lines:markdown::parse(text).unwrap(), draft_lines:vec![], blank_rows:vec![], approved_text:None }
    }
    fn payload(p: &Project, text: &str, approved: bool) -> Value {
        let s = &p.script_sources[0];
        json!({"base_revision":p.script_revision,"sources":[{"id":s.id,"name":s.name,"kind":s.kind,"text":text,"lines":markdown::parse(text).unwrap(),"approved_text":if approved { Value::String(text.into()) } else { Value::Null }}]})
    }
    #[test]
    fn legacy_project_migrates_to_a_draftable_source() {
        let mut p = Project::new("x".into()); p.source_markdown = "甲".into(); p.script_lines = markdown::parse("甲").unwrap();
        // Store performs v4 migration; this test covers draft/commit after that shape exists.
        p.script_sources.push(ScriptSource { id:id(), name:"既存台本".into(), kind:"markdown".into(), confirmed_text:"甲".into(), draft_text:"甲".into(), lines:markdown::parse("甲").unwrap(), draft_lines:vec![], blank_rows:vec![], approved_text:None }); p.script_revision=1;
        let rejected = payload(&p, "乙", false);
        assert!(commit(&mut p, &rejected).is_err());
        let v=payload(&p,"乙",true); commit(&mut p,&v).unwrap(); assert_eq!(p.script_sources[0].confirmed_text,"乙"); assert_eq!(p.script_revision,2);
    }
    #[test]
    fn draft_is_independent_and_conflicts_are_rejected() {
        let mut p=Project::new("x".into()); insert_empty(&mut p,&json!({"name":"a.md","base_revision":0})).unwrap();
        let s=p.script_sources[0].clone(); let v=json!({"base_revision":0,"sources":[{"id":s.id,"name":"a.md","text":"空\n本文","lines":markdown::parse("空\n本文").unwrap()}]});
        update_draft(&mut p,&v).unwrap(); assert!(p.segments.is_empty()); assert_eq!(p.script_sources[0].name,"a.md");
        assert!(update_draft(&mut p,&json!({"base_revision":99,"sources":[]})).is_err());
    }
    #[test]
    fn blank_recording_row_fills_draft_without_moving_existing_recordings() {
        let mut p = Project::new("x".into());
        let source_id = id();
        p.script_sources.push(ScriptSource { id:source_id.clone(), name:"a.md".into(), kind:"markdown".into(), confirmed_text:"A\nB".into(), draft_text:"A\nB".into(), lines:markdown::parse("A\nB").unwrap(), draft_lines:vec![], blank_rows:vec![], approved_text:None });
        p.script_revision = 1;
        let mut a=Segment::new("A".into()); a.source_id=source_id.clone(); a.source_line=Some(0); a.source_text="A".into();
        let mut b=Segment::new("B".into()); b.source_id=source_id.clone(); b.source_line=Some(1); b.source_text="B".into();
        let aid=a.id.clone(); let bid=b.id.clone(); p.segments=vec![a,b];
        insert_empty(&mut p,&json!({"base_revision":1,"source_id":source_id,"after_segment":aid})).unwrap();
        let blank=p.segments[1].id.clone();
        update_blank_draft(&mut p,&blank,"空録音",1).unwrap();
        let source=&p.script_sources[0];
        let lines=markdown::parse(&source.draft_text).unwrap();
        let v=json!({"base_revision":1,"sources":[{"id":source.id,"name":source.name,"kind":"markdown","text":source.draft_text,"lines":[
            {"line":0,"enabled":true,"previous_id":aid}, {"line":1,"enabled":true,"previous_id":blank,"keep":true}, {"line":2,"enabled":true,"previous_id":bid}],"approved_text":source.draft_text}]});
        commit(&mut p,&v).unwrap();
        assert_eq!(p.segments.iter().map(|s|s.id.clone()).collect::<Vec<_>>(),vec![aid,blank,bid]);
        assert_eq!(p.segments[0].source_line,Some(0)); assert_eq!(p.segments[2].source_line,Some(2));
        assert!(lines[1].enabled);
    }
    #[test]
    fn multiple_blank_rows_shift_metadata_and_fill_out_of_order() {
        let mut p=Project::new("x".into()); let s=source("a.md","A\nB"); let source_id=s.id.clone(); p.script_sources.push(s); p.script_revision=1;
        let mut a=Segment::new("A".into()); a.source_id=source_id.clone(); a.source_line=Some(0); a.source_text="A".into(); let aid=a.id.clone(); p.segments.push(a);
        insert_empty(&mut p,&json!({"base_revision":1,"source_id":source_id,"after_segment":aid})).unwrap(); let first=p.segments[1].id.clone();
        let source_id=p.script_sources[0].id.clone();
        insert_empty(&mut p,&json!({"base_revision":1,"source_id":source_id,"after_segment":first})).unwrap(); let second=p.segments[2].id.clone();
        update_blank_draft(&mut p,&second,"二",1).unwrap(); update_blank_draft(&mut p,&first,"一",1).unwrap();
        assert_eq!(p.script_sources[0].draft_text,"A\n一\n二\nB");
        assert_eq!(p.script_sources[0].blank_rows.iter().map(|row|row.line).collect::<Vec<_>>(),vec![1,2]);
    }
    #[test]
    fn draft_requires_all_existing_sources_and_allows_a_client_generated_uuid() {
        let mut p=Project::new("x".into()); p.script_sources=vec![source("a.md","A")];
        let missing=json!({"base_revision":0,"sources":[{"id":id(),"name":"b.md","text":"B","lines":markdown::parse("B").unwrap()}]});
        assert!(update_draft(&mut p,&missing).is_err());
        let new_id=id(); let empty=Project::new("x".into()); let mut p=empty;
        let add=json!({"base_revision":0,"sources":[{"id":new_id,"name":"b.md","text":"B","lines":markdown::parse("B").unwrap()}]});
        update_draft(&mut p,&add).unwrap(); assert_eq!(p.script_sources[0].id,new_id);
    }
    #[test]
    fn duplicate_names_and_ids_are_rejected() {
        let mut p=Project::new("x".into()); let one=id();
        let duplicate=json!({"base_revision":0,"sources":[
            {"id":one,"name":"same.md","text":"A","lines":[]},
            {"id":one,"name":"same.md","text":"B","lines":[]}]});
        assert!(update_draft(&mut p,&duplicate).is_err());
    }
    #[test]
    fn disabled_row_archives_and_explicit_reenable_restores_its_id() {
        let mut p=Project::new("x".into()); let s=source("a.md","A"); let sid=s.id.clone(); p.script_sources.push(s); p.script_revision=1;
        let mut segment=Segment::new("A".into()); segment.source_id=sid.clone(); segment.source_line=Some(0); segment.source_text="A".into(); let original=segment.id.clone(); p.segments.push(segment);
        let off=json!({"base_revision":1,"sources":[{"id":sid,"name":"a.md","text":"A","lines":[{"line":0,"enabled":false,"previous_id":original}]}]});
        commit(&mut p,&off).unwrap(); assert!(p.segments.is_empty()); assert_eq!(p.archived_segments[0].id,original);
        let sid=p.script_sources[0].id.clone(); let on=json!({"base_revision":2,"sources":[{"id":sid,"name":"a.md","text":"A","lines":[{"line":0,"enabled":true,"previous_id":original}]}]});
        commit(&mut p,&on).unwrap(); assert_eq!(p.segments[0].id,original); assert!(p.archived_segments.is_empty());
    }
    #[test]
    fn explicit_null_previous_line_creates_a_new_row_and_unknown_or_cross_file_id_fails() {
        let mut p=Project::new("x".into()); let first=source("a.md","A"); let second=source("b.md","A"); let a=first.id.clone(); let b=second.id.clone(); p.script_sources=vec![first,second]; p.script_revision=1;
        let mut old=Segment::new("A".into()); old.source_id=a.clone(); old.source_line=Some(0); old.source_text="A".into(); let old_id=old.id.clone(); p.segments.push(old);
        let null=json!({"base_revision":1,"sources":[{"id":a,"name":"a.md","text":"A","lines":[{"line":0,"enabled":true,"previous_line":null}]},{"id":b,"name":"b.md","text":"A","lines":[{"line":0,"enabled":false}]}]});
        commit(&mut p,&null).unwrap(); assert_ne!(p.segments[0].id,old_id);
        let a=p.script_sources[0].id.clone(); let b=p.script_sources[1].id.clone(); let current=p.segments[0].id.clone();
        let cross=json!({"base_revision":2,"sources":[{"id":a,"name":"a.md","text":"A","lines":[{"line":0,"enabled":true,"previous_id":current}]},{"id":b,"name":"b.md","text":"A","lines":[{"line":0,"enabled":true,"previous_id":old_id}]}]});
        assert!(commit(&mut p,&cross).is_err());
    }
    #[test]
    fn changed_kept_line_requires_keep_and_same_text_sources_stay_distinct() {
        let mut p=Project::new("x".into()); let first=source("a.md","同文"); let second=source("b.md","同文"); let a=first.id.clone(); let b=second.id.clone(); p.script_sources=vec![first,second]; p.script_revision=1;
        let mut left=Segment::new("同文".into()); left.source_id=a.clone(); left.source_line=Some(0); left.source_text="同文".into(); let lid=left.id.clone();
        let mut right=Segment::new("同文".into()); right.source_id=b.clone(); right.source_line=Some(0); right.source_text="同文".into(); let rid=right.id.clone(); p.segments=vec![left,right];
        let no_keep=json!({"base_revision":1,"sources":[{"id":a,"name":"a.md","text":"変更","lines":[{"line":0,"enabled":true,"previous_id":lid}],"approved_text":"変更"},{"id":b,"name":"b.md","text":"同文","lines":[{"line":0,"enabled":true,"previous_id":rid}]}]});
        assert!(commit(&mut p,&no_keep).is_err());
        let a=p.script_sources[0].id.clone(); let b=p.script_sources[1].id.clone();
        let keep=json!({"base_revision":1,"sources":[{"id":a,"name":"a.md","text":"変更","lines":[{"line":0,"enabled":true,"previous_id":lid,"keep":true}],"approved_text":"変更"},{"id":b,"name":"b.md","text":"同文","lines":[{"line":0,"enabled":true,"previous_id":rid}]}]});
        commit(&mut p,&keep).unwrap(); assert_eq!(p.segments[0].id,lid); assert_eq!(p.segments[1].id,rid);
    }
    #[test]
    fn legacy_rebind_requires_exact_confirmed_files_and_preserves_segments() {
        let mut p=Project::new("x".into()); let legacy=source("既存台本","A\n\nB"); let legacy_id=legacy.id.clone(); p.script_sources.push(legacy); p.script_revision=1;
        let mut active=Segment::new("A".into()); active.source_id=legacy_id.clone(); active.source_line=Some(0); active.source_text="A".into(); active.text="えー".into(); active.takes.push(Take{script_text:String::new(),trim_start:0.,trim_end:1.,id:id(),file:"takes/a.wav".into(),hash:String::new(),status:"ready".into(),duration:1.,peak:0.,warning:String::new(),created:0,endpoint:String::new()});let active_id=active.id.clone();
        let mut archived=Segment::new("B".into()); archived.source_id=legacy_id.clone(); archived.source_line=Some(2); archived.source_text="B".into();let archived_id=archived.id.clone();p.segments.push(active);p.archived_segments.push(archived);
        let request=json!({"base_revision":1,"base_generation":0,"legacy_source_id":legacy_id,"sources":[
            {"name":"01.md","kind":"markdown","text":"A","lines":[{"line":0,"enabled":true,"previous_id":active_id}]},
            {"name":"02.md","kind":"markdown","text":"B","lines":[{"line":0,"enabled":true,"previous_id":archived_id}]}]});
        rebind_legacy(&mut p,&request).unwrap();
        assert_eq!(p.script_sources.len(),2); assert_eq!(p.script_sources[0].id,legacy_id); assert_eq!(p.script_sources[0].confirmed_text,"A"); assert_eq!(p.script_revision,1);
        assert_eq!(p.segments[0].id,active_id);assert_eq!(p.segments[0].text,"えー");assert_eq!(p.segments[0].takes.len(),1);assert_eq!(p.segments[0].source_id,legacy_id);assert_eq!(p.segments[0].source_line,Some(0));
        assert_eq!(p.archived_segments[0].id,archived_id);assert_eq!(p.archived_segments[0].source_id,p.script_sources[1].id);assert_eq!(p.archived_segments[0].source_line,Some(0));
    }
    #[test]
    fn legacy_rebind_rejects_draft_incomplete_and_nonmatching_content() {
        let mut p=Project::new("x".into()); let mut legacy=source("既存台本","A"); legacy.draft_text="編集".into(); let legacy_id=legacy.id.clone();p.script_sources.push(legacy);p.script_revision=1;
        let request=json!({"base_revision":1,"base_generation":0,"legacy_source_id":legacy_id,"sources":[{"name":"a.md","text":"A","lines":[]}]});
        assert!(rebind_legacy(&mut p,&request).is_err());
        p.script_sources[0].draft_text="A".into();let mut segment=Segment::new("A".into());segment.source_id=legacy_id.clone();segment.source_text="A".into();p.segments.push(segment);
        assert!(rebind_legacy(&mut p,&request).is_err());
        let wrong=json!({"base_revision":1,"base_generation":0,"legacy_source_id":legacy_id,"sources":[{"name":"a.md","text":"別","lines":[]}]});assert!(rebind_legacy(&mut p,&wrong).is_err());
    }
    #[test]
    fn multiple_valid_large_files_commit_without_a_combined_legacy_limit() {
        let first="A".repeat(1_100_000); let second="B".repeat(1_100_000);
        let mut p=Project::new("x".into());let one=source("01.md",&first);let two=source("02.md",&second);let one_id=one.id.clone();let two_id=two.id.clone();p.script_sources=vec![one,two];p.script_revision=1;
        let request=json!({"base_revision":1,"sources":[
            {"id":one_id,"name":"01.md","text":first,"lines":markdown::parse(&first).unwrap()},
            {"id":two_id,"name":"02.md","text":second,"lines":markdown::parse(&second).unwrap()}]});
        commit(&mut p,&request).unwrap();assert_eq!(p.segments.len(),2);assert!(p.script_lines.is_empty());assert!(p.source_markdown.len()>2_000_000);
    }
}
