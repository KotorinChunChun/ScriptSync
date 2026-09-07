//! TalkのValueTree形式。形式資料はdocs/talk-project-format.mdを参照。
use crate::{app::string, domain::*, storage::*};
use serde_json::{Value, json};
use std::path::Path;
#[derive(Clone, Debug, PartialEq)]
struct Node {
    name: String,
    props: Vec<(String, Vec<u8>)>,
    children: Vec<Node>,
}
impl Node {
    fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            props: vec![],
            children: vec![],
        }
    }
    fn text(mut self, key: &str, value: &str) -> Self {
        let mut b = vec![5];
        b.extend(value.as_bytes());
        b.push(0);
        self.props.push((key.into(), b));
        self
    }
    fn child(mut self, node: Node) -> Self {
        self.children.push(node);
        self
    }
    fn value(&self, key: &str) -> Result<String> {
        let b = &self
            .props
            .iter()
            .find(|(k, _)| k == key)
            .ok_or(format!("Talk項目がありません: {key}"))?
            .1;
        if b.first() != Some(&5) || b.last() != Some(&0) {
            return Err("Talk文字列の型が不正です".into());
        }
        String::from_utf8(b[1..b.len() - 1].to_vec()).map_err(|e| e.to_string())
    }
    fn find<'a>(&'a self, name: &str, out: &mut Vec<&'a Node>) {
        if self.name == name {
            out.push(self);
        }
        for c in &self.children {
            c.find(name, out);
        }
    }
}
fn integer(out: &mut Vec<u8>, mut n: usize) {
    if n == 0 {
        out.push(0);
        return;
    }
    let mut b = vec![];
    while n > 0 {
        b.push((n & 255) as u8);
        n >>= 8;
    }
    out.push(b.len() as u8);
    out.extend(b);
}
fn encode(node: &Node, out: &mut Vec<u8>) {
    out.extend(node.name.as_bytes());
    out.push(0);
    integer(out, node.props.len());
    for (k, v) in &node.props {
        out.extend(k.as_bytes());
        out.push(0);
        integer(out, v.len());
        out.extend(v);
    }
    integer(out, node.children.len());
    for c in &node.children {
        encode(c, out);
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    budget: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self.at.checked_add(n).ok_or("Talk長さが不正です")?;
        let b = self
            .bytes
            .get(self.at..end)
            .ok_or("Talkファイルが途中で切れています")?;
        self.at = end;
        Ok(b)
    }
    fn int(&mut self) -> Result<usize> {
        let n = self.take(1)?[0] as usize;
        if n > 4 {
            return Err("Talk整数形式が不正です".into());
        }
        Ok(self
            .take(n)?
            .iter()
            .enumerate()
            .map(|(i, b)| (*b as usize) << (8 * i))
            .sum())
    }
    fn string(&mut self) -> Result<String> {
        let n = self.bytes[self.at..]
            .iter()
            .position(|b| *b == 0)
            .ok_or("Talk文字列が未終端です")?;
        let b = self.take(n + 1)?;
        String::from_utf8(b[..n].to_vec()).map_err(|e| e.to_string())
    }
    fn node(&mut self, depth: usize) -> Result<Node> {
        if depth > 32 || self.budget == 0 {
            return Err("Talk構造が大きすぎます".into());
        }
        self.budget -= 1;
        let name = self.string()?;
        let count = self.int()?;
        if count > 1000 {
            return Err("Talk属性数が不正です".into());
        }
        let mut props = vec![];
        for _ in 0..count {
            let k = self.string()?;
            let n = self.int()?;
            props.push((k, self.take(n)?.to_vec()));
        }
        let count = self.int()?;
        if count > 10000 {
            return Err("Talkノード数が不正です".into());
        }
        let mut children = vec![];
        for _ in 0..count {
            children.push(self.node(depth + 1)?);
        }
        Ok(Node {
            name,
            props,
            children,
        })
    }
}
fn decode(bytes: &[u8]) -> Result<Node> {
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("Talkプロジェクトは16 MB以内です".into());
    }
    let mut r = Reader {
        bytes,
        at: 0,
        budget: 20000,
    };
    let node = r.node(0)?;
    if r.at != bytes.len() || node.name != "TSTalker" {
        return Err("対応するTalkプロジェクトではありません".into());
    }
    Ok(node)
}
fn param(name: &str, value: f64) -> Node {
    Node::new(name).text("values", &format!("{value:.9}"))
}
fn build(v: &Variant) -> Result<Node> {
    let a = &v.actual;
    if a["voice_name"] != "tanaka-san_ja_JP" {
        return Err("現在のTalk編集出力は田中さんの音声に対応します".into());
    }
    let version = a["voice_version"].as_str().ok_or("話者版が不明です")?;
    let tsml = a["analyzed_text"]
        .as_str()
        .ok_or("解析済み発音データがありません")?;
    let tsml = tsml
        .strip_prefix("<tsml>")
        .and_then(|t| t.strip_suffix("</tsml>"))
        .unwrap_or(tsml);
    if !tsml.contains("<acoustic_phrase>") || tsml.contains('\0') || tsml.contains('\u{fffd}') {
        return Err("解析済み発音データが不正です".into());
    }
    let durations = a["phoneme_durations"]
        .as_array()
        .ok_or("音素長がありません")?;
    if durations.len()
        != a["phonemes"]
            .as_array()
            .ok_or("音素情報がありません")?
            .len()
        || durations.is_empty()
    {
        return Err("音素と長さが一致しません".into());
    }
    let values: Vec<String> = durations
        .iter()
        .map(|d| {
            d.as_f64()
                .filter(|n| n.is_finite() && *n >= 0.0 && *n <= 10.0)
                .map(|d| format!("{d:.6}"))
                .ok_or("音素長が不正です".to_string())
        })
        .collect::<Result<_>>()?;
    let g = &a["global_parameters"];
    let num = |key: &str| {
        g[key]
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or(format!("声条件がありません: {key}"))
    };
    let style = g["style_weights"]
        .as_array()
        .ok_or("スタイル比率がありません")?;
    let mut style_text = String::from("0:1");
    for w in style {
        let n = w
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0)
            .ok_or("スタイル比率が不正です")?;
        style_text += &format!(":{n:.6}");
    }
    if num("huskiness")? != 0.0 {
        return Err("ハスキー設定のTalk編集形式への変換は未対応です".into());
    }
    let utterance = Node::new("Utterance")
        .text("text", &v.text)
        .text("tsml", tsml)
        .text("start", "0.000")
        .text("export_name", "ScriptSync_1")
        .child(Node::new("FrameStyle").text("values", &style_text))
        .child(Node::new("PhonemeOriginalDuration").text("values", &values.join(",")))
        .child(param("SpeedRatio", num("speed")?))
        .child(param(
            "C0Shift",
            num("volume")? * 0.1151292877 + 0.00000001758793963,
        ))
        .child(param(
            "LogF0Shift",
            num("pitch")? * 0.0005776230847 - 0.00000007838179519,
        ))
        .child(param("AlphaShift", num("alp")?))
        .child(param("LogF0Scale", num("intonation")?));
    let track = Node::new("Track")
        .text("name", "ScriptSync")
        .child(
            Node::new("Voice")
                .text("speaker", "Tanaka San")
                .text("name", "techno-sp_ja_JP_f701_tts.tsnvoice")
                .text("version", version),
        )
        .child(Node::new("Contents").child(utterance));
    let mut root = Node::new("TSTalker").child(Node::new("Tracks").child(track));
    root.props
        .push(("format_version".into(), vec![1, 6, 0, 0, 0]));
    Ok(root)
}
pub fn export(store: &Store, payload: &Value) -> Result<Value> {
    let sid = string(payload, "segment")?;
    let rid = string(payload, "run")?;
    let vid = string(payload, "variant")?;
    let s = store
        .project
        .segments
        .iter()
        .find(|s| s.id == sid)
        .ok_or("発話がありません")?;
    let r = s
        .comparisons
        .iter()
        .find(|r| r.id == rid && !r.stale)
        .ok_or("現在の比較がありません")?;
    let v = r
        .variants
        .iter()
        .find(|v| v.id == vid && v.status == "succeeded")
        .ok_or("生成済み音声がありません")?;
    if hash_file(&safe(&store.root, &v.file)?)? != v.hash {
        return Err("音声が変わっています".into());
    }
    let tree = build(v)?;
    let mut bytes = vec![];
    encode(&tree, &mut bytes);
    if decode(&bytes)? != tree {
        return Err("Talk出力の再読込検査に失敗しました".into());
    }
    let folder = safe(&store.root, &format!("exports/talk-{}", id()))?;
    let file = folder.join("reading.tstprj");
    write_new(&file, &bytes)?;
    json_new(
        &folder.join("source.json"),
        &json!({"run":r.id,"variant":v,"project_file_hash":hash_bytes(&bytes),"verification":"binary-roundtrip-only","limitations":"Talk画面での読込・編集保存・再合成は未検証。田中さん、ハスキー0に限定。"}),
    )?;
    Ok(
        json!({"path":file,"warning":"Talk実画面での往復は未検証です。生成条件はsource.jsonに保存しました。"}),
    )
}
pub fn inspect_file(path: &Path) -> Result<Value> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
        return Err("ファイルが大きすぎます".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let tree = decode(&bytes)?;
    let mut utterances = vec![];
    tree.find("Utterance", &mut utterances);
    let mut voices = vec![];
    tree.find("Voice", &mut voices);
    Ok(
        json!({"path":path,"hash":hash_bytes(&bytes),"voices":voices.iter().map(|v|json!({"speaker":v.value("speaker").ok(),"name":v.value("name").ok(),"version":v.value("version").ok()})).collect::<Vec<_>>(),"utterances":utterances.iter().map(|u|json!({"text":u.value("text").ok(),"tsml":u.value("tsml").ok(),"start":u.value("start").ok(),"parameters":u.children.iter().map(|c|json!({"name":c.name,"values":c.value("values").ok()})).collect::<Vec<_>>()})).collect::<Vec<_>>()}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary_roundtrip_and_malformed_lengths() {
        let root = Node::new("TSTalker").text("text", &"市場😀".repeat(100));
        let mut b = vec![];
        encode(&root, &mut b);
        assert_eq!(decode(&b).unwrap(), root);
        assert!(decode(&b[..b.len() - 2]).is_err());
        assert!(decode(b"TSTalker\0\xff").is_err());
    }
    #[test]
    fn verify_public_sample_if_available() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../output/research/template.tstprj");
        if path.exists() {
            let bytes = std::fs::read(path).unwrap();
            let n = decode(&bytes).unwrap();
            let mut out = vec![];
            encode(&n, &mut out);
            assert_eq!(bytes, out);
        }
    }
}
