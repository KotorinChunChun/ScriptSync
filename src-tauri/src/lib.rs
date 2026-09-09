mod export_settings;
mod project_location;
mod take_trim;
mod ui_session;
use tauri::Manager;

mod alignment;
mod app;
mod asr;
mod automatic_asr;
mod archive;
mod script_sources;
mod audio;
#[cfg(test)]
mod boundary_tests;
mod domain;
mod editing;
mod export_blocks;
#[cfg(test)]
mod feature_tests;
mod hotkeys;
mod markdown;
// 実原稿によるアルゴリズム検証段階。画面との接続は受入条件を満たしてから行う。
#[cfg(test)]
mod script_merge;
#[cfg(test)]
mod merge_experiment_tests;
mod media;
mod queue;
mod rules;
mod speech;
mod storage;
mod talk_project;
mod window_state;
#[cfg(test)]
mod workflow_tests;

#[tauri::command]
async fn command(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, app::App>,
    action: String,
    payload: serde_json::Value,
) -> std::result::Result<serde_json::Value, String> {
    let app = state.inner().clone();
    let title_app = app.clone();
    let title_action = action.clone();
    // 音声デバイス列挙はMTAを使うため、STAが必要な選択画面にプールのスレッドを再利用しない。
    let result = tauri::async_runtime::spawn_blocking(move || {
        if matches!(
            action.as_str(),
            "new" | "open" | "talk_inspect" | "import_script" | "import_audio" | "markdown_import"
        ) {
            std::thread::Builder::new()
                .name("scriptsync-dialog".into())
                .spawn(move || app.dispatch(&action, payload))
                .map_err(|e| e.to_string())?
                .join()
                .map_err(|_| "ファイル選択処理が異常終了しました".to_string())?
        } else {
            app.dispatch(&action, payload)
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    if result.is_ok() && matches!(title_action.as_str(), "new" | "open") {
        if let Ok(snapshot) = title_app.snapshot() {
            if let Some(title) = snapshot["project"]["title"].as_str() {
                let _ = window.set_title(&format!("{title} — 読みあわせ"));
            }
        }
    }
    result
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
            let path = std::path::PathBuf::from(path);
            let extension = path.extension().and_then(|extension| extension.to_str());
            let opened = if extension == Some(storage::PROJECT_FILE_EXTENSION)
                || extension == Some("json")
            {
                storage::Store::open_project_file(path)
            } else {
                storage::Store::open(path, false)
            };
            match opened {
                Ok(store) => app.0.lock().unwrap().store = Some(store),
                Err(e) => app.0.lock().unwrap().message = e,
            }
        }
    }
    let startup_title = app.0.lock().ok().and_then(|state| {
        state
            .store
            .as_ref()
            .map(|store| store.project.title.clone())
    });
    ui_session::start_watchdog(&app);
    tauri::Builder::default()
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                ui_session::lost(&webview.state::<app::App>());
            }
        })
        .register_uri_scheme_protocol("scriptsync-audio", {
            let app = app.clone();
            move |_, request| media::response(&app, request)
        })
        .manage(app)
        .setup(move |application| {
            hotkeys::install(application.handle().clone());
            let config = application
                .path()
                .app_config_dir()?
                .join("読みあわせ/project-location.json");
            application
                .state::<app::App>()
                .0
                .lock()
                .unwrap()
                .project_location = project_location::ProjectLocation::load(config);
            let window = application
                .get_webview_window("main")
                .ok_or_else(|| std::io::Error::other("メインウィンドウが見つかりません"))?;
            window_state::restore_before_show(&window);
            if let Some(title) = &startup_title {
                window.set_title(&format!("{title} — 読みあわせ"))?;
            }
            let saved_window = window.clone();
            window.on_window_event(move |event| {
                if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                    window_state::save(&saved_window);
                    hotkeys::stop();
                }
            });
            window.show()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            command,
            hotkeys::continuous_hotkeys
        ])
        .run(tauri::generate_context!())
        .expect("Tauriアプリケーションの起動に失敗しました");
}
