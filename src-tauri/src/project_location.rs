use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct ProjectLocation {
    pub settings_file: Option<PathBuf>,
    pub directory: Option<PathBuf>,
}
impl ProjectLocation {
    pub fn load(settings_file: PathBuf) -> Self {
        let directory = std::fs::read(&settings_file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<PathBuf>(&bytes).ok())
            .filter(|path| path.is_dir());
        Self {
            settings_file: Some(settings_file),
            directory,
        }
    }
    pub fn remember(&mut self, project_file: &Path) -> Result<(), String> {
        let directory = project_file
            .parent()
            .filter(|p| p.is_dir())
            .ok_or("保存フォルダーがありません")?
            .to_path_buf();
        self.directory = Some(directory.clone());
        if let Some(file) = &self.settings_file {
            std::fs::create_dir_all(file.parent().ok_or("設定フォルダーがありません")?)
                .map_err(|e| e.to_string())?;
            std::fs::write(
                file,
                serde_json::to_vec(&directory).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn created_project_directory_is_reused_after_restart() {
        let root = std::env::temp_dir().join(crate::domain::id());
        std::fs::create_dir_all(root.join("原稿")).unwrap();
        let config = root.join("settings/location.json");
        let mut location = ProjectLocation::load(config.clone());
        assert!(location.directory.is_none());
        location
            .remember(&root.join("原稿/収録.scriptsync"))
            .unwrap();
        assert_eq!(
            ProjectLocation::load(config).directory,
            Some(root.join("原稿"))
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
