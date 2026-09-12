//! 確定原稿のWeb・会社クラウド送信。トークンとBox secretはメモリーだけに保持する。
//! 参考: Microsoft device authorization / Graph driveItem content, Box OAuth2 / files-content。
use crate::{
    app::App,
    domain::{Project, Result},
    machine_settings::Settings,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::{
    Url,
    blocking::{Client, Response, multipart},
};
use serde_json::{Value, json};
use sha2::Sha256;
use std::{
    io::{Cursor, Read, Write},
    net::TcpListener,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const WEB_API: &str = "https://dev-k.sakura.ne.jp/tools/scriptsyncweb/api.php";
const BOX_REDIRECT: &str = "http://127.0.0.1:8767/callback";
const LIMIT: usize = 100 * 1024 * 1024;
struct Credential {
    token: String,
    identity: String,
    expires: Instant,
}
struct DeviceFlow {
    code: String,
    identity: String,
    endpoint: String,
    client_id: String,
    next_poll: Instant,
    expires: Instant,
    interval: u64,
}
struct BoxFlow {
    listener: TcpListener,
    state: String,
    secret: String,
    client_id: String,
    expires: Instant,
}
#[derive(Default)]
struct Session {
    microsoft: Option<Credential>,
    box_token: Option<Credential>,
    device: Option<DeviceFlow>,
    box_flow: Option<BoxFlow>,
}
fn session() -> &'static Mutex<Session> {
    static STATE: OnceLock<Mutex<Session>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(Session::default()))
}
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
fn identity(s: &Settings) -> String {
    format!("{}:{}", s.entra_tenant, s.entra_client_id)
}
fn microsoft_endpoint(s: &Settings) -> Result<String> {
    if uuid::Uuid::parse_str(&s.entra_tenant).is_err()
        || uuid::Uuid::parse_str(&s.entra_client_id).is_err()
    {
        return Err("設定で会社のEntraテナントIDとアプリのクライアントIDを指定してください。アプリ登録ではパブリッククライアントフローと委任Files.ReadWriteを有効にします。".into());
    }
    Ok(format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0",
        s.entra_tenant
    ))
}
fn credential(value: &Value, identity: String) -> Result<Credential> {
    if !value["token_type"]
        .as_str()
        .is_some_and(|s| s.eq_ignore_ascii_case("bearer"))
    {
        return Err("認証応答のtoken_typeが不正です".into());
    }
    Ok(Credential {
        token: text(value, "access_token")?.into(),
        identity,
        expires: Instant::now()
            + Duration::from_secs(
                value["expires_in"]
                    .as_u64()
                    .unwrap_or(3600)
                    .saturating_sub(30)
                    .min(86400),
            ),
    })
}
fn token(provider: &str, s: &Settings) -> Result<String> {
    let guard = session().lock().map_err(|e| e.to_string())?;
    let value = if provider == "onedrive" {
        &guard.microsoft
    } else {
        &guard.box_token
    };
    let expected = if provider == "onedrive" {
        identity(s)
    } else {
        s.box_client_id.clone()
    };
    value
        .as_ref()
        .filter(|v| v.identity == expected && v.expires > Instant::now())
        .map(|v| v.token.clone())
        .ok_or_else(|| {
            "送信前にユーザー認証してください。認証はアプリ終了時に破棄されます。".into()
        })
}
fn begin_microsoft(s: &Settings) -> Result<Value> {
    let endpoint = microsoft_endpoint(s)?;
    let value = send_json(client()?.post(format!("{endpoint}/devicecode")).form(&[
        ("client_id", s.entra_client_id.as_str()),
        ("scope", "https://graph.microsoft.com/Files.ReadWrite"),
    ]))?;
    let code = text(&value, "device_code")?.to_owned();
    let user_code = text(&value, "user_code")?.to_owned();
    let expires = value["expires_in"].as_u64().unwrap_or(900).min(1800);
    let interval = value["interval"].as_u64().unwrap_or(5).clamp(5, 60);
    session().lock().map_err(|e| e.to_string())?.device = Some(DeviceFlow {
        code,
        identity: identity(s),
        endpoint,
        client_id: s.entra_client_id.clone(),
        next_poll: Instant::now() + Duration::from_secs(interval),
        expires: Instant::now() + Duration::from_secs(expires),
        interval,
    });
    Ok(
        json!({"url":"https://microsoft.com/devicelogin","user_code":user_code,"interval":interval,"expires_in":expires}),
    )
}
fn poll_microsoft() -> Result<Value> {
    let mut guard = session().lock().map_err(|e| e.to_string())?;
    let flow = guard.device.as_mut().ok_or("認証を開始してください")?;
    if Instant::now() > flow.expires {
        guard.device = None;
        return Err("認証の有効時間が過ぎました。もう一度開始してください。".into());
    }
    if Instant::now() < flow.next_poll {
        return Ok(json!({"pending":true,"interval":flow.interval}));
    }
    let raw = client()?
        .post(format!("{}/token", flow.endpoint))
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", flow.client_id.as_str()),
            ("device_code", flow.code.as_str()),
        ])
        .send()
        .map_err(|_| "認証結果を取得できません")?;
    let status = raw.status();
    let value: Value =
        serde_json::from_reader(raw.take(2 * 1024 * 1024)).map_err(|_| "認証応答が不正です")?;
    if value["error"] == "authorization_pending" || value["error"] == "slow_down" {
        if value["error"] == "slow_down" {
            flow.interval = (flow.interval + 5).min(120);
        }
        flow.next_poll = Instant::now() + Duration::from_secs(flow.interval);
        return Ok(json!({"pending":true,"interval":flow.interval}));
    }
    if !status.is_success() {
        guard.device = None;
        return Err(
            "認証が拒否されたか期限切れです。会社のポリシーとアプリ登録を確認してください。".into(),
        );
    }
    let auth = credential(&value, flow.identity.clone())?;
    guard.microsoft = Some(auth);
    guard.device = None;
    Ok(json!({"ready":true}))
}
fn begin_box(s: &Settings, secret: &str) -> Result<Value> {
    if s.box_client_id.is_empty()
        || s.box_client_id.len() > 200
        || !s.box_client_id.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return Err("設定にBoxアプリのクライアントIDを入力してください".into());
    }
    if secret.is_empty() || secret.len() > 1000 || secret.contains(['\r', '\n', '\0']) {
        return Err("Boxのclient secretが必要です。メモリーだけに保持し保存しません。".into());
    }
    let mut guard = session().lock().map_err(|e| e.to_string())?;
    guard.box_flow = None;
    let listener = TcpListener::bind("127.0.0.1:8767")
        .map_err(|_| "Box認証用ポート8767を開けません。他の認証処理を閉じて再試行してください。")?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let state = crate::domain::id();
    let mut url = Url::parse("https://account.box.com/api/oauth2/authorize").unwrap();
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &s.box_client_id)
        .append_pair("redirect_uri", BOX_REDIRECT)
        .append_pair("state", &state);
    guard.box_flow = Some(BoxFlow {
        listener,
        state,
        secret: secret.into(),
        client_id: s.box_client_id.clone(),
        expires: Instant::now() + Duration::from_secs(600),
    });
    Ok(json!({"url":url.as_str(),"interval":2,"redirect_uri":BOX_REDIRECT}))
}
fn callback_code(target: &str, state: &str) -> Result<String> {
    let url = Url::parse(&format!("http://127.0.0.1:8767{target}"))
        .map_err(|_| "認証コールバックが不正です")?;
    if url.path() != "/callback" {
        return Err("認証コールバックのパスが不正です".into());
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    let get = |key: &str| -> Result<String> {
        let values: Vec<_> = pairs.iter().filter(|(k, _)| k == key).collect();
        if values.len() != 1 {
            return Err("認証コールバックの項目が不正です".into());
        }
        Ok(values[0].1.to_string())
    };
    if get("state")? != state {
        return Err("認証stateが一致しません".into());
    }
    let code = get("code")?;
    if code.is_empty() || code.len() > 4096 {
        return Err("認証コードが不正です".into());
    }
    Ok(code)
}
fn poll_box() -> Result<Value> {
    let mut guard = session().lock().map_err(|e| e.to_string())?;
    let flow = guard.box_flow.as_ref().ok_or("Box認証を開始してください")?;
    if Instant::now() > flow.expires {
        guard.box_flow = None;
        return Err("Box認証の有効時間が過ぎました".into());
    }
    let (mut stream, _) = match flow.listener.accept() {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            return Ok(json!({"pending":true,"interval":2}));
        }
        Err(_) => return Err("認証コールバックを受信できません".into()),
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let mut request = [0u8; 8192];
    let size = stream
        .read(&mut request)
        .map_err(|_| "認証コールバックを読めません")?;
    let request = String::from_utf8_lossy(&request[..size]);
    let line = request.lines().next().unwrap_or("");
    let code = if let Some(target) = line.strip_prefix("GET ").and_then(|v| v.split(' ').next()) {
        callback_code(target, &flow.state)
    } else {
        Err("認証要求が不正です".into())
    };
    let ok = code.is_ok();
    let message = if ok {
        "Authentication received. Return to ScriptSync."
    } else {
        "Invalid authentication callback. Return to ScriptSync."
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {}\r\nContent-Type: text/plain\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        if ok { "200 OK" } else { "400 Bad Request" },
        message.len(),
        message
    );
    let code = match code {
        Ok(c) => c,
        Err(_) => return Ok(json!({"pending":true,"interval":2})),
    };
    let flow = guard.box_flow.take().unwrap();
    let value = send_json(client()?.post("https://api.box.com/oauth2/token").form(&[
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("client_id", flow.client_id.as_str()),
        ("client_secret", flow.secret.as_str()),
        ("redirect_uri", BOX_REDIRECT),
    ]))?;
    guard.box_token = Some(credential(&value, flow.client_id)?);
    Ok(json!({"ready":true}))
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
fn onedrive_url(folder: &str, filename: &str) -> Result<Url> {
    let folder = folder.trim_matches('/');
    let mut url = Url::parse("https://graph.microsoft.com/v1.0/me/drive/root:").unwrap();
    {
        let mut path = url.path_segments_mut().map_err(|_| "送信先が不正です")?;
        for part in folder.split('/').filter(|p| !p.is_empty()) {
            if part == "." || part == ".." || part.contains(['\\', ':', '\0']) {
                return Err(
                    "OneDrive保存先はドライブ直下からのフォルダーパスを指定してください".into(),
                );
            }
            path.push(part);
        }
        path.push(&format!("{filename}:"));
        path.push("content");
    }
    Ok(url)
}
fn upload_onedrive(bytes: Vec<u8>, s: &Settings, filename: &str) -> Result<Value> {
    let auth = token("onedrive", s)?;
    let client = client()?;
    let url = onedrive_url(&s.onedrive_folder, filename)?;
    // 一意なファイル名で既存ファイルを置き換えない。フォルダーは利用者が設定した既存フォルダー。
    let value = send_json(
        client
            .put(url)
            .bearer_auth(auth)
            .header("Content-Type", "application/zip")
            .body(bytes),
    )?;
    Ok(
        json!({"provider":"onedrive","id":text(&value,"id")?,"name":filename,"message":"OneDriveへZIPをアップロードしました"}),
    )
}
fn upload_box(bytes: Vec<u8>, s: &Settings, filename: &str) -> Result<Value> {
    if s.box_folder.is_empty() || !s.box_folder.bytes().all(|c| c.is_ascii_digit()) {
        return Err("Box保存先フォルダーIDを指定してください".into());
    }
    let form = multipart::Form::new()
        .text(
            "attributes",
            json!({"name":filename,"parent":{"id":s.box_folder}}).to_string(),
        )
        .part(
            "file",
            multipart::Part::bytes(bytes)
                .file_name(filename.to_owned())
                .mime_str("application/zip")
                .map_err(|e| e.to_string())?,
        );
    let value = send_json(
        client()?
            .post("https://upload.box.com/api/2.0/files/content")
            .bearer_auth(token("box", s)?)
            .multipart(form),
    )?;
    Ok(
        json!({"provider":"box","id":text(&value["entries"][0],"id")?,"name":filename,"message":"BoxへZIPをアップロードしました"}),
    )
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
    if !action.starts_with("cloud_") {
        return None;
    }
    Some((|| {
        let settings = app
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .machine_settings
            .settings
            .clone();
        match action {
            "cloud_status" => Ok(
                json!({"onedrive":token("onedrive",&settings).is_ok(),"box":token("box",&settings).is_ok(),"box_redirect_uri":BOX_REDIRECT}),
            ),
            "cloud_auth_start" => match text(payload, "provider")? {
                "onedrive" => begin_microsoft(&settings),
                "box" => begin_box(&settings, payload["secret"].as_str().unwrap_or("")),
                _ => Err("認証先が不正です".into()),
            },
            "cloud_auth_poll" => match text(payload, "provider")? {
                "onedrive" => poll_microsoft(),
                "box" => poll_box(),
                _ => Err("認証先が不正です".into()),
            },
            "cloud_auth_cancel" => {
                let mut guard = session().lock().map_err(|e| e.to_string())?;
                match payload["provider"].as_str() {
                    Some("onedrive") => guard.device = None,
                    Some("box") => guard.box_flow = None,
                    _ => {
                        guard.device = None;
                        guard.box_flow = None;
                    }
                }
                Ok(Value::Null)
            }
            "cloud_logout" => {
                *session().lock().map_err(|e| e.to_string())? = Session::default();
                Ok(Value::Null)
            }
            "cloud_open_auth" => {
                let url = text(payload, "url")?;
                let parsed = Url::parse(url).map_err(|_| "URLが不正です")?;
                if parsed.scheme() != "https"
                    || !matches!(parsed.host_str(), Some("microsoft.com" | "account.box.com"))
                {
                    return Err("認証URLが不正です".into());
                }
                open_url(url)?;
                Ok(Value::Null)
            }
            "cloud_upload" => {
                let provider = text(payload, "provider")?;
                let project = {
                    let guard = app.0.lock().map_err(|e| e.to_string())?;
                    let store = guard.store.as_ref().ok_or("プロジェクトを開いてください")?;
                    if payload["project_id"].as_str() != Some(store.project.id.as_str()) {
                        return Err("プロジェクトが変わっています".into());
                    }
                    store.project.clone()
                };
                let bytes = script_bytes(
                    &project,
                    text(payload, "source_id")?,
                    text(payload, "exported_at")?,
                )?;
                let filename = format!("ScriptSync-{}.zip", crate::domain::id());
                match provider {
                    "web" => upload_web_with(
                        |v| send_json(client()?.post(WEB_API).json(&v)),
                        &bytes,
                        text(payload, "hn")?,
                        text(payload, "key")?,
                    ),
                    "onedrive" => upload_onedrive(bytes, &settings, &filename),
                    "box" => upload_box(bytes, &settings, &filename),
                    _ => Err("送信先が不正です".into()),
                }
            }
            _ => Err("クラウド操作が不正です".into()),
        }
    })())
}
fn open_url(url: &str) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let value: Vec<u16> = std::ffi::OsStr::new(url)
            .encode_wide()
            .chain(Some(0))
            .collect();
        let result = unsafe {
            windows::Win32::UI::Shell::ShellExecuteW(
                None,
                windows::core::w!("open"),
                windows::core::PCWSTR(value.as_ptr()),
                None,
                None,
                windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err("ブラウザーを開けません".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("Windows環境で使用してください".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn oauth_callback_requires_matching_state_and_single_code() {
        assert_eq!(
            callback_code("/callback?state=expected&code=abc", "expected").unwrap(),
            "abc"
        );
        for path in [
            "/callback?state=other&code=abc",
            "/callback?state=expected&code=abc&code=def",
            "/other?state=expected&code=abc",
            "/callback?state=expected&error=access_denied",
        ] {
            assert!(callback_code(path, "expected").is_err());
        }
    }
    #[test]
    fn missing_cloud_registration_fails_without_network() {
        let settings = Settings::default();
        assert!(microsoft_endpoint(&settings).is_err());
        assert!(begin_box(&settings, "secret").is_err());
        assert!(web_token("", "123456").is_err());
        assert!(web_token("fixture", "12345x").is_err());
    }
    #[test]
    fn graph_path_encodes_folder_segments_without_changing_endpoint() {
        let url = onedrive_url("Script Sync/#draft", "unique.zip").unwrap();
        assert_eq!(
            url.as_str(),
            "https://graph.microsoft.com/v1.0/me/drive/root:/Script%20Sync/%23draft/unique.zip:/content"
        );
        assert!(onedrive_url("../other", "unique.zip").is_err());
        assert!(onedrive_url("C:\\private", "unique.zip").is_err());
    }
}
