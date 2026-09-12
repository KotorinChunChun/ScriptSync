use crate::domain::*;
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const PROJECT_FILE_EXTENSION: &str = "scriptsync";
const PROJECT_FILE_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
struct ProjectFile {
    version: u32,
    data_directory: String,
}
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
    if relative.contains(':') {
        return Err("代替データストリームは使用できません".into());
    }
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
    pub project_file: Option<PathBuf>,
    pub project: Project,
    _lock: File,
}
fn validate_project(root: &Path, p: &Project) -> Result<()> {
    let mut ids = std::collections::HashSet::new();
    let mut check_id = |value: &str| -> Result<()> {
        uuid::Uuid::parse_str(value).map_err(|_| "IDの形式が不正です")?;
        if !ids.insert(value.to_owned()) {
            return Err("IDが重複しています".into());
        }
        Ok(())
    };
    let hash_ok = |h: &str| h.len() == 64 && h.bytes().all(|c| c.is_ascii_hexdigit());
    check_id(&p.id)?;
    let mut source_names = std::collections::HashSet::new();
    for source in &p.script_sources {
        check_id(&source.id)?;
        if source.name.trim().is_empty() || !source_names.insert(source.name.clone()) {
            return Err("台本ファイル名が空か重複しています".into());
        }
        if source.confirmed_text.len() > 2_000_000 || source.draft_text.len() > 2_000_000 {
            return Err("台本ファイルは2MB以内にしてください".into());
        }
    }
    for source in &p.archived_sources {
        check_id(&source.id)?;
        if source.name.trim().is_empty() { return Err("原稿名が空です".into()); }
    }
    let mut pending = std::collections::HashSet::new();
    for source_id in &p.pending_source_deletions {
        if !pending.insert(source_id) || !p.script_sources.iter().any(|s| &s.id == source_id) {
            return Err("削除予定の原稿が不正です".into());
        }
    }
    if p.segments.len() > 1000 {
        return Err("発話数の上限を超えています".into());
    }
    for s in p.segments.iter().chain(&p.archived_segments) {
        if !s.gain_db.is_finite() || !(-36.0..=24.0).contains(&s.gain_db) {
            return Err("音量調整の範囲が不正です".into());
        }
        check_id(&s.id)?;
        for t in &s.takes {
            check_id(&t.id)?;
            safe(root, &t.file)?;
            if (!t.hash.is_empty() && !hash_ok(&t.hash))
                || !t.duration.is_finite()
                || t.duration < 0.0
                || !t.peak.is_finite()
            {
                return Err("音声メタデータが不正です".into());
            }
            if t.status == "ready" {
                t.trimmed_range()?;
            }
        }
        if s.selected
            .as_ref()
            .is_some_and(|id| !s.takes.iter().any(|t| &t.id == id))
        {
            return Err("採用テイクが存在しません".into());
        }
        for c in &s.corrections {
            check_id(&c.id)?;
            slice(&s.text, c.start, c.end)?;
        }
        corrected(s)?;
        for r in &s.comparisons {
            check_id(&r.id)?;
            for v in &r.variants {
                check_id(&v.id)?;
                if !v.file.is_empty() {
                    safe(root, &v.file)?;
                }
                if !v.hash.is_empty() && !hash_ok(&v.hash) {
                    return Err("比較音声ハッシュが不正です".into());
                }
            }
            if r.order.len() != r.variants.len()
                || r.order
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    != r.order.len()
                || r.order
                    .iter()
                    .any(|id| !r.variants.iter().any(|v| &v.id == id))
            {
                return Err("比較の提示順が不正です".into());
            }
        }
    }
    Ok(())
}
impl Store {
    pub fn open(root: PathBuf, create: bool) -> Result<Self> {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(safe(&root, ".scriptsync.lock")?)
            .map_err(|e| e.to_string())?;
        lock.try_lock_exclusive()
            .map_err(|_| "別のウィンドウがこのプロジェクトを開いています")?;
        let revisions = safe(&root, "revisions")?;
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
        if project.version != 1 && project.version != 2 && project.version != 3 && project.version != 4 && project.version != 5 {
            return Err("未対応のプロジェクト版です".into());
        }
        validate_project(&root, &project)?;
        let mut s = Self {
            root,
            project_file: None,
            project,
            _lock: lock,
        };
        for batch in &mut s.project.batches {
            for item in &mut batch.items {
                if item.status == "running" || item.status == "pending" {
                    item.status = "interrupted".into();
                    item.error = "前回の一括処理が中断しました".into();
                }
            }
        }
        for seg in s
            .project
            .segments
            .iter_mut()
            .chain(s.project.archived_segments.iter_mut())
        {
            if seg.source_text.is_empty() {
                seg.source_text = seg.text.clone();
            }
            if seg.line_id.is_empty() {
                // 既存segment_idを行IDとして継承するため、テイクや出力設定の参照を壊さない。
                seg.line_id = seg.id.clone();
            }
            if seg.line_revisions.is_empty() {
                seg.line_revisions.push(LineRevision {
                    id: id(),
                    source_text: seg.source_text.clone(),
                    source_markdown: String::new(),
                    created: now(),
                });
            }
            if seg.hierarchy.is_empty() {
                seg.hierarchy = vec![0; 4];
            }
            for record in &mut seg.transcriptions {
                if record["status"] == "running" {
                    record["status"] = "interrupted".into();
                    record["error"] =
                        "前回の認識が中断しました。同じ音声で再実行してください".into();
                }
            }
            for run in &mut seg.comparisons {
                for variant in &mut run.variants {
                    if variant.status == "running" || variant.status == "pending" {
                        variant.status = "failed".into();
                        variant.error =
                            "前回の処理が中断しました。保存済み条件で再試行できます".into();
                    }
                }
            }
            for take in &mut seg.takes {
                if take.status == "recording" {
                    take.status = "interrupted".into();
                    take.warning =
                        "前回の録音が中断しました。音声取込で回収可能なWAVを検査できます".into();
                }
            }
        }
        if s.project.version < 5 && s.project.script_sources.is_empty() && (s.project.version < 4 || !s.project.source_markdown.is_empty() || !s.project.script_lines.is_empty() || !s.project.segments.is_empty() || !s.project.archived_segments.is_empty()) {
            // 旧形式の一つの原稿を一つのファイル台本へ包む。既存の発話・テイクIDは変更しない。
            let text = if s.project.source_markdown.is_empty() && s.project.script_lines.is_empty() {
                s.project.segments.iter().map(|segment| if segment.source_text.is_empty() { segment.text.as_str() } else { segment.source_text.as_str() }).collect::<Vec<_>>().join("\n")
            } else { s.project.source_markdown.clone() };
            let lines = if s.project.script_lines.is_empty() {
                crate::markdown::parse(&text)?
            } else {
                s.project.script_lines.clone()
            };
            let source_id = id();
            s.project.script_sources.push(ScriptSource {
                id: source_id.clone(), name: "既存台本".into(), kind: "markdown".into(),
                confirmed_text: text.clone(), draft_text: text, lines,
                draft_lines: vec![], blank_rows: vec![], approved_text: None,
            });
            for seg in s.project.segments.iter_mut().chain(s.project.archived_segments.iter_mut()) {
                seg.source_id = source_id.clone();
            }
            s.project.script_revision = s.project.script_revision.max(1);
        } else if !s.project.script_sources.is_empty() {
            let fallback = s.project.script_sources[0].id.clone();
            for seg in s.project.segments.iter_mut().chain(s.project.archived_segments.iter_mut()) {
                if seg.source_id.is_empty() { seg.source_id = fallback.clone(); }
            }
        }
        s.project.version = 5;
        s.save()?;
        Ok(s)
    }

    /// 新しい形式では、選択する `.scriptsync` ファイルと実データのフォルダを分ける。
    /// 音声などをプロジェクトファイルへ詰め込まず、従来と同じ非破壊の世代保存を続ける。
    pub fn create_project_file(path: PathBuf) -> Result<Self> {
        if !path.extension().and_then(|value| value.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case(PROJECT_FILE_EXTENSION)) {
            return Err("プロジェクトファイルの拡張子は .scriptsync にしてください".into());
        }
        let parent = path
            .parent()
            .ok_or("プロジェクトファイルの保存先が不正です")?;
        let name = path.file_name().ok_or("プロジェクトファイル名が不正です")?;
        let data_name = format!("{}.data", name.to_string_lossy());
        let data_root = parent.join(&data_name);
        if data_root.exists() {
            return Err("同名のプロジェクトデータフォルダが既にあります".into());
        }
        let project_file = ProjectFile {
            version: PROJECT_FILE_VERSION,
            data_directory: data_name,
        };
        // create_new により、既存のプロジェクトファイルは一切置き換えない。
        json_new(&path, &project_file)?;
        let mut store = match Self::open(data_root, true) {
            Ok(store) => store,
            Err(error) => {
                // この呼出しが create_new した入口だけを消す。作成済みデータは保全する。
                let _ = fs::remove_file(&path);
                return Err(error);
            }
        };
        store.project.title = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        store.save()?;
        store.project_file = Some(path);
        Ok(store)
    }

    pub fn open_project_file(path: PathBuf) -> Result<Self> {
        if path.extension().and_then(|value| value.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("json")) {
            return Self::migrate_legacy_project(path);
        }
        if !path.extension().and_then(|value| value.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case(PROJECT_FILE_EXTENSION)) {
            return Err(".scriptsync プロジェクトファイルを選択してください".into());
        }
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        let project_file: ProjectFile = serde_json::from_slice(&bytes)
            .map_err(|e| format!("プロジェクトファイルを読めません: {e}"))?;
        if project_file.version != PROJECT_FILE_VERSION {
            return Err("未対応のプロジェクトファイル版です".into());
        }
        let parent = path
            .parent()
            .ok_or("プロジェクトファイルの保存先が不正です")?;
        let data_root = if project_file.data_directory == "." {
            parent.canonicalize().map_err(|e| e.to_string())?
        } else {
            safe(parent, &project_file.data_directory)?
        };
        let mut store = Self::open(data_root, false)?;
        store.project_file = Some(path);
        Ok(store)
    }

    fn migrate_legacy_project(project_json: PathBuf) -> Result<Self> {
        if project_json.file_name().and_then(|value| value.to_str()) != Some("project.json") {
            return Err("従来形式では project.json を選択してください".into());
        }
        let root = project_json
            .parent()
            .ok_or("従来プロジェクトの保存先が不正です")?
            .to_path_buf();
        let manifest = root.join("project.scriptsync");
        // 先に正本のrevisionsを検証する。不正なフォルダに入口ファイルを残さない。
        let mut store = Self::open(root, false)?;
        if !manifest.exists() {
            let project_file = ProjectFile {
                version: PROJECT_FILE_VERSION,
                data_directory: ".".into(),
            };
            // 既存データを移動・書換えせず、追加の入口ファイルだけを作る。
            json_new(&manifest, &project_file)?;
        }
        store.project_file = Some(manifest);
        Ok(store)
    }
    pub fn save(&mut self) -> Result<()> {
        let generation = self.project.generation + 1;
        let mut snapshot = self.project.clone();
        snapshot.version = 5;
        snapshot.generation = generation;
        let rev = safe(&self.root, &format!("revisions/{generation:020}.json"))?;
        let temp = rev.with_extension(format!("{}.tmp", id()));
        json_new(&temp, &snapshot)?;
        // renameはWindowsでも既存世代を置き換え得る。新規リンクの公開なら
        // 既存名との競合時に失敗し、不変世代を上書きしない。
        if let Err(error) = fs::hard_link(&temp, &rev) {
            let _ = fs::remove_file(&temp);
            return Err(format!(
                "保存世代を確定できません。再読込して再確認してください: {error}"
            ));
        }
        let _ = fs::remove_file(&temp);
        self.project.generation = generation;
        self.project.version = 5;
        // 世代は既に確定済み。閲覧用索引の失敗でメモリを旧世代へ戻さない。
        if let Ok(index) = safe(&self.root, &format!("project-{}.tmp", id())) {
            if json_new(&index, &snapshot).is_ok() {
                if let Ok(destination) = safe(&self.root, "project.json") {
                    let _ = fs::rename(index, destination);
                }
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_save_never_overwrites_an_existing_revision() {
        let root = std::env::temp_dir().join(id());
        let mut store = Store::open(root.clone(), true).unwrap();
        let before = store.project.clone();
        store.project.title = "確定済みの内容".into();
        store.save().unwrap();
        let revision = root.join(format!("revisions/{:020}.json", store.project.generation));
        let bytes = fs::read(&revision).unwrap();
        store.project = before;
        store.project.title = "古い下書き".into();
        assert!(store.save().is_err());
        assert_eq!(fs::read(&revision).unwrap(), bytes);
        drop(store);
        let reopened = Store::open(root.clone(), false).unwrap();
        assert_eq!(reopened.project.title, "確定済みの内容");
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interrupted_zero_duration_take_can_reopen() {
        let root = std::env::temp_dir().join(id());
        let mut store = Store::open(root.clone(), true).unwrap();
        let mut segment = Segment::new("中断した録音".into());
        segment.takes.push(
            serde_json::from_value(serde_json::json!({
                "id": id(), "file": "takes/interrupted.wav", "hash": "", "status": "recording",
                "duration": 0.0, "peak": 0.0, "warning": "", "created": 0, "endpoint": ""
            }))
            .unwrap(),
        );
        store.project.segments.push(segment);
        store.save().unwrap();
        drop(store);
        let reopened = Store::open(root.clone(), false).unwrap();
        assert_eq!(reopened.project.segments[0].takes.len(), 1);
        assert_ne!(reopened.project.segments[0].takes[0].status, "recording");
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }
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

    #[test]
    fn v2_project_keeps_segment_ids_when_line_metadata_is_added() {
        let root = std::env::temp_dir().join(id());
        let mut store = Store::open(root.clone(), true).unwrap();
        let segment = Segment::new("旧版の発話".into());
        let segment_id = segment.id.clone();
        store.project.segments.push(segment);
        store.project.version = 2;
        let mut legacy = serde_json::to_value(&store.project).unwrap();
        legacy["generation"] = serde_json::json!(store.project.generation + 1);
        let row = legacy["segments"][0].as_object_mut().unwrap();
        row.remove("line_id");
        row.remove("line_revisions");
        fs::write(
            root.join(format!(
                "revisions/{:020}.json",
                store.project.generation + 1
            )),
            serde_json::to_vec(&legacy).unwrap(),
        )
        .unwrap();
        drop(store);

        let reopened = Store::open(root.clone(), false).unwrap();
        let migrated = &reopened.project.segments[0];
        assert_eq!(migrated.id, segment_id);
        assert_eq!(migrated.line_id, segment_id);
        assert_eq!(migrated.line_revisions.len(), 1);
        assert_eq!(migrated.line_revisions[0].source_text, "旧版の発話");
        assert_eq!(reopened.project.version, 5);
        assert_eq!(reopened.project.script_sources.len(), 1);
        assert_eq!(reopened.project.script_sources[0].name, "既存台本");
        assert_eq!(migrated.source_id, reopened.project.script_sources[0].id);
        let line_revision_id = migrated.line_revisions[0].id.clone();
        drop(reopened);
        let again = Store::open(root.clone(), false).unwrap();
        assert_eq!(
            again.project.segments[0].line_revisions[0].id,
            line_revision_id
        );
        drop(again);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_file_uses_a_sidecar_data_directory_and_reopens() {
        let parent = std::env::temp_dir().join(id());
        fs::create_dir_all(&parent).unwrap();
        let project_file = parent.join("収録.scriptsync");
        let mut store = Store::create_project_file(project_file.clone()).unwrap();
        assert_eq!(store.root, parent.join("収録.scriptsync.data"));
        assert!(project_file.exists());
        assert_eq!(store.project.title, "収録");
        store.project.title = "収録テスト".into();
        store.save().unwrap();
        drop(store);

        let reopened = Store::open_project_file(project_file).unwrap();
        assert_eq!(reopened.project.title, "収録テスト");
        assert!(reopened.root.join("revisions").exists());
        drop(reopened);
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn selecting_legacy_project_json_adds_a_manifest_without_moving_data() {
        let root = std::env::temp_dir().join(id());
        let mut original = Store::open(root.clone(), true).unwrap();
        original.project.title = "従来形式".into();
        original.save().unwrap();
        drop(original);

        let migrated = Store::open_project_file(root.join("project.json")).unwrap();
        assert_eq!(migrated.root, root);
        assert_eq!(migrated.project.title, "従来形式");
        assert!(root.join("project.scriptsync").exists());
        drop(migrated);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_legacy_project_does_not_gain_a_migration_manifest() {
        let root = std::env::temp_dir().join(id());
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("project.json"), b"{}").unwrap();
        assert!(Store::open_project_file(root.join("project.json")).is_err());
        assert!(!root.join("project.scriptsync").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
