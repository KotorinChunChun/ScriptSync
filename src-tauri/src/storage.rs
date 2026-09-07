use crate::domain::*;
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn hash_file(path: &Path) -> Result<String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = f.read(&mut b).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?
    }
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())
}
pub fn json_new<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    write_new(
        path,
        &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
}
pub fn safe(root: &Path, relative: &str) -> Result<PathBuf> {
    let p = Path::new(relative);
    if p.as_os_str().is_empty()
        || p.components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("作業フォルダ外のパスは使用できません".into());
    }
    let canonical = root.canonicalize().map_err(|e| e.to_string())?;
    let result = root.join(p);
    let mut ancestor = result.as_path();
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or("保存先が不正です")?;
    }
    if !ancestor
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&canonical)
    {
        return Err("リンクによるフォルダ外参照です".into());
    }
    Ok(result)
}
pub struct Store {
    pub root: PathBuf,
    pub project: Project,
    _lock: File,
}
impl Store {
    pub fn open(root: PathBuf, create: bool) -> Result<Self> {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(".scriptsync.lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock_exclusive()
            .map_err(|_| "別のウィンドウがこのプロジェクトを開いています")?;
        let revisions = root.join("revisions");
        fs::create_dir_all(&revisions).map_err(|e| e.to_string())?;
        let mut files: Vec<_> = fs::read_dir(&revisions)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        let project = if let Some(last) = files.last() {
            serde_json::from_slice::<Project>(&fs::read(last).map_err(|e| e.to_string())?).map_err(
                |e| format!("最新保存世代を読めません。revisionsを保全してください: {e}"),
            )?
        } else if create {
            Project::new(
                root.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
            )
        } else {
            return Err("ScriptSyncプロジェクトではありません".into());
        };
        if project.version != 1 {
            return Err("未対応のプロジェクト版です".into());
        }
        let mut s = Self {
            root,
            project,
            _lock: lock,
        };
        for seg in &mut s.project.segments {
            for take in &mut seg.takes {
                if take.status == "recording" {
                    take.status = "interrupted".into();
                    take.warning =
                        "前回の録音が中断しました。音声取込で回収可能なWAVを検査できます".into();
                }
            }
        }
        s.save()?;
        Ok(s)
    }
    pub fn save(&mut self) -> Result<()> {
        let generation = self.project.generation + 1;
        let mut snapshot = self.project.clone();
        snapshot.generation = generation;
        let rev = safe(&self.root, &format!("revisions/{generation:020}.json"))?;
        let temp = rev.with_extension(format!("{}.tmp", id()));
        json_new(&temp, &snapshot)?;
        fs::rename(&temp, &rev).map_err(|e| e.to_string())?;
        self.project.generation = generation;
        let index = self.root.join(format!("project-{}.tmp", id()));
        json_new(&index, &snapshot)?;
        fs::rename(index, self.root.join("project.json")).map_err(|e| e.to_string())?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revisions_lock_and_recovery() {
        let root = std::env::temp_dir().join(id());
        let mut s = Store::open(root.clone(), true).unwrap();
        s.project.segments.push(Segment::new("テスト".into()));
        s.save().unwrap();
        assert!(Store::open(root.clone(), false).is_err());
        drop(s);
        let s = Store::open(root.clone(), false).unwrap();
        assert_eq!(s.project.segments.len(), 1);
        assert!(safe(&root, "../secret").is_err());
        drop(s);
        fs::remove_dir_all(root).unwrap();
    }
}
