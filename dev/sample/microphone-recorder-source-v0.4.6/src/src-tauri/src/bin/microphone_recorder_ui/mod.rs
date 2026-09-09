use super::*;
use serde::Deserialize;
#[cfg(test)]
mod tests;
use serde_json::{json, Value};
use std::{
    io::Read,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
};

struct State {
    session: Session,
    directory: PathBuf,
    devices: Vec<AudioDeviceInfo>,
    duration_ms: u64,
    recording: Option<Arc<AtomicBool>>,
    prompt: Option<usize>,
    levels: (f32, f32, u64),
    message: String,
}
#[derive(Deserialize)]
struct Command {
    action: String,
    index: Option<usize>,
    endpoint: Option<String>,
}

pub fn run(
    directory: PathBuf,
    name: Option<String>,
    endpoint: Option<String>,
    duration_ms: u64,
) -> Result<(), String> {
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    let devices = list_devices(DeviceDirection::Capture).map_err(|e| e.to_string())?;
    let mut session = if directory.join("session.json").exists() {
        Session::load(&directory)?
    } else {
        Session::new(String::new())
    };
    if name.is_some() || endpoint.is_some() {
        session.endpoint_id = select_device(&devices, endpoint.as_deref(), name.as_deref())?
            .endpoint_id
            .clone();
    }
    let state = Arc::new(Mutex::new(State {
        session,
        directory,
        devices,
        duration_ms,
        recording: None,
        prompt: None,
        levels: (0., 0., 0),
        message: "マイクを選んで課題の録音ボタンを押してください。".into(),
    }));
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
    let host = listener
        .local_addr()
        .map_err(|e| e.to_string())?
        .to_string();
    let token = uuid::Uuid::new_v4().to_string();
    let url = format!("http://{host}/#{token}");
    println!(
        "クリック収録画面: {url}\n終了は画面の「終了」ボタン。番号操作は --interactive --console。"
    );
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("rundll32.exe")
            .arg("url.dll,FileProtocolHandler")
            .arg(&url)
            .spawn();
    }
    let quit = AtomicBool::new(false);
    for stream in listener.incoming() {
        let mut stream = stream.map_err(|e| e.to_string())?;
        let response =
            handle(&mut stream, &host, &token, &state, &quit).unwrap_or_else(|message| {
                (
                    400,
                    "application/json",
                    serde_json::to_vec(&json!({"error":message})).unwrap(),
                )
            });
        if let Err(error) = write_response(&mut stream, response) {
            eprintln!("画面への応答を終了しました: {error}");
        }
        if quit.load(Ordering::Relaxed) {
            break;
        }
    }
    // 終了時に録音workerがWAVと台帳を確定するまで待つ。
    loop {
        let stop = state.lock().unwrap().recording.clone();
        if let Some(stop) = stop {
            stop.store(true, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(25))
        } else {
            break;
        }
    }
    Ok(())
}

type Response = (u16, &'static str, Vec<u8>);
fn write_response(stream: &mut TcpStream, (status, mime, bytes): Response) -> std::io::Result<()> {
    write!(stream,"HTTP/1.1 {status} OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; media-src blob:; connect-src 'self'; frame-ancestors 'none'\r\n\r\n",bytes.len())?;
    stream.write_all(&bytes)
}
fn handle(
    stream: &mut TcpStream,
    host: &str,
    token: &str,
    state: &Arc<Mutex<State>>,
    quit: &AtomicBool,
) -> Result<Response, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    let mut buffer = [0u8; 4096];
    let header_end = loop {
        let count = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("空の要求".into());
        }
        data.extend_from_slice(&buffer[..count]);
        if data.len() > 32768 {
            return Err("要求が大きすぎます".into());
        }
        if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let head = String::from_utf8_lossy(&data[..header_end]);
    let mut lines = head.lines();
    let request = lines.next().unwrap_or("").to_string();
    let headers: Vec<_> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let header = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    if header("host") != Some(host) {
        return Err("接続先が一致しません".into());
    }
    if request == "GET / HTTP/1.1" {
        return Ok((
            200,
            "text/html; charset=utf-8",
            include_bytes!("index.html").to_vec(),
        ));
    }
    if request != "POST /api HTTP/1.1" || header("x-recorder-token") != Some(token) {
        return Err("操作トークンが一致しません".into());
    }
    if let Some(origin) = header("origin") {
        if origin != format!("http://{host}") {
            return Err("外部ページからの操作はできません".into());
        }
    }
    let length = header("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or("要求サイズがありません")?;
    if length > 8192 {
        return Err("要求が大きすぎます".into());
    }
    while data.len() < header_end + length {
        let count = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("要求が途中で終了しました".into());
        }
        data.extend_from_slice(&buffer[..count]);
        if data.len() > 32768 {
            return Err("要求が大きすぎます".into());
        }
    }
    let command: Command = serde_json::from_slice(&data[header_end..header_end + length])
        .map_err(|e| e.to_string())?;
    execute(command, state, quit)
}
fn json_response(value: Value) -> Response {
    (200, "application/json", serde_json::to_vec(&value).unwrap())
}
fn safe_audio(root: &std::path::Path, file: &str) -> Result<PathBuf, String> {
    let path = sample_path(root, file)?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !path.starts_with(root) {
        return Err("保存先外の音声は開けません".into());
    }
    Ok(path)
}
fn execute(
    command: Command,
    state: &Arc<Mutex<State>>,
    quit: &AtomicBool,
) -> Result<Response, String> {
    let mut current = state.lock().map_err(|e| e.to_string())?;
    if command.action == "state" {
        return Ok(json_response(
            json!({"session":current.session,"devices":current.devices,"recording":current.prompt,"levels":current.levels,"message":current.message,"directory":current.directory}),
        ));
    }
    if command.action == "stop" || command.action == "quit" {
        if let Some(stop) = &current.recording {
            stop.store(true, Ordering::Relaxed)
        }
        if command.action == "quit" {
            quit.store(true, Ordering::Relaxed)
        }
        return Ok(json_response(json!({"ok":true})));
    }
    if current.recording.is_some() {
        return Err("録音停止後に操作してください".into());
    }
    if command.action == "device" {
        let endpoint = command.endpoint.ok_or("マイクを指定してください")?;
        select_device(&current.devices, Some(&endpoint), None)?;
        current.session.endpoint_id = endpoint;
        current.session.save(&current.directory)?;
        return Ok(json_response(json!({"ok":true})));
    }
    let index = command.index.ok_or("課題番号がありません")?;
    if index >= current.session.prompts.len() {
        return Err("課題番号が範囲外です".into());
    }
    if command.action == "record" {
        select_device(&current.devices, Some(&current.session.endpoint_id), None)?;
        let stop = Arc::new(AtomicBool::new(false));
        current.recording = Some(stop.clone());
        current.prompt = Some(index);
        current.levels = (0., 0., 0);
        current.message = "録音中：停止ボタンで保存します。".into();
        let endpoint = current.session.endpoint_id.clone();
        let root = current.directory.clone();
        let duration = current.duration_ms;
        let low = current.session.low_level_db;
        let room = current.session.prompts[index].room_tone;
        let state = Arc::clone(state);
        std::thread::spawn(move || record(state, stop, root, endpoint, duration, index, low, room));
        return Ok(json_response(json!({"ok":true})));
    }
    let root = current.directory.clone();
    let silence = current.session.silence.clone();
    let prompt = &mut current.session.prompts[index];
    let take = prompt
        .takes
        .iter_mut()
        .rev()
        .find(|take| take.status != "failed")
        .ok_or("録音がありません")?;
    match command.action.as_str() {
        "audio" | "derived_audio" => {
            let file = if command.action == "audio" {
                &take.file
            } else {
                &take.derived.last().ok_or("短縮版がありません")?.file
            };
            let bytes = std::fs::read(safe_audio(&root, file)?).map_err(|e| e.to_string())?;
            return Ok((200, "audio/wav", bytes));
        }
        "shorten" => {
            let file = format!("short-{}.wav", uuid::Uuid::new_v4());
            let edit =
                derive_shortened(&safe_audio(&root, &take.file)?, &root.join(&file), &silence)?;
            take.derived.push(Derived {
                file,
                source_take_id: take.id.clone(),
                edit,
            });
        }
        "accept" => {
            prompt.accepted_take = Some(take.id.clone());
        }
        _ => return Err("不明な操作です".into()),
    }
    current.session.save(&root)?;
    Ok(json_response(json!({"ok":true})))
}
fn record(
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    root: PathBuf,
    endpoint: String,
    duration: u64,
    index: usize,
    low: f32,
    room: bool,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let file = format!("prompt-{}-{id}.wav", index + 1);
    let request = MicrophoneRecordingRequest::new(&endpoint, root.join(&file), duration);
    let result = record_microphone_to_wav_controlled(&request, &stop, |peak, rms, elapsed| {
        if let Ok(mut current) = state.lock() {
            current.levels = (peak, rms, elapsed)
        }
    });
    let mut take = Take {
        id: id.clone(),
        file: file.clone(),
        endpoint_id: endpoint,
        recorded_at: chrono::Utc::now().to_rfc3339(),
        duration_ms: duration,
        status: "failed".into(),
        quality: None,
        error: None,
        derived: Vec::new(),
    };
    match result {
        Ok(report) => {
            take.duration_ms = report.wav.audio_duration_ms;
            match read_wav(&root.join(&file)) {
                Ok((_, samples)) => {
                    let q = quality(&samples, low, room);
                    take.status = if q.low_level_warning || q.clipped_samples > 0 {
                        "warning"
                    } else {
                        "success"
                    }
                    .into();
                    take.quality = Some(q)
                }
                Err(error) => take.error = Some(error),
            }
        }
        Err(error) => take.error = Some(error.to_string()),
    }
    let mut current = state.lock().unwrap();
    if take.status == "success" {
        current.session.prompts[index].accepted_take = Some(id)
    }
    current.message = take.error.clone().unwrap_or_else(|| {
        if take.status == "warning" {
            "品質警告：再録音または採用を選んでください。"
        } else {
            "保存しました。"
        }
        .into()
    });
    current.session.prompts[index].takes.push(take);
    if let Err(error) = current.session.save(&root) {
        current.message = format!("台帳保存失敗: {error}")
    }
    current.recording = None;
    current.prompt = None;
}
