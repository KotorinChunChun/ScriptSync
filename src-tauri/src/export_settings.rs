use crate::domain::Result;
use serde_json::{Value, json};
pub fn validate(value: &Value) -> Result<Value> {
    if !value.is_object() {
        return Err("出力設定が必要です".into());
    }
    let mut v = value.clone();
    for (key, choices) in [
        (
            "unit",
            &["all", "part", "chapter", "section", "utterance", "custom"][..],
        ),
        ("naming", &["none", "jp", "number"][..]),
        ("format", &["wav", "flac"][..]),
    ] {
        if !v[key].as_str().is_some_and(|s| choices.contains(&s)) {
            return Err(format!("{key}の設定が不正です"));
        }
    }
    for key in ["normal", "part", "chapter", "section", "head", "tail"] {
        let n = v["global_gaps"][key].as_f64().ok_or("既定間隔が必要です")?;
        if !n.is_finite() || !(0.0..=60.0).contains(&n) {
            return Err("間隔は0～60秒にしてください".into());
        }
    }
    if let Some(blocks) = v["blocks"].as_object_mut() {
        for b in blocks.values_mut() {
            if let Some(gaps) = b["gaps"].as_object_mut() {
                for setting in gaps.values_mut() {
                    if setting["mode"] == "manual" {
                        let n = setting["value"].as_f64().ok_or("手入力の間隔が必要です")?;
                        if !n.is_finite() || !(0.0..=60.0).contains(&n) {
                            return Err("間隔は0～60秒にしてください".into());
                        }
                    } else {
                        *setting = json!({"mode":"default"});
                    }
                }
            }
        }
    }
    Ok(v)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_drops_cached_manual_value() {
        let input = json!({"unit":"all","naming":"jp","format":"flac","global_gaps":{"normal":0.3,"part":1.2,"chapter":0.8,"section":0.5,"head":0.1,"tail":0.3},"blocks":{"a":{"gaps":{"normal":{"mode":"default","value":5.0},"tail":{"mode":"manual","value":1.5}}}}});
        let saved = validate(&input).unwrap();
        assert!(
            saved["blocks"]["a"]["gaps"]["normal"]
                .get("value")
                .is_none()
        );
        assert_eq!(saved["blocks"]["a"]["gaps"]["tail"]["value"], 1.5);
    }
}
