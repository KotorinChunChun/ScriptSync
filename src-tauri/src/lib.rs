mod machine_settings;
mod text_improvement;
mod native_playback;
mod cloud_upload;
#[cfg(test)]
mod v004_tests;
mod asr_runtime;
mod export_settings;
mod exchange;
mod project_location;
mod project_open;
mod project_rename;
mod take_trim;
mod take_join;
mod ui_session;
use tauri::{Manager,Emitter};

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
fn close_application(window:tauri::WebviewWindow)->std::result::Result<(),String>{
    window_state::save(&window);
    hotkeys::stop();
    window.destroy().map_err(|e|e.to_string())
}
fn application_title(project_title: Option<&str>) -> String {
    let version: serde_json::Value = serde_json::from_str(include_str!("../../dev/msix-version.json"))
        .expect("MSIXバージョン情報が不正です");
    let version = format!("{}.{}", version["applicationVersion"].as_str().expect("アプリ版がありません"), version["revision"].as_u64().expect("再ビルド番号がありません"));
    let app_title = format!("読みあわせ (ScriptSync) {version}");
    match project_title {
        Some(title) => format!("{title} — {app_title}"),
        None => app_title,
    }
}

#[cfg(test)]
#[test]
fn application_title_keeps_distribution_version_with_or_without_project() {
    let info: serde_json::Value = serde_json::from_str(include_str!("../../dev/msix-version.json")).unwrap();
    let version = format!("{}.{}", info["applicationVersion"].as_str().unwrap(), info["revision"].as_u64().unwrap());
    assert_eq!(application_title(None), format!("読みあわせ (ScriptSync) {version}"));
    assert_eq!(application_title(Some("試験台本")), format!("試験台本 — 読みあわせ (ScriptSync) {version}"));
    let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert_eq!(config["app"]["windows"][0]["title"].as_str().unwrap(), application_title(None));
}

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
            "new" | "open" | "talk_inspect" | "import_script" | "import_audio" | "markdown_import" | "exchange_inspect" | "exchange_export" | "download_take"
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
    if result.is_ok() && matches!(title_action.as_str(), "new" | "open" | "project_rename") {
        if let Ok(snapshot) = title_app.snapshot() {
            if let Some(title) = snapshot["project"]["title"].as_str() {
                let _ = window.set_title(&application_title(Some(title)));
            }
        }
    }
    if result.is_ok() && title_action == "close_project" { let _ = window.set_title(&application_title(None)); }
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
    if let Some(path) = project_open::startup_path(&args) {
        {
            let path = std::path::PathBuf::from(path);
            let extension = path.extension().and_then(|extension| extension.to_str());
            let opened = if extension.is_some_and(|ext| ext.eq_ignore_ascii_case(storage::PROJECT_FILE_EXTENSION) || ext.eq_ignore_ascii_case("json"))
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
            application.state::<app::App>().0.lock().unwrap().machine_settings = machine_settings::MachineSettings::load(application.path().app_config_dir()?.join("machine-settings.json"), application.path().app_local_data_dir()?.join("asr"));
            let window = application
                .get_webview_window("main")
                .ok_or_else(|| std::io::Error::other("メインウィンドウが見つかりません"))?;
            window_state::restore_before_show(&window);
            window.set_title(&application_title(startup_title.as_deref()))?;
            let saved_window = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = saved_window.emit("app-close-requested", ());
                }
            });
            window.show()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            health_check,
            close_application,
            command,
            hotkeys::continuous_hotkeys
        ])
        .run(tauri::generate_context!())
        .expect("Tauriアプリケーションの起動に失敗しました");
}
