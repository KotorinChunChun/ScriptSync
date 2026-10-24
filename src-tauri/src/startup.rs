//! 通常表示・管理起動と、検証用の明示的な設定分離。
use std::path::PathBuf;
use tauri::Manager;

#[derive(Default)]
pub struct Options { pub hidden: bool, pub settings_dir: Option<PathBuf> }
impl Options {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let settings_dir = if let Some(index) = args.iter().position(|arg| arg == "--settings-dir") {
            let path = PathBuf::from(args.get(index + 1).ok_or("設定ディレクトリがありません")?);
            if !path.is_absolute() { return Err("設定ディレクトリには絶対パスが必要です".into()); }
            Some(path)
        } else { None };
        Ok(Self { hidden: args.iter().any(|arg| arg == "--startup" || arg == "--hidden"), settings_dir })
    }
}
pub fn config_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Some(path) = &app.state::<Options>().settings_dir { return Ok(path.join("config")); }
    app.path().app_config_dir().map_err(|error| error.to_string())
}
pub fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Some(path) = &app.state::<Options>().settings_dir { return Ok(path.join("data")); }
    app.path().app_local_data_dir().map_err(|error| error.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_start_is_visible_and_managed_start_is_hidden() {
        assert!(!Options::parse(&["app".into()]).unwrap().hidden);
        assert!(Options::parse(&["app".into(), "--startup".into()]).unwrap().hidden);
        assert!(Options::parse(&["app".into(), "--settings-dir".into()]).is_err());
        assert!(Options::parse(&["app".into(), "--settings-dir".into(), "relative".into()]).is_err());
    }
}
