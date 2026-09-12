//! 確定済み原稿のScriptSyncWeb送信。会社クラウドへの直接連携は扱わない。
use crate::{app::App, domain::{Project, Result}};
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::blocking::{Client, Response};
use serde_json::{Value, json};
use sha2::Sha256;
use std::{io::{Cursor, Read}, time::Duration};
const WEB_API: &str = "https://dev-k.sakura.ne.jp/tools/scriptsyncweb/api.php";
const LIMIT: usize = 100 * 1024 * 1024;
fn client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "通信環境を初期化できません".into())
}
fn response(response: Response) -> Result<Value> {
    let status = response.status();
    let value: Value = serde_json::from_reader(response.take(2 * 1024 * 1024))
        .map_err(|_| format!("サーバー応答を読めません ({status})"))?;
    if !status.is_success() {
        let code = value["error"]["code"]
            .as_str()
            .or(value["error"].as_str())
            .or(value["code"].as_str())
            .unwrap_or("request_failed");
        // OAuth応答の詳細・URL・トークンをエラーやログに複製しない。
        let safe: String = code
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .take(60)
            .collect();
        return Err(format!(
            "サービスへの要求が失敗しました ({status}, {safe})。認証・登録設定・保存先を確認してください。"
        ));
    }
    Ok(value)
}
fn send_json(request: reqwest::blocking::RequestBuilder) -> Result<Value> {
    response(
        request
            .send()
            .map_err(|_| "サービスに接続できません。接続先とネットワークを確認してください。")?,
    )
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{key}が必要です"))
}
fn web_token(hn: &str, key: &str) -> Result<String> {
    if hn.is_empty()
        || hn.len() > 1000
        || !key.bytes().all(|c| c.is_ascii_digit())
        || key.len() != 6
    {
        return Err("HNと6桁のワンタイムキーを入力してください".into());
    }
    let mut output = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(
        key.as_bytes(),
        format!("voice-notebook-one-time-key-v2:{hn}").as_bytes(),
        150000,
        &mut output,
    );
    Ok(output.iter().map(|b| format!("{b:02x}")).collect())
}
fn encrypted_zip(bytes: &[u8], key: &str) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .with_aes_encryption(zip::AesMode::Aes256, key);
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        output
            .start_file(file.name(), options)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut output).map_err(|e| e.to_string())?;
    }
    Ok(output.finish().map_err(|e| e.to_string())?.into_inner())
}
fn upload_web_with(
    mut call: impl FnMut(Value) -> Result<Value>,
    bytes: &[u8],
    hn: &str,
    key: &str,
) -> Result<Value> {
    let token = web_token(hn, key)?;
    let encrypted = encrypted_zip(bytes, key)?;
    if encrypted.len() > LIMIT {
        return Err("ZIPは100MiB以内にしてください".into());
    }
    let auth = |action: &str, mut payload: Value| {
        payload["action"] = json!(action);
        payload["hn"] = json!(hn);
        payload["token"] = json!(token);
        payload
    };
    let start = call(auth("start", json!({"size":encrypted.len()})))?;
    let id = text(&start, "id")?;
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Webの転送IDが不正です".into());
    }
    let result = (|| {
        for (index, chunk) in encrypted.chunks(512 * 1024).enumerate() {
            let reply = call(auth(
                "chunk",
                json!({"id":id,"offset":index*512*1024,"data":STANDARD.encode(chunk)}),
            ))?;
            if reply["ok"] != true {
                return Err("Webが分割データの受信を確認しませんでした".into());
            }
        }
        let reply = call(auth("finish", json!({"id":id})))?;
        if reply["ok"] != true {
            return Err("WebがZIPの保存完了を確認しませんでした".into());
        }
        Ok(
            json!({"provider":"web","id":id,"url":crate::machine_settings::WEB_URL,"message":"暗号化ZIPを保存しました。スマートフォンのWeb画面で同じHNと6桁キーを入力して取得してください。保存期間は24時間です。"}),
        )
    })();
    if result.is_err() {
        let _ = call(auth("delete", json!({"id":id})));
    }
    result
}
fn script_bytes(project: &Project, source_id: &str, exported_at: &str) -> Result<Vec<u8>> {
    let path = std::env::temp_dir().join(format!("scriptsync-upload-{}.zip", crate::domain::id()));
    let result = (|| {
        crate::exchange::export_script(project, source_id, &path, exported_at)?;
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        if bytes.len() > LIMIT {
            return Err("ZIPは100MiB以内にしてください".into());
        }
        Ok(bytes)
    })();
    let _ = std::fs::remove_file(path);
    result
}
pub fn dispatch(app: &App, action: &str, payload: &Value) -> Option<Result<Value>> {
    if !action.starts_with("cloud_") { return None; }
    Some((|| {
        if action != "cloud_upload" || payload["provider"].as_str() != Some("web") {
            return Err("この連携には対応していません。ZIP出力またはWebアップロードを使用してください".into());
        }
        let project = {
            let guard = app.0.lock().map_err(|e| e.to_string())?;
            if guard.busy { return Err("処理中です".into()); }
            let store = guard.store.as_ref().ok_or("プロジェクトを開いてください")?;
            crate::exchange::validate_export_request(&store.project, payload)?;
            store.project.clone()
        };
        let source_id = text(payload, "source_id")?;
        crate::exchange::ensure_export_ready(&project, source_id)?;
        let bytes = script_bytes(&project, source_id, text(payload, "exported_at")?)?;
        upload_web_with(|v| send_json(client()?.post(WEB_API).json(&v)), &bytes,
            text(payload, "hn")?, text(payload, "key")?)
    })())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn simple_zip() -> Vec<u8> {
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        out.start_file("project.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(b"{\"format\":\"fixture\"}").unwrap();
        out.finish().unwrap().into_inner()
    }
    #[test]
    fn web_upload_uses_authenticated_ordered_chunks_and_aes256() {
        let mut calls = Vec::new();
        let bytes = simple_zip();
        let id = "1234567890abcdef1234567890abcdef";
        let result = upload_web_with(
            |value| {
                calls.push(value);
                Ok(if calls.len() == 1 {
                    json!({"id":id})
                } else {
                    json!({"ok":true})
                })
            },
            &bytes,
            "fixture",
            "123456",
        )
        .unwrap();
        assert_eq!(result["id"], id);
        assert_eq!(calls[0]["action"], "start");
        assert_eq!(calls[1]["action"], "chunk");
        assert_eq!(calls.last().unwrap()["action"], "finish");
        assert_eq!(calls[1]["offset"], 0);
        let token = web_token("fixture", "123456").unwrap();
        assert!(
            calls
                .iter()
                .all(|v| v["token"] == token && v["hn"] == "fixture")
        );
        let data = STANDARD.decode(calls[1]["data"].as_str().unwrap()).unwrap();
        assert_eq!(data.len() as u64, calls[0]["size"].as_u64().unwrap());
        let mut zip = zip::ZipArchive::new(Cursor::new(data)).unwrap();
        assert!(zip.by_index(0).is_err());
        let mut file = zip.by_index_decrypt(0, b"123456").unwrap();
        let mut decoded = String::new();
        file.read_to_string(&mut decoded).unwrap();
        assert_eq!(decoded, "{\"format\":\"fixture\"}");
    }
    #[test]
    fn failed_web_upload_cleans_only_its_own_reservation() {
        let mut calls = Vec::new();
        let result = upload_web_with(
            |v| {
                calls.push(v.clone());
                match v["action"].as_str() {
                    Some("start") => Ok(json!({"id":"abcdefabcdefabcdefabcdefabcdefab"})),
                    Some("chunk") => Err("network".into()),
                    Some("delete") => Ok(json!({"ok":true})),
                    _ => panic!("unexpected"),
                }
            },
            &simple_zip(),
            "fixture",
            "123456",
        );
        assert!(result.is_err());
        assert_eq!(calls.last().unwrap()["action"], "delete");
        assert_eq!(calls.len(), 3);
    }
    #[test]
    fn removed_providers_cannot_start_network_requests() {
        let app = App::default();
        for provider in ["onedrive", "box"] {
            assert!(dispatch(&app,"cloud_upload", &json!({"provider":provider})).unwrap().is_err());
            assert!(dispatch(&app,"cloud_auth_start", &json!({"provider":provider})).unwrap().is_err());
        }
        assert!(web_token("", "123456").is_err());
        assert!(web_token("fixture", "12345x").is_err());
    }
}
