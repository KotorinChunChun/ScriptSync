use serde::{Deserialize, Serialize};
pub type Result<T> = std::result::Result<T, String>;
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Take {
    #[serde(default)]
    pub script_text: String,
    /// 保存済みの残す区間（秒）。旧プロジェクトの 0, 0 は全区間を表す。
    #[serde(default)]
    pub trim_start: f64,
    #[serde(default)]
    pub trim_end: f64,
    pub id: String,
    pub file: String,
    pub hash: String,
    pub status: String,
    pub duration: f64,
    pub peak: f32,
    pub warning: String,
    pub created: u64,
    pub endpoint: String,
}
impl Take {
    pub fn trimmed_range(&self) -> Result<(f64, f64)> {
        let end = if self.trim_start == 0.0 && self.trim_end == 0.0 {
            self.duration
        } else {
            self.trim_end
        };
        if !self.trim_start.is_finite()
            || !end.is_finite()
            || self.trim_start < 0.0
            || end <= self.trim_start
            || end > self.duration
        {
            return Err("テイクの残す範囲が不正です".into());
        }
        Ok((self.trim_start, end))
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Mapping {
    pub start: usize,
    pub end: usize,
    pub kana: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Correction {
    pub id: String,
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub replacement: String,
    pub decision: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Rating {
    pub accuracy: Option<u8>,
    pub naturalness: Option<u8>,
    pub accent: Option<u8>,
    pub rhythm: Option<u8>,
    pub emotion: Option<u8>,
    pub comment: String,
    pub created: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Variant {
    pub id: String,
    pub label: String,
    pub text: String,
    pub file: String,
    pub hash: String,
    pub status: String,
    pub error: String,
    pub plan: serde_json::Value,
    pub actual: serde_json::Value,
    pub transcript: String,
    pub rating: Option<Rating>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Comparison {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub correction_hash: String,
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub reference_kana: String,
    pub id: String,
    pub source_hash: String,
    pub reference_hash: String,
    pub variants: Vec<Variant>,
    pub order: Vec<String>,
    pub revealed: bool,
    pub created: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Segment {
    /// v0.0.2以降の内容ベース照合で使う不変の行識別子。旧形式ではsegment IDを引き継ぐ。
    #[serde(default)]
    pub line_id: String,
    #[serde(default)]
    pub line_revisions: Vec<LineRevision>,
    #[serde(default)]
    pub source_line: Option<usize>,
    #[serde(default)]
    pub source_text: String,
    #[serde(default)]
    pub kind: u8,
    #[serde(default)]
    pub hierarchy: Vec<u32>,
    #[serde(default)]
    pub take_notes: std::collections::HashMap<String, TakeNote>,
    #[serde(default)]
    pub needs_review: bool,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub transcriptions: Vec<serde_json::Value>,
    pub id: String,
    pub text: String,
    pub revision: u64,
    pub takes: Vec<Take>,
    pub selected: Option<String>,
    pub kana: String,
    pub confirmed: bool,
    pub mappings: Vec<Mapping>,
    pub corrections: Vec<Correction>,
    pub trim_start: f64,
    pub trim_end: f64,
    pub gap: f64,
    pub skip: bool,
    pub comparisons: Vec<Comparison>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LineRevision {
    pub id: String,
    pub source_text: String,
    pub source_markdown: String,
    pub created: u64,
}
impl Segment {
    pub fn new(text: String) -> Self {
        Self {
            line_id: id(),
            line_revisions: vec![],
            source_line: None,
            source_text: text.clone(),
            kind: 0,
            hierarchy: vec![0; 4],
            take_notes: Default::default(),
            needs_review: false,
            gain_db: 0.0,
            transcriptions: vec![],
            id: id(),
            text,
            revision: 1,
            takes: vec![],
            selected: None,
            kana: String::new(),
            confirmed: false,
            mappings: vec![],
            corrections: vec![],
            trim_start: 0.0,
            trim_end: 0.0,
            gap: 0.0,
            skip: false,
            comparisons: vec![],
        }
    }
    pub fn selected_take(&self) -> Result<&Take> {
        self.takes
            .iter()
            .find(|t| Some(&t.id) == self.selected.as_ref() && t.status == "ready")
            .ok_or_else(|| "採用済み音声がありません".into())
    }
}
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct MergeDraft {
    #[serde(default)] pub base_generation: u64,
    #[serde(default)] pub input_markdown: String,
    #[serde(default)] pub fixed_matches: Vec<serde_json::Value>,
    #[serde(default)] pub proposed_order: Vec<String>,
}
#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct TakeNote {
    #[serde(default)]
    pub score: Option<u8>,
    pub memo: String,
    pub favorite: bool,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Project {
    #[serde(default)]
    pub source_markdown: String,
    #[serde(default)]
    pub script_lines: Vec<crate::markdown::ScriptLine>,
    #[serde(default)]
    pub export_settings: serde_json::Value,
    #[serde(default)]
    pub rules: Vec<ReadingRule>,
    #[serde(default)]
    pub batches: Vec<Batch>,
    #[serde(default)]
    pub archived_segments: Vec<Segment>,
    #[serde(default)]
    pub merge_draft: Option<MergeDraft>,
    #[serde(default)]
    pub match_history: Vec<serde_json::Value>,
    #[serde(default)]
    pub deletion_records: Vec<serde_json::Value>,
    pub version: u32,
    pub id: String,
    pub title: String,
    pub generation: u64,
    pub segments: Vec<Segment>,
}
impl Project {
    pub fn new(title: String) -> Self {
        Self {
            source_markdown: String::new(),
            script_lines: vec![],
            export_settings: serde_json::Value::Null,
            rules: vec![],
            batches: vec![],
            archived_segments: vec![],
            merge_draft: None,
            match_history: vec![],
            deletion_records: vec![],
            version: 3,
            id: id(),
            title,
            generation: 0,
            segments: vec![],
        }
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ReadingRule {
    #[serde(default)]
    pub known_source_hashes: Vec<String>,
    pub id: String,
    pub original: String,
    pub reading: String,
    pub context: String,
    pub dictionary: String,
    pub source: Comparison,
    pub status: String,
    pub validations: Vec<Comparison>,
    pub history: Vec<serde_json::Value>,
    pub created: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Batch {
    pub id: String,
    pub items: Vec<BatchItem>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BatchItem {
    pub action: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub error: String,
    pub result_id: Option<String>,
    pub source_hash: String,
}
pub fn slice(text: &str, start: usize, end: usize) -> Result<String> {
    let chars: Vec<char> = text.chars().collect();
    if start >= end || end > chars.len() {
        return Err("原文範囲が不正です（Unicode文字単位）".into());
    }
    Ok(chars[start..end].iter().collect())
}
pub fn corrected(segment: &Segment) -> Result<String> {
    let mut accepted: Vec<_> = segment
        .corrections
        .iter()
        .filter(|c| c.decision == "accepted")
        .collect();
    accepted.sort_by_key(|c| c.start);
    let mut end = 0;
    for c in &accepted {
        if c.start < end
            || slice(&segment.text, c.start, c.end)? != c.original
            || c.replacement.trim().is_empty()
        {
            return Err("補正範囲の重複・原文不一致・空の読みです".into());
        }
        end = c.end;
    }
    let mut chars: Vec<char> = segment.text.chars().collect();
    for c in accepted.iter().rev() {
        chars.splice(c.start..c.end, c.replacement.chars());
    }
    Ok(chars.into_iter().collect())
}
pub fn validate_reference(s: &Segment) -> Result<()> {
    s.selected_take()?;
    if s.kana.trim().is_empty() {
        return Err("正解読みを入力してください".into());
    }
    let mut end = 0;
    for m in &s.mappings {
        slice(&s.text, m.start, m.end)?;
        if m.start < end || m.kana.trim().is_empty() {
            return Err("対応範囲が重複、または読みが空です".into());
        }
        end = m.end;
    }
    Ok(())
}
pub fn normalize(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_whitespace() || "。、！？,.!?「」『』（）()".contains(c) {
                None
            } else if ('ァ'..='ヶ').contains(&c) {
                char::from_u32(c as u32 - 0x60)
            } else {
                Some(c)
            }
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_replacements_keep_repeated_words_and_unicode() {
        let mut s = Segment::new("😀市場と市場".into());
        s.corrections.push(Correction {
            id: id(),
            start: 1,
            end: 3,
            original: "市場".into(),
            replacement: "いちば".into(),
            decision: "accepted".into(),
        });
        assert_eq!(corrected(&s).unwrap(), "😀いちばと市場");
        s.corrections.push(s.corrections[0].clone());
        assert!(corrected(&s).is_err());
    }
    #[test]
    fn normalization_preserves_sounds() {
        assert_eq!(normalize("シジョウ。"), "しじょう");
        assert_ne!(normalize("おう"), normalize("おお"));
    }
}
