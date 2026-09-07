mod alignment;
mod app;
mod asr;
mod audio;
#[cfg(test)]
mod boundary_tests;
mod domain;
mod editing;
mod media;
mod queue;
mod speech;
mod storage;
#[cfg(test)]
mod workflow_tests;

#[tauri::command]
async fn command(
    state: tauri::State<'_, app::App>,
    action: String,
    payload: serde_json::Value,
) -> std::result::Result<serde_json::Value, String> {
    let app = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || app.dispatch(&action, payload))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn health_check() -> &'static str {
    "Rustバックエンドに接続しました。"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = app::App::default();
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--project") {
        if let Some(path) = args.get(i + 1) {
            match storage::Store::open(path.into(), false) {
                Ok(store) => app.0.lock().unwrap().store = Some(store),
                Err(e) => app.0.lock().unwrap().message = e,
            }
        }
    }
    tauri::Builder::default()
        .register_uri_scheme_protocol("scriptsync-audio", {
            let app = app.clone();
            move |_, request| media::response(&app, request)
        })
        .manage(app)
        .invoke_handler(tauri::generate_handler![health_check, command])
        .run(tauri::generate_context!())
        .expect("Tauriアプリケーションの起動に失敗しました");
}
