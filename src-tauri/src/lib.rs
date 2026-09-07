mod app;
mod audio;
mod domain;
mod speech;
mod storage;

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
    tauri::Builder::default()
        .manage(app::App::default())
        .invoke_handler(tauri::generate_handler![health_check, command])
        .run(tauri::generate_context!())
        .expect("Tauriアプリケーションの起動に失敗しました");
}
