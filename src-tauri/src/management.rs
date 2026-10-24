//! KotorinToys管理契約2。パイプの低層はEveryDBの同契約を移植した。
mod pipe;

use crate::app::App;
use anyhow::{ensure, Context};
use serde_json::{json, Value};
use std::sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const TOOL_ID: &str = "script-sync";
const TIMEOUT: Duration = Duration::from_secs(2);

pub struct Operation(App);
impl Drop for Operation {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0.0.lock() {
            state.management_operations = state.management_operations.saturating_sub(1);
        }
    }
}
pub fn begin_operation(app: &App) -> Result<Operation, String> {
    let mut state = app.0.lock().map_err(|_| "処理状態を確認できません")?;
    if state.management_stopping { return Err("正常終了処理中です".into()); }
    state.management_operations += 1;
    Ok(Operation(app.clone()))
}
fn commit_stop(app: &App) -> bool {
    let Ok(mut state) = app.0.try_lock() else { return false };
    if state.management_stopping || state.management_operations != 0 || state.busy || state.stop.is_some()
        || state.queue_stop.is_some() || state.auto_asr_running || !state.auto_asr_queue.is_empty() {
        return false;
    }
    state.management_stopping = true;
    true
}
fn rollback_stop(app: &App) {
    if let Ok(mut state) = app.0.lock() { state.management_stopping = false; }
}

struct PendingStop { id: String, deadline: Instant, sender: mpsc::SyncSender<bool> }
pub struct Management {
    ready: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    pending: Arc<Mutex<Option<PendingStop>>>,
}
impl Management {
    pub fn start(app: AppHandle) -> anyhow::Result<Self> {
        let ready = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));
        let pending = Arc::new(Mutex::new(None));
        let (server_ready, server_shutdown, server_pending) = (ready.clone(), shutdown.clone(), pending.clone());
        let (tx, rx) = mpsc::sync_channel(1);
        let origin = launch_origin(&std::env::args().collect::<Vec<_>>());
        std::thread::Builder::new().name("scriptsync-management".into()).spawn(move || {
            let server = match pipe::ManagementPipe::create(TOOL_ID) {
                Ok(server) => { let _ = tx.send(Ok(())); server }
                Err(error) => { let _ = tx.send(Err(error.to_string())); return; }
            };
            while !server_shutdown.load(Ordering::Acquire) {
                if server.accept(Duration::from_millis(100)).is_err() { continue; }
                let deadline = Instant::now() + TIMEOUT;
                let stop = handle_request(&server, &app, &server_ready, &server_pending, &origin, deadline);
                server.wait_for_client_close(deadline);
                server.disconnect();
                if stop {
                    let app_for_ui = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        crate::hotkeys::stop();
                        if let Some(window) = app_for_ui.get_webview_window("main") { crate::window_state::save(&window); }
                        let state = app_for_ui.state::<App>();
                        if let Ok(mut inner) = state.0.lock() { inner.native_playback.close(); inner.cue_playback.close(); }
                        app_for_ui.exit(0);
                    });
                    break;
                }
            }
        }).context("管理スレッドを起動できません")?;
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(Self { ready, shutdown, pending }),
            outcome => { shutdown.store(true, Ordering::Release); anyhow::bail!("管理窓口を作成できません: {outcome:?}") }
        }
    }
    pub fn ui_lost(&self) { self.ready.store(false, Ordering::Release); }
}
impl Drop for Management { fn drop(&mut self) { self.shutdown.store(true, Ordering::Release); } }

#[tauri::command]
pub fn management_ui_ready(state: tauri::State<'_, Management>) { state.ready.store(true, Ordering::Release); }

#[tauri::command]
pub fn management_stop_reply(id: String, allowed: bool, management: tauri::State<'_, Management>, app: tauri::State<'_, App>) -> bool {
    let Ok(mut pending) = management.pending.lock() else { return false };
    if !pending.as_ref().is_some_and(|request| request.id == id && Instant::now() < request.deadline) { return false; }
    let request = pending.take().unwrap();
    let accepted = allowed && commit_stop(&app);
    if request.sender.send(accepted).is_err() && accepted { rollback_stop(&app); return false; }
    accepted
}

fn launch_origin(args: &[String]) -> String {
    args.iter().position(|arg| arg == "--launch-origin")
        .and_then(|index| args.get(index + 1))
        .filter(|origin| matches!(origin.as_str(), "host_manual" | "via_kotorintoys" | "direct_task"))
        .cloned().unwrap_or_else(|| "standalone".into())
}
fn version() -> String {
    let value: Value = serde_json::from_str(include_str!("../../dev/msix-version.json")).unwrap();
    format!("{}.{}", value["applicationVersion"].as_str().unwrap(), value["revision"].as_u64().unwrap())
}
fn parse_request(line: &str) -> anyhow::Result<(String, String, Option<u32>)> {
    ensure!(line.len() <= 8192, "管理要求が長すぎます");
    if line == "status" { return Ok(("status".into(), String::new(), None)); }
    let value: Value = serde_json::from_str(line)?;
    let command = value["command"].as_str().context("操作がありません")?.to_owned();
    let id = value.get("requestId").map(|v| v.as_str().context("要求IDが不正です")).transpose()?.unwrap_or("");
    ensure!(id.len() <= 128, "要求IDが長すぎます");
    let pid = value["expectedPid"].as_u64().and_then(|pid| u32::try_from(pid).ok());
    Ok((command, id.into(), pid))
}
fn handle_request(server: &pipe::ManagementPipe, app: &AppHandle, ready: &AtomicBool,
    pending: &Mutex<Option<PendingStop>>, origin: &str, deadline: Instant) -> bool {
    let parsed = server.read_line(deadline).and_then(|line| parse_request(&line));
    let (command, request_id, expected_pid) = parsed.as_ref().cloned().unwrap_or_default();
    let is_ready = ready.load(Ordering::Acquire);
    let (mut result, mut detail) = ("completed", "");
    let mut stop = false;
    if !server.same_session().unwrap_or(false) { result = "refused"; detail = "別の対話セッションです。"; }
    else if parsed.is_err() { result = "failed"; detail = "管理要求が不正です。"; }
    else {
        match command.as_str() {
            "status" => {},
            "show" | "stop" if expected_pid != Some(std::process::id()) => { result = "refused"; detail = "対象PIDが一致しません。"; },
            "show" | "stop" if !is_ready => { result = "refused"; detail = "画面の準備が完了していません。"; },
            "show" => if !show_on_main_thread(app, deadline) { result = "failed"; detail = "画面表示が完了しませんでした。"; },
            "stop" => {
                let id = uuid::Uuid::new_v4().to_string();
                let (tx, rx) = mpsc::sync_channel(1);
                *pending.lock().unwrap() = Some(PendingStop { id: id.clone(), deadline, sender: tx });
                stop = app.emit("management-stop-requested", &id).is_ok()
                    && rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap_or(false);
                *pending.lock().unwrap() = None;
                if stop { result = "accepted"; } else { result = "refused"; detail = "編集中・処理中、または画面の終了確認に応答がありません。作業を完了してから再試行してください。"; }
            },
            _ => { result = "unsupported"; detail = "未対応の操作です。"; }
        }
    }
    let response = json!({"protocolVersion":2,"requestId":request_id,"pid":std::process::id(),"ready":is_ready,
        "version":version(),"launchOrigin":origin,"capabilities":["status","show","stop"],"result":result,"detail":detail});
    let written = server.write_line(&response.to_string(), deadline).is_ok();
    if command == "stop" && (!stop || !written) {
        rollback_stop(&app.state::<App>());
        let _ = app.emit("management-stop-cancelled", ());
    }
    stop && written
}
fn show_on_main_thread(app: &AppHandle, deadline: Instant) -> bool {
    let (tx, rx) = mpsc::sync_channel(1);
    let for_ui = app.clone();
    if app.run_on_main_thread(move || {
        let shown = Instant::now() < deadline && for_ui.get_webview_window("main").is_some_and(|window|
            window.unminimize().is_ok() && window.show().is_ok() && window.set_focus().is_ok());
        let _ = tx.send(shown);
    }).is_err() { return false; }
    rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pid_requests_and_limits() {
        assert_eq!(parse_request("status").unwrap().0, "status");
        assert_eq!(parse_request(r#"{"command":"stop","expectedPid":42,"requestId":"abc"}"#).unwrap(), ("stop".into(), "abc".into(), Some(42)));
        for invalid in ["{".into(), "x".repeat(8193), format!(r#"{{"command":"show","requestId":"{}"}}"#, "x".repeat(129))] { assert!(parse_request(&invalid).is_err()); }
    }
    #[test]
    fn shutdown_is_atomic_and_rejects_jobs_and_new_commands() {
        let app = App::default();
        let operation = begin_operation(&app).unwrap();
        assert!(!commit_stop(&app)); drop(operation);
        { let mut state = app.0.lock().unwrap(); state.busy = true; }
        assert!(!commit_stop(&app)); app.0.lock().unwrap().busy = false;
        app.0.lock().unwrap().auto_asr_running = true;
        assert!(!commit_stop(&app)); app.0.lock().unwrap().auto_asr_running = false;
        app.0.lock().unwrap().queue_stop = Some(Arc::new(AtomicBool::new(false)));
        assert!(!commit_stop(&app)); app.0.lock().unwrap().queue_stop = None;
        app.0.lock().unwrap().stop = Some(Arc::new(AtomicBool::new(false)));
        assert!(!commit_stop(&app)); app.0.lock().unwrap().stop = None;
        assert!(commit_stop(&app)); assert!(begin_operation(&app).is_err());
        rollback_stop(&app); assert!(begin_operation(&app).is_ok());
    }
    #[test]
    fn held_backend_lock_never_blocks_stop() {
        let app = App::default(); let _lock = app.0.lock().unwrap(); assert!(!commit_stop(&app));
    }
}
