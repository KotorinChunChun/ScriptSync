//! ウィンドウ・WebView・管理パイプを作らず、通常のアプリ処理を呼ぶCLI。
use crate::{app::App, machine_settings::MachineSettings, project_location::ProjectLocation};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io::{self, BufRead, Write}, path::PathBuf, thread, time::{Duration, Instant}};

const MAX_LINE: usize = 1024 * 1024;
const MAX_WAIT: u64 = 640_000;

#[derive(Deserialize)]
struct Request {
    #[serde(default)] id: Value,
    action: String,
    #[serde(default)] payload: Value,
}

fn line(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, String> {
    let mut result = Vec::new();
    loop {
        let buffer = reader.fill_buf().map_err(|_| "入力を読めません")?;
        if buffer.is_empty() { return Ok((!result.is_empty()).then_some(result)); }
        let end = buffer.iter().position(|b| *b == b'\n').map(|n| n + 1);
        let count = end.unwrap_or(buffer.len());
        if result.len() + count > MAX_LINE { return Err("1コマンドは1MiB以内で指定してください".into()); }
        result.extend_from_slice(&buffer[..count]);
        reader.consume(count);
        if end.is_some() { return Ok(Some(result)); }
    }
}

fn idle(app: &App, timeout: u64) -> Result<Value, String> {
    let deadline = Instant::now() + Duration::from_millis(timeout.min(MAX_WAIT));
    loop {
        let busy = {
            let state = app.0.lock().map_err(|_| "状態を取得できません")?;
            state.busy || state.stop.is_some() || state.queue_stop.is_some()
                || state.auto_asr_running || !state.auto_asr_queue.is_empty()
                || state.machine_settings.setup.running
        };
        if !busy { return Ok(json!({"idle":true})); }
        if Instant::now() >= deadline { return Err("処理の完了待ちが期限切れです".into()); }
        thread::sleep(Duration::from_millis(20));
    }
}

fn execute(app: &App, request: &Request) -> Result<Value, String> {
    let action = request.action.as_str();
    if matches!(action, "wait_idle" | "shutdown") {
        if action == "shutdown" { app.dispatch("stop", json!({}))?; }
        return idle(app, request.payload["timeout_ms"].as_u64().unwrap_or(MAX_WAIT));
    }
    if matches!(action, "open_settings_link" | "open_export_folder") || action.starts_with("ui_") {
        return Err("CLIでは外部画面とUIセッションを開きません".into());
    }
    if matches!(action, "new" | "open" | "import_script" | "import_audio" | "exchange_inspect"
        | "exchange_export" | "download_take" | "talk_inspect") {
        let path = request.payload["path"].as_str().map(PathBuf::from)
            .filter(|path| path.is_absolute()).ok_or("CLIではファイルの絶対パスが必要です")?;
        if matches!(action, "exchange_export" | "download_take") && path.exists() {
            return Err("CLIの出力先ファイルは新しい名前を指定してください".into());
        }
    }
    if action == "markdown_import" && !request.payload["paths"].is_array() {
        return Err("CLIではpathsにファイル一覧が必要です".into());
    }
    if matches!(action,"markdown_import" | "markdown_drop") {
        let paths=request.payload["paths"].as_array().ok_or("ファイル一覧が必要です")?;
        if paths.iter().any(|value| !value.as_str().is_some_and(|value| PathBuf::from(value).is_absolute())) {
            return Err("CLIではファイルの絶対パスが必要です".into());
        }
    }
    app.dispatch(action, request.payload.clone())
}

fn serve(app: &App, reader: &mut impl BufRead, output: &mut impl Write) -> Result<(), String> {
    while let Some(bytes) = line(reader)? {
        if bytes.iter().all(u8::is_ascii_whitespace) { continue; }
        let request = serde_json::from_slice::<Request>(&bytes);
        let (response, shutdown) = match request {
            Ok(request) => {
                let result = execute(app, &request);
                let shutdown = request.action == "shutdown" && result.is_ok();
                let response = match result {
                    Ok(value) => json!({"id":request.id,"ok":true,"result":value}),
                    Err(error) => json!({"id":request.id,"ok":false,"error":error}),
                };
                (response, shutdown)
            }
            Err(_) => (json!({"id":null,"ok":false,"error":"JSONコマンドが不正です"}), false),
        };
        serde_json::to_writer(&mut *output, &response).map_err(|_| "結果を書き出せません")?;
        output.write_all(b"\n").and_then(|_| output.flush()).map_err(|_| "結果を書き出せません")?;
        if shutdown { break; }
    }
    Ok(())
}

pub fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("scriptsync-cli --settings-dir 絶対パス\n標準入力: {{\"id\":1,\"action\":\"state\",\"payload\":{{}}}} を1行ずつ送信\n標準出力: id、okとresultまたはerror。wait_idleで非同期完了待ち、shutdownで正常終了。");
        return std::process::ExitCode::SUCCESS;
    }
    let result = (|| -> Result<(), String> {
        if args.len() != 2 || args[0] != "--settings-dir" {
            return Err("--settings-dirに隔離設定の絶対パスを指定してください（--helpで使い方）".into());
        }
        let directory = PathBuf::from(&args[1]);
        if !directory.is_absolute() { return Err("設定ディレクトリには絶対パスが必要です".into()); }
        std::fs::create_dir_all(directory.join("config")).map_err(|_| "設定ディレクトリを作成できません")?;
        let app = App::default();
        {
            let mut state = app.0.lock().map_err(|_| "状態を初期化できません")?;
            state.machine_settings = MachineSettings::load(directory.join("config/machine-settings.json"), directory.join("data/asr"));
            state.project_location = ProjectLocation::load(directory.join("config/読みあわせ/project-location.json"));
        }
        let result = serve(&app, &mut io::stdin().lock(), &mut io::stdout().lock());
        // EOFと出力エラーでも通常の停止処理で録音を確定し、バックグラウンド処理を待つ。
        let _ = app.dispatch("stop", json!({}));
        let stopped = idle(&app, MAX_WAIT);
        if let Ok(mut state) = app.0.lock() { state.native_playback.close(); state.cue_playback.close(); }
        result.and(stopped.map(|_| ()))
    })();
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => { eprintln!("CLI: {error}"); std::process::ExitCode::from(2) }
    }
}
