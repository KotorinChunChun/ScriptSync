use crate::{app::string, domain::*, speech::same_conditions, storage::*};
use serde_json::{Value, json};
use std::path::Path;
fn evidence<'a>(root: &Path, run: &'a Comparison) -> Result<&'a Variant> {
    if run.stale || (!run.kind.is_empty() && run.kind != "reading") {
        return Err("現行原稿の読み比較を指定してください".into());
    }
    let a = run
        .variants
        .iter()
        .find(|v| v.label == "A")
        .ok_or("原文比較がありません")?;
    let c = run
        .variants
        .iter()
        .find(|v| v.label == "C")
        .ok_or("局所補正比較がありません")?;
    for v in [a, c] {
        if v.status != "succeeded"
            || !v.error.is_empty()
            || hash_file(&safe(root, &v.file)?)? != v.hash
        {
            return Err("比較音声または条件を確認できません".into());
        }
    }
    if !same_conditions(&a.actual, &c.actual) {
        return Err("声条件が一致しません".into());
    }
    a.rating.as_ref().ok_or("原文の採点が必要です")?;
    c.rating.as_ref().ok_or("局所補正の採点が必要です")?;
    Ok(c)
}
fn improves(run: &Comparison) -> bool {
    let rating = |label: &str| {
        run.variants
            .iter()
            .find(|v| v.label == label)
            .and_then(|v| v.rating.as_ref())
    };
    match (rating("A"), rating("C")) {
        (Some(a), Some(c)) => match (a.accuracy, c.accuracy, a.naturalness, c.naturalness) {
            (Some(aa), Some(ca), Some(an), Some(cn)) => ca >= 4 && ca > aa && cn >= 4 && cn >= an,
            _ => false,
        },
        _ => false,
    }
}
pub fn mutate(store: &mut Store, action: &str, v: &Value) -> Result<()> {
    if action == "rule_create" {
        let sid = string(v, "segment")?;
        let rid = string(v, "run")?;
        let dictionary = string(v, "dictionary")?;
        if dictionary.trim().is_empty() {
            return Err("確認した辞書条件（変更なし等）を記録してください".into());
        }
        let s = store
            .project
            .segments
            .iter()
            .find(|s| s.id == sid)
            .ok_or("発話がありません")?;
        let run = s
            .comparisons
            .iter()
            .find(|r| r.id == rid)
            .ok_or("比較がありません")?;
        evidence(&store.root, run)?;
        if !improves(run) {
            return Err(
                "候補化には読みの改善と自然さの維持（補正後は各4以上）の採点が必要です".into(),
            );
        }
        let accepted: Vec<_> = s
            .corrections
            .iter()
            .filter(|c| c.decision == "accepted")
            .collect();
        if accepted.len() != 1 {
            return Err("個別ルールの根拠には補正1件だけの比較を使用してください".into());
        }
        let c = accepted[0];
        let known_source_hashes = store
            .project
            .segments
            .iter()
            .flat_map(|s| s.comparisons.iter().map(|r| r.source_hash.clone()))
            .collect();
        store.project.rules.push(ReadingRule {
            known_source_hashes,
            id: id(),
            original: c.original.clone(),
            reading: c.replacement.clone(),
            context: s.text.clone(),
            dictionary,
            source: run.clone(),
            status: "draft".into(),
            validations: vec![],
            history: vec![json!({"action":"created","at":now()})],
            created: now(),
        });
        return Ok(());
    }
    let id = string(v, "id")?;
    let index = store
        .project
        .rules
        .iter()
        .position(|r| r.id == id)
        .ok_or("ルールがありません")?;
    if action == "rule_validate" {
        let rid = string(v, "run")?;
        let rule = &store.project.rules[index];
        let (s, run) = store
            .project
            .segments
            .iter()
            .find_map(|s| s.comparisons.iter().find(|r| r.id == rid).map(|r| (s, r)))
            .ok_or("検証比較がありません")?;
        let c = evidence(&store.root, run)?;
        let source = evidence(&store.root, &rule.source)?;
        if run.source_hash == rule.source.source_hash
            || rule.known_source_hashes.contains(&run.source_hash)
            || run.created < rule.created
            || !same_conditions(&source.actual, &c.actual)
        {
            return Err("候補作成後の未使用文を、同じ声条件で比較してください".into());
        }
        if string(v, "dictionary")? != rule.dictionary {
            return Err("辞書条件が一致しません".into());
        }
        let accepted: Vec<_> = s
            .corrections
            .iter()
            .filter(|c| c.decision == "accepted")
            .collect();
        if accepted.len() != 1
            || accepted[0].original != rule.original
            || accepted[0].replacement != rule.reading
        {
            return Err("同じ語・読みの補正1件だけを検証してください".into());
        }
        if rule.validations.iter().any(|r| r.id == run.id) {
            return Err("登録済みの検証です".into());
        }
        let run = run.clone();
        let r = &mut store.project.rules[index];
        r.validations.push(run);
        r.status = "draft".into();
        r.history
            .push(json!({"action":"validated","run":rid,"at":now()}));
    } else if action == "rule_decide" {
        let decision = string(v, "decision")?;
        if !["approved", "rejected", "draft"].contains(&decision.as_str()) {
            return Err("判断が不正です".into());
        }
        let r = &store.project.rules[index];
        if decision == "approved" {
            evidence(&store.root, &r.source)?;
            if r.validations.is_empty() || !improves(&r.source) {
                return Err("未使用文の検証が必要です".into());
            }
            for run in &r.validations {
                evidence(&store.root, run)?;
                if !improves(run) {
                    return Err("反例または改善未確認の検証があります".into());
                }
            }
        }
        let r = &mut store.project.rules[index];
        r.status = decision.clone();
        r.history.push(json!({"action":decision,"at":now()}));
    } else {
        return Err("未対応のルール操作です".into());
    }
    Ok(())
}
pub fn export(store: &Store) -> Result<Value> {
    let approved: Vec<_> = store
        .project
        .rules
        .iter()
        .filter(|r| r.status == "approved")
        .collect();
    if approved.is_empty() {
        return Err("承認済みルールがありません".into());
    }
    let mut text = String::from(
        "# 読み補正の参考ルール\n\n以下は指定文脈・声・辞書条件で確認した参考事例です。新しい原稿の出現位置ごとに試聴・承認し、一律置換しないでください。\n\n",
    );
    for r in &approved {
        let c = evidence(&store.root, &r.source)?;
        if r.validations.is_empty() || !improves(&r.source) {
            return Err("ルールの検証が不足しています".into());
        }
        for run in &r.validations {
            evidence(&store.root, run)?;
            if !improves(run) {
                return Err("反例があります".into());
            }
        }
        text += &format!(
            "- 語：{} / 読み：{}\n  文脈：{}\n  話者：{} / 版：{}\n  辞書条件（利用者確認）：{}\n  根拠比較：{} / 未使用文の比較：{}\n\n",
            r.original,
            r.reading,
            r.context,
            c.actual["voice_name"],
            c.actual["voice_version"],
            r.dictionary,
            r.source.id,
            r.validations
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let folder = safe(&store.root, &format!("exports/rules-{}", id()))?;
    write_new(&folder.join("prompt.md"), text.as_bytes())?;
    json_new(&folder.join("evidence.json"), &approved)?;
    Ok(json!({"path":folder.join("prompt.md")}))
}
