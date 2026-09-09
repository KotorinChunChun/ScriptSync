use crate::domain::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn read_dropped_files(paths: &[String]) -> Result<Value> {
    use std::io::Read;
    if paths.is_empty() || paths.len() > 100 { return Err("台本ファイルは1～100個で指定してください".into()); }
    let mut paths = paths.to_vec();
    paths.sort_by(|a, b| std::path::Path::new(a).file_name().cmp(&std::path::Path::new(b).file_name()).then(a.cmp(b)));
    let mut text = String::new();
    let mut names = Vec::new();
    for path in paths {
        let path = std::path::Path::new(&path);
        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if !matches!(extension.as_str(), "md" | "markdown" | "txt") { return Err("Markdown（.md / .markdown）または.txtをドロップしてください".into()); }
        let file = std::fs::File::open(path).map_err(|e| format!("台本を読み込めません: {e}"))?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() { return Err("フォルダーは読み込めません".into()); }
        if !text.is_empty() { text.push_str("\n\n"); }
        let mut part = String::new();
        file.take(2_000_001u64.saturating_sub(text.len() as u64)).read_to_string(&mut part).map_err(|e| format!("UTF-8の台本を読み込めません: {e}"))?;
        if text.len() + part.len() > 2_000_000 { return Err("台本は合計2MB以内にしてください".into()); }
        text.push_str(part.trim_start_matches('\u{feff}'));
        if text.len() > 2_000_000 { return Err("台本は合計2MB以内にしてください".into()); }
        names.push(path.file_name().unwrap_or_default().to_string_lossy().into_owned());
    }
    Ok(serde_json::json!({"lines":parse(&text)?,"text":text,"names":names}))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptLine {
    pub line: usize,
    pub raw: String,
    pub text: String,
    pub enabled: bool,
    pub kind: u8,
    pub hierarchy: Vec<u32>,
}

fn readable(raw: &str) -> String {
    let mut text = raw.trim().to_owned();
    // リンクの表示文字を残す。コード・装飾の記号は発音しない。
    while let Some(start) = text.find("](") {
        let Some(end) = text[start + 2..].find(')') else {
            break;
        };
        text.replace_range(start..start + 3 + end, "");
        if let Some(open) = text[..start].rfind('[') {
            text.remove(open);
        }
    }
    text.replace(['`', '*', '_'], "")
}

pub fn parse(text: &str) -> Result<Vec<ScriptLine>> {
    if text.len() > 2_000_000 {
        return Err("台本は2MB以内にしてください".into());
    }
    let mut fence: Option<(char, usize)> = None;
    let mut html = false;
    let mut hierarchy = vec![0u32; 4];
    let raw: Vec<_> = text.lines().collect();
    let mut rows = Vec::new();
    for (i, line) in raw.iter().enumerate() {
        let t = line.trim();
        let marker = t.chars().next().unwrap_or(' ');
        let count = t.chars().take_while(|&c| c == marker).count();
        let is_fence = (marker == '`' || marker == '~') && count >= 3;
        let code =
            fence.is_some() || is_fence || line.starts_with("    ") || line.starts_with('\t');
        if is_fence {
            if let Some((m, n)) = fence {
                if m == marker && count >= n {
                    fence = None;
                }
            } else {
                fence = Some((marker, count));
            }
        }
        if t.starts_with("<!--") {
            html = true;
        }
        let table = t.contains('|')
            && (t.starts_with('|')
                || t.matches('|').count() >= 2
                || raw
                    .get(i + 1)
                    .is_some_and(|r| r.contains("---") && r.contains('|')));
        let ignored = code
            || html
            || table
            || t.is_empty()
            || t.starts_with("![")
            || t.starts_with('<')
            || t.starts_with('>')
            || t.starts_with("[!")
            || (t.starts_with('[') && t.contains("]:"))
            || (t.len() >= 3 && t.chars().all(|c| "-*_ ".contains(c)));
        if t.contains("-->") {
            html = false;
        }
        let mut kind = 0;
        let mut body = t;
        if !ignored {
            let hashes = t.chars().take_while(|&c| c == '#').count();
            if (1..=3).contains(&hashes) && t.as_bytes().get(hashes) == Some(&b' ') {
                kind = hashes as u8;
                body = t[hashes..].trim().trim_end_matches('#').trim_end();
            } else if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
                kind = 4;
                body = &t[2..];
            } else {
                let digits = t.bytes().take_while(|c| c.is_ascii_digit()).count();
                if digits > 0 && (t[digits..].starts_with(". ") || t[digits..].starts_with(") ")) {
                    kind = 4;
                    body = &t[digits + 2..];
                }
            }
            if kind > 0 {
                hierarchy[kind as usize - 1] += 1;
                for n in hierarchy.iter_mut().skip(kind as usize) {
                    *n = 0;
                }
            }
        }
        rows.push(ScriptLine {
            line: i,
            raw: line.to_string(),
            text: readable(body),
            enabled: !ignored && !t.starts_with("####"),
            kind,
            hierarchy: hierarchy.clone(),
        });
    }
    Ok(rows)
}

pub fn commit(p: &mut Project, v: &Value) -> Result<()> {
    let text = v["text"].as_str().ok_or("元原稿が必要です")?;
    let mut rows = parse(text)?;
    let choices = v["lines"].as_array().ok_or("行の選択が必要です")?;
    for row in &mut rows {
        if let Some(choice) = choices
            .iter()
            .find(|c| c["line"].as_u64() == Some(row.line as u64))
        {
            row.enabled = choice["enabled"].as_bool().unwrap_or(row.enabled);
        }
    }
    let active: Vec<_> = rows
        .iter()
        .filter(|r| r.enabled && !r.text.is_empty())
        .collect();
    if active.len() > 1000 {
        return Err("読み上げる行は1000行以内で選んでください".into());
    }
    let old = std::mem::take(&mut p.segments);
    let mut used = std::collections::HashSet::new();
    for row in active {
        // 同じ行・元原稿なら、録音画面での読みや句読点の変更も保持する。
        let previous = old
            .iter()
            .chain(p.archived_segments.iter())
            .find(|s| s.source_line == Some(row.line) && s.source_text == row.text);
        let mut s = if let Some(s) = previous {
            used.insert(s.id.clone());
            s.clone()
        } else {
            Segment::new(row.text.clone())
        };
        s.source_line = Some(row.line);
        s.skip = false;
        s.source_text = row.text.clone();
        s.kind = row.kind;
        s.hierarchy = row.hierarchy.clone();
        p.segments.push(s);
    }
    p.archived_segments.retain(|s| !used.contains(&s.id));
    p.archived_segments
        .extend(old.into_iter().filter(|s| !used.contains(&s.id)));
    p.source_markdown = text.to_owned();
    p.script_lines = rows;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn dropped_files_are_sorted_and_invalid_inputs_fail_as_a_whole() {
        let dir = std::env::temp_dir().join(id());
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("01.md"); let b = dir.join("02.MD");
        std::fs::write(&a, "\u{feff}最初").unwrap();
        std::fs::write(&b, "次").unwrap();
        let paths = vec![b.to_string_lossy().into_owned(), a.to_string_lossy().into_owned()];
        let result = read_dropped_files(&paths).unwrap();
        assert_eq!(result["text"], "最初\n\n次");
        assert!(read_dropped_files(&[paths[0].clone(), dir.join("missing.md").to_string_lossy().into_owned()]).is_err());
        assert!(read_dropped_files(&[dir.join("other.exe").to_string_lossy().into_owned()]).is_err());
        std::fs::write(&a, "a".repeat(2_000_001)).unwrap();
        assert!(read_dropped_files(&paths).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn markdown_blocks_and_hierarchy() {
        let rows=parse("# 部\n本文\n## 章\n### 節\n- 項\n```rs\ncode\n```\n![image](a.png)\n| A | B |\n|---|---|\n末尾").unwrap();
        assert_eq!(rows[4].hierarchy, vec![1, 1, 1, 1]);
        assert!(rows[5..11].iter().all(|r| !r.enabled));
        assert!(rows[11].enabled);
    }
    #[test]
    fn reconfirm_preserves_recording_edits_and_archives_removed() {
        let mut p = Project::new("test".into());
        let text = "# 見出し\n漢字";
        let lines = parse(text).unwrap();
        let v = json!({"text":text,"lines":lines});
        commit(&mut p, &v).unwrap();
        let id = p.segments[1].id.clone();
        p.segments[1].text = "かんじ。".into();
        commit(&mut p, &v).unwrap();
        assert_eq!(p.segments[1].id, id);
        assert_eq!(p.segments[1].text, "かんじ。");
        let mut v = v;
        v["lines"][1]["enabled"] = json!(false);
        commit(&mut p, &v).unwrap();
        assert_eq!(p.archived_segments[0].id, id);
        v["lines"][1]["enabled"] = json!(true);
        commit(&mut p, &v).unwrap();
        assert_eq!(p.segments[1].id, id);
        assert_eq!(p.segments[1].text, "かんじ。");
        assert!(p.archived_segments.is_empty());
        for line in v["lines"].as_array_mut().unwrap() {
            line["enabled"] = json!(false);
        }
        commit(&mut p, &v).unwrap();
        assert!(p.segments.is_empty());
        assert_eq!(p.archived_segments.len(), 2);
        v["lines"][1]["enabled"] = json!(true);
        commit(&mut p, &v).unwrap();
        assert_eq!(p.segments[0].id, id);
    }
    #[test]
    fn source_and_edited_text_survive_project_reopen() {
        let path = std::env::temp_dir().join(format!("scriptsync-markdown-{}", id()));
        let mut store = crate::storage::Store::open(path.clone(), true).unwrap();
        let text = "# 部\n## 章\n### 節\n1. 漢字を読みます。";
        commit(
            &mut store.project,
            &json!({"text":text,"lines":parse(text).unwrap()}),
        )
        .unwrap();
        store.project.segments[3].text = "かんじを、読みます。".into();
        store.save().unwrap();
        drop(store);
        let store = crate::storage::Store::open(path.clone(), false).unwrap();
        let s = &store.project.segments[3];
        assert_eq!(s.source_text, "漢字を読みます。");
        assert_eq!(s.text, "かんじを、読みます。");
        assert_eq!(s.hierarchy, vec![1, 1, 1, 1]);
        assert_eq!(store.project.source_markdown, text);
        drop(store);
        std::fs::remove_dir_all(path).unwrap();
    }
}
