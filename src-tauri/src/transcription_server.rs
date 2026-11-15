use crate::{domain::Result, storage::hash_bytes};
use reqwest::{
    StatusCode, Url,
    blocking::{Client, RequestBuilder, Response, multipart},
    header::HeaderValue,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{io::Read, path::Path, time::Duration};

#[cfg(test)]
#[path = "transcription_server_tests.rs"]
mod tests;

pub const DEFAULT_URL: &str = "http://127.0.0.1:8769";
pub const DEFAULT_MODEL: &str = "whisper/tiny";
pub const PROVIDER: &str = "transcription_server";
const MAX_AUDIO_BYTES: u64 = 64 * 1024 * 1024;

pub fn parse_url(value: &str) -> Result<Url> {
    let error =
        || "TranscriptionServerの接続先はHTTPまたはHTTPSのオリジンを指定してください".to_owned();
    if value.len() > 2048 || value.chars().any(char::is_whitespace) {
        return Err(error());
    }
    let url = Url::parse(value).map_err(|_| error())?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.has_host()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(error());
    }
    Ok(url)
}

fn model_part(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

pub fn valid_model(value: &str) -> bool {
    value.len() <= 200
        && value
            .split_once('/')
            .is_some_and(|(engine, model)| model_part(engine) && model_part(model))
}

fn response(request: RequestBuilder, operation: &str, timeout: Duration) -> Result<Value> {
    let mut response = request.timeout(timeout).send().map_err(|error| {
        let reason = if error.is_timeout() {
            "時間切れです"
        } else {
            "接続できません"
        };
        format!("TranscriptionServerの{operation}に失敗しました。{reason}。")
    })?;
    if !response.status().is_success() {
        return Err(http_error(&response, operation));
    }
    let mut body = Vec::new();
    response
        .by_ref()
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut body)
        .map_err(|_| format!("TranscriptionServerの{operation}の応答を読み取れませんでした"))?;
    if body.len() > 2 * 1024 * 1024 {
        return Err(format!(
            "TranscriptionServerの{operation}の応答が大きすぎます"
        ));
    }
    serde_json::from_slice(&body)
        .map_err(|_| format!("TranscriptionServerの{operation}の応答形式が不正です"))
}

fn http_error(response: &Response, operation: &str) -> String {
    let status = response.status();
    let reason = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            "認証または接続元の許可を確認してください"
        }
        StatusCode::CONFLICT | StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE => {
            "サーバーが受付可能になってから再試行してください"
        }
        StatusCode::PAYLOAD_TOO_LARGE => "サーバーの音声サイズ上限を超えています",
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
            "音声形式とサーバー側のモデル設定を確認してください"
        }
        _ => "サーバーの状態を確認してください",
    };
    format!(
        "TranscriptionServerの{operation}に失敗しました（HTTP {}）。{reason}。",
        status.as_u16()
    )
}

#[derive(Serialize)]
pub struct Model {
    id: String,
    availability: String,
}

#[derive(Serialize)]
pub struct ServerInfo {
    version: String,
    auth_mode: &'static str,
    protocol: u64,
    models: Vec<Model>,
}

struct Connection {
    client: Client,
    url: Url,
    token: HeaderValue,
    info: ServerInfo,
}

impl Connection {
    fn open(value: &str) -> Result<Self> {
        let url = parse_url(value)?;
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| "TranscriptionServerへの接続を初期化できませんでした")?;
        let timeout = Duration::from_secs(10);
        let health = response(
            client.get(url.join("/api/health").unwrap()),
            "状態確認",
            timeout,
        )?;
        let version = health["serverVersion"]
            .as_str()
            .filter(|value| {
                let mut parts = value.split('.');
                let valid =
                    (0..3).all(|_| parts.next().is_some_and(|part| part.parse::<u32>().is_ok()));
                valid && parts.next().is_none()
            })
            .ok_or("TranscriptionServerのバージョンを確認できませんでした")?;
        if health["ok"] != true
            || health["protocolVersion"] != 3
            || health["capabilities"]["openaiAudio"] != true
            || health["capabilities"]["modelDiscovery"] != true
        {
            return Err(
                "TranscriptionServerのプロトコル3または音声認識APIに対応していません".into(),
            );
        }
        let auth_mode = match health["authMode"].as_str() {
            Some("local") => "local",
            Some("lan") => "lan",
            _ => return Err("TranscriptionServerの認証方式が未対応です。同じPCまたは信頼LANのサーバーを指定してください".into()),
        };
        if health["ready"] != true || health["acceptingJobs"] != true {
            return Err("TranscriptionServerは文字起こしを受け付けられる状態ではありません。サーバーの管理画面で準備と受付状態を確認してください".into());
        }
        let mut token = health["token"]
            .as_str()
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 1024
                    && value.bytes().all(|b| b.is_ascii_graphic())
            })
            .and_then(|value| HeaderValue::from_str(value).ok())
            .ok_or("TranscriptionServerの一時トークンを取得できませんでした")?;
        token.set_sensitive(true);

        let catalog = response(
            client.get(url.join("/v1/models").unwrap()),
            "モデル一覧の取得",
            timeout,
        )?;
        let rows = catalog["data"]
            .as_array()
            .filter(|_| catalog["object"] == "list")
            .ok_or("TranscriptionServerのモデル一覧の応答形式が不正です")?;
        let models = rows.iter().filter_map(available_model).collect();
        Ok(Self {
            client,
            url,
            token,
            info: ServerInfo {
                version: version.to_owned(),
                auth_mode,
                protocol: 3,
                models,
            },
        })
    }
}

fn available_model(row: &Value) -> Option<Model> {
    let id = row["id"].as_str().filter(|id| valid_model(id))?;
    let (engine, model) = id.split_once('/')?;
    let info = &row["x_meeting"];
    let availability = info["availability"].as_str()?;
    if info["engineId"] != engine
        || info["modelId"] != model
        || info["task"] != "transcription"
        || info["runtimeReady"] != true
        || info["supported"] != true
        || info["manualInstallRequired"] == true && info["modelInstalled"] != true
        || !matches!(availability, "installed" | "download_on_first_use")
        || [
            &info["supportedLanguages"],
            &info["capabilities"]["languages"],
        ]
        .iter()
        .filter_map(|languages| languages.as_array())
        .any(|languages| {
            !languages.iter().any(|lang| {
                lang.as_str().is_some_and(|code| {
                    code.split('-')
                        .next()
                        .is_some_and(|value| value.eq_ignore_ascii_case("ja"))
                })
            })
        })
    {
        return None;
    }
    Some(Model {
        id: id.to_owned(),
        availability: availability.to_owned(),
    })
}

pub fn check(url: &str) -> Result<ServerInfo> {
    Ok(Connection::open(url)?.info)
}

pub fn transcribe(url: &str, model: &str, input: &Path, expected_hash: &str) -> Result<Value> {
    let mut audio = Vec::new();
    std::fs::File::open(input)
        .and_then(|file| file.take(MAX_AUDIO_BYTES + 1).read_to_end(&mut audio))
        .map_err(|_| "テイクの音声ファイルを読み込めませんでした")?;
    if audio.len() as u64 > MAX_AUDIO_BYTES {
        return Err("TranscriptionServerに送る音声は64 MiB以内にしてください".into());
    }
    if hash_bytes(&audio) != expected_hash {
        return Err("テイクの音声が変更されています".into());
    }
    let connection = Connection::open(url)?;
    if !connection.info.models.iter().any(|row| row.id == model) {
        return Err("選択したモデルはTranscriptionServerで日本語の文字起こしに使用できません。設定の接続確認でモデルを選び直してください".into());
    }
    let part = multipart::Part::bytes(audio)
        .file_name("take.wav")
        .mime_str("audio/wav")
        .map_err(|_| "音声送信の準備に失敗しました")?;
    let form = multipart::Form::new()
        .part("file", part)
        .text("model", model.to_owned())
        .text("language", "ja")
        .text("response_format", "json");
    let result = response(
        connection
            .client
            .post(connection.url.join("/v1/audio/transcriptions").unwrap())
            .header("X-Local-Token", connection.token)
            .multipart(form),
        "文字起こし",
        Duration::from_secs(600),
    )?;
    let text = result["text"]
        .as_str()
        .ok_or("TranscriptionServerの結果に文字起こし本文がありません")?
        .trim();
    if text.is_empty() {
        return Err("TranscriptionServerが発話を認識できませんでした".into());
    }
    Ok(json!({"raw":text,"provider":PROVIDER,"model":model}))
}
