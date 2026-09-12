//! 端末専用の設定。プロジェクトへ埋め込まず、音声デバイスの既定値を変更しない。
use crate::{app::App, domain::Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

pub const WEB_URL: &str = "https://dev-k.sakura.ne.jp/tools/scriptsyncweb/";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub capture_endpoint: String,
    pub render_endpoint: String,
    pub asr_provider: String,
    pub asr_model: String,
    pub codex_path: String,
    pub entra_tenant: String,
    pub entra_client_id: String,
    pub onedrive_folder: String,
    pub box_client_id: String,
    pub box_folder: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            capture_endpoint: String::new(),
            render_endpoint: String::new(),
            asr_provider: "local".into(),
            asr_model: "small".into(),
            codex_path: String::new(),
            entra_tenant: "a0fa18d4-3880-44a4-b5b2-bf59e18d7fc2".into(),
            entra_client_id: String::new(),
            onedrive_folder: "ScriptSync".into(),
            box_client_id: String::new(),
            box_folder: "0".into(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.asr_provider.as_str(), "local" | "codex") {
            return Err("文字起こし方式が不正です".into());
        }
        if !matches!(
            self.asr_model.as_str(),
            "tiny" | "base" | "small" | "medium" | "large-v3"
        ) {
            return Err("認識モデルが不正です".into());
        }
        if self.codex_path.contains(['\n', '\r', '\0']) {
            return Err("Codexのパスが不正です".into());
        }
        for text in [
            &self.capture_endpoint,
            &self.render_endpoint,
            &self.entra_tenant,
            &self.entra_client_id,
            &self.onedrive_folder,
            &self.box_client_id,
            &self.box_folder,
        ] {
            if text.len() > 2048 || text.contains(['\0', '\r', '\n']) {
                return Err("設定値が長すぎるか不正です".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct SetupStatus {
    pub running: bool,
    pub phase: String,
    pub message: String,
}
#[derive(Default)]
pub struct MachineSettings {
    pub settings: Settings,
    pub file: Option<PathBuf>,
    pub runtime_root: PathBuf,
    pub setup: SetupStatus,
    pub load_error: Option<String>,
}
impl MachineSettings {
    pub fn load(file: PathBuf, runtime_root: PathBuf) -> Self {
        let parsed = std::fs::read(&file)
            .map_err(|e| e.to_string())
            .and_then(|b| serde_json::from_slice::<Settings>(&b).map_err(|e| e.to_string()))
            .and_then(|s| {
                s.validate()?;
                Ok(s)
            });
        let load_error = if file.exists() {
            parsed.as_ref().err().cloned()
        } else {
            None
        };
        Self {
            settings: parsed.unwrap_or_default(),
            file: Some(file),
            runtime_root,
            load_error,
            setup: SetupStatus::default(),
        }
    }
    pub fn save(&mut self, settings: Settings) -> Result<()> {
        settings.validate()?;
        let file = self
            .file
            .as_ref()
            .ok_or("端末設定の保存先が初期化されていません")?;
        std::fs::create_dir_all(file.parent().ok_or("設定保存先が不正です")?)
            .map_err(|e| e.to_string())?;
        // 設定専用のatomic replacementを使用し、保存失敗時は旧設定を保持する。
        let temp = file.with_extension(format!("{}.tmp", crate::domain::id()));
        std::fs::write(
            &temp,
            serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        #[cfg(windows)]
        let result = {
            use std::os::windows::ffi::OsStrExt;
            let source: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let target: Vec<u16> = file.as_os_str().encode_wide().chain(Some(0)).collect();
            unsafe {
                windows::Win32::Storage::FileSystem::MoveFileExW(
                    windows::core::PCWSTR(source.as_ptr()),
                    windows::core::PCWSTR(target.as_ptr()),
                    windows::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING
                        | windows::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
                )
            }
            .map_err(|e| e.to_string())
        };
        #[cfg(not(windows))]
        let result = std::fs::rename(&temp, file).map_err(|e| e.to_string());
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result?;
        self.settings = settings;
        self.load_error = None;
        Ok(())
    }
    pub fn snapshot(&self) -> Value {
        json!({ "settings": self.settings, "web_url": WEB_URL, "runtime_root": self.runtime_root,
            "ready": crate::asr_runtime::is_ready(&self.runtime_root, &self.settings.asr_model), "setup": self.setup,
            "load_error": self.load_error })
    }
}
pub fn dispatch(app: &App, action: &str, payload: &Value) -> Option<Result<Value>> {
    Some(match action {
        "machine_settings" | "asr_setup_status" => app
            .0
            .lock()
            .map(|a| a.machine_settings.snapshot())
            .map_err(|e| e.to_string()),
        "settings_save" => (|| {
            let settings: Settings = serde_json::from_value(
                payload
                    .get("settings")
                    .cloned()
                    .unwrap_or_else(|| payload.clone()),
            )
            .map_err(|e| e.to_string())?;
            let mut a = app.0.lock().map_err(|e| e.to_string())?;
            if a.busy || a.auto_asr_running || a.machine_settings.setup.running {
                return Err("録音・認識・導入の終了後に設定を変更してください".into());
            }
            a.machine_settings.save(settings)?;
            Ok(a.machine_settings.snapshot())
        })(),
        "settings_devices" => (|| {
            use audio_mixer_backend::windows::devices::{DeviceDirection, list_devices};
            Ok(
                json!({ "capture": list_devices(DeviceDirection::Capture).map_err(|e| e.to_string())?, "render": list_devices(DeviceDirection::Render).map_err(|e| e.to_string())? }),
            )
        })(),
        "asr_setup" => crate::asr_runtime::start_setup(app),
        "open_settings_link" => (|| {
            let url = match payload["kind"].as_str().unwrap_or("web") {
                "web" => WEB_URL,
                "codex" => "https://developers.openai.com/codex/cli/",
                "python" => "https://www.python.org/downloads/",
                "whisper" => "https://github.com/SYSTRAN/faster-whisper",
                "api" => "https://developers.openai.com/api/docs/guides/speech-to-text",
                _ => return Err("登録されていないリンクです".into()),
            };
            open_link(url)?;
            Ok(Value::Null)
        })(),
        _ => return None,
    })
}
fn open_link(url: &str) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = std::ffi::OsStr::new(url)
            .encode_wide()
            .chain(Some(0))
            .collect();
        let result = unsafe {
            windows::Win32::UI::Shell::ShellExecuteW(
                None,
                windows::core::w!("open"),
                windows::core::PCWSTR(wide.as_ptr()),
                None,
                None,
                windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err("ブラウザーを開けませんでした".into());
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
    #[test]
    fn settings_are_pc_local_and_survive_reopen() {
        let root = std::env::temp_dir().join(crate::domain::id());
        let path = root.join("settings.json");
        let mut s = MachineSettings::load(path.clone(), root.join("asr"));
        assert!(s.settings.capture_endpoint.is_empty());
        let mut v = s.settings.clone();
        v.capture_endpoint = "microphone-1".into();
        s.save(v.clone()).unwrap();
        assert_eq!(
            MachineSettings::load(path.clone(), root.join("asr")).settings,
            v
        );
        v.asr_model = "../outside".into();
        assert!(s.save(v).is_err());
        assert_eq!(
            MachineSettings::load(path, root.join("asr"))
                .settings
                .capture_endpoint,
            "microphone-1"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_fields_get_defaults() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s.asr_model, "small");
        assert_eq!(s.asr_provider, "local");
    }
}
