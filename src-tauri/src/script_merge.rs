//! 内容に基づくマージ。候補は確認用で、確定前にProjectを変更しない。
use crate::{domain::*, markdown, storage::hash_bytes};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Keep,
    Replace,
    Add,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Choice {
    pub new_line: usize,
    pub old_id: Option<String>,
    pub decision: Decision,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub text: String,
    /// Noneは全体更新。範囲は基準スナップショットの発話位置で半開区間。
    pub range: Option<(usize, usize)>,
    pub choices: Vec<Choice>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub line: markdown::ScriptLine,
    pub old_id: Option<String>,
    pub decision: Option<Decision>,
    pub fixed: bool,
    pub candidates: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preview {
    pub fingerprint: String,
    pub rows: Vec<Candidate>,
    pub removed: Vec<String>,
    pub unresolved: usize,
}

pub fn fingerprint(p: &Project) -> Result<String> {
    Ok(hash_bytes(
        &serde_json::to_vec(&serde_json::to_value(p).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?,
    ))
}

pub fn preview(p: &Project, request: &Request) -> Result<Preview> {
    let (start, end) = request.range.unwrap_or((0, p.segments.len()));
    if start > end || end > p.segments.len() {
        return Err("更新範囲が不正です".into());
    }
    let old = &p.segments[start..end];
    let mut rows: Vec<_> = markdown::parse(&request.text)?
        .into_iter()
        .filter(|r| r.enabled && !r.text.is_empty())
        .map(|line| Candidate {
            line,
            old_id: None,
            decision: None,
            fixed: false,
            candidates: vec![],
        })
        .collect();
    if p.segments.len() - old.len() + rows.len() > 1000 {
        return Err("台本は1000発話以内にしてください".into());
    }
    let mut used = HashSet::new();
    for c in &request.choices {
        let row = rows
            .iter_mut()
            .find(|r| r.line.line == c.new_line)
            .ok_or("固定した起点の再指定が必要です")?;
        if row.fixed {
            return Err("同じ新行への指定が重複しています".into());
        }
        match (&c.decision, &c.old_id) {
            (Decision::Add, None) => (),
            (Decision::Keep | Decision::Replace, Some(id)) => {
                if !old.iter().any(|s| s.id == *id) || !used.insert(id.clone()) {
                    return Err("旧行が更新範囲にないか、複数の新行に指定されています".into());
                }
            }
            _ => return Err("新規追加と既存行への対応指定が矛盾しています".into()),
        }
        row.old_id = c.old_id.clone();
        row.decision = Some(c.decision.clone());
        row.fixed = true;
    }
    // 双方向に一意な完全一致だけを自動確定候補にする。重複文の位置は推測しない。
    for index in 0..rows.len() {
        if rows[index].fixed {
            continue;
        }
        let text = rows[index].line.text.clone();
        let matches: Vec<_> = old
            .iter()
            .filter(|s| !used.contains(&s.id) && s.source_text == text)
            .collect();
        let new_count = rows
            .iter()
            .filter(|r| !r.fixed && r.line.text == text)
            .count();
        rows[index].candidates = matches.iter().map(|s| s.id.clone()).collect();
        if matches.len() == 1 && new_count == 1 {
            rows[index].old_id = Some(matches[0].id.clone());
            rows[index].decision = Some(Decision::Keep);
            used.insert(matches[0].id.clone());
        }
    }
    let removed = old
        .iter()
        .filter(|s| !used.contains(&s.id))
        .map(|s| s.id.clone())
        .collect();
    let unresolved = rows.iter().filter(|r| r.decision.is_none()).count();
    Ok(Preview {
        fingerprint: fingerprint(p)?,
        rows,
        removed,
        unresolved,
    })
}

pub fn apply(
    p: &Project,
    request: &Request,
    expected: &str,
    confirm_removed: bool,
) -> Result<Project> {
    if fingerprint(p)? != expected {
        return Err("台本または音声が更新されました。再比較してください".into());
    }
    let review = preview(p, request)?;
    if review.unresolved != 0 {
        return Err("未解決の行があります。対応または新規追加を指定してください".into());
    }
    if !review.removed.is_empty() && !confirm_removed {
        return Err("旧行の廃棄を確認してください".into());
    }
    // 出力設定の参照解決UIが揃うまで、設定付き台本の変更は安全側で止める。
    if !p.export_settings.is_null() {
        return Err("出力設定への影響確認が必要です".into());
    }
    let (start, end) = request.range.unwrap_or((0, p.segments.len()));
    let mut result = p.clone();
    let mut retained = HashSet::new();
    let mut next = Vec::new();
    for row in &review.rows {
        let old = row
            .old_id
            .as_ref()
            .and_then(|id| p.segments[start..end].iter().find(|s| s.id == *id));
        let mut s = if row.decision == Some(Decision::Keep) {
            let prior = old.ok_or("維持する旧行がありません")?;
            retained.insert(prior.id.clone());
            prior.clone()
        } else {
            Segment::new(row.line.text.clone())
        };
        if s.line_id.is_empty() {
            s.line_id = s.id.clone();
        }
        if s.line_revisions.is_empty() {
            s.line_revisions.push(LineRevision {
                id: id(),
                source_text: s.source_text.clone(),
                source_markdown: String::new(),
                created: now(),
            });
        }
        if s.source_text != row.line.text
            || s.line_revisions.last().unwrap().source_markdown != row.line.raw
        {
            s.line_revisions.push(LineRevision {
                id: id(),
                source_text: row.line.text.clone(),
                source_markdown: row.line.raw.clone(),
                created: now(),
            });
        }
        // 維持の場合は編集本文・採用・文字位置をそのまま保つ。本文を変更する解決操作は別途実装。
        s.source_text = row.line.text.clone();
        s.source_line = Some(row.line.line);
        s.kind = row.line.kind;
        s.hierarchy = row.line.hierarchy.clone();
        result.match_history.push(serde_json::json!({"old_id":row.old_id,"new_id":s.id,"decision":row.decision,"fixed":row.fixed}));
        next.push(s);
    }
    result.archived_segments.extend(
        p.segments[start..end]
            .iter()
            .filter(|s| !retained.contains(&s.id))
            .cloned(),
    );
    result.segments.splice(start..end, next);
    if request.range.is_none() {
        result.source_markdown = request.text.clone();
        result.script_lines = markdown::parse(&request.text)?;
    } else {
        // 部分更新で旧source_markdownを再利用して誤対応しないよう、新順序で再構築する。
        result.source_markdown = result
            .segments
            .iter()
            .map(|s| {
                s.line_revisions
                    .last()
                    .filter(|r| !r.source_markdown.is_empty())
                    .map(|r| r.source_markdown.clone())
                    .or_else(|| {
                        p.script_lines
                            .iter()
                            .find(|r| Some(r.line) == s.source_line && r.text == s.source_text)
                            .map(|r| r.raw.clone())
                    })
                    .unwrap_or_else(|| s.source_text.clone())
            })
            .collect::<Vec<_>>()
            .join("\n");
        result.script_lines = markdown::parse(&result.source_markdown)?;
        for (index, s) in result.segments.iter_mut().enumerate() {
            s.source_line = Some(index);
            s.kind = result.script_lines[index].kind;
            s.hierarchy = result.script_lines[index].hierarchy.clone();
        }
    }
    for row in &mut result.script_lines {
        if let Some(s) = result
            .segments
            .iter()
            .find(|s| s.source_line == Some(row.line))
        {
            row.enabled = !s.skip;
        }
    }
    Ok(result)
}
