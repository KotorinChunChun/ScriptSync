//! インストール済みアプリでも動く端末専用ASR環境と導入ワーカー。
use crate::{
    app::App,
    domain::Result,
    machine_settings::{Settings, SetupStatus},
};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
#[cfg(windows)]
#[path = "asr_authenticode.rs"]
mod authenticode;
const TRANSCRIBE: &str = include_str!("../../tools/asr/transcribe.py");
const PYTHON_URL: &str = "https://www.python.org/ftp/python/3.12.10/python-3.12.10-embed-amd64.zip";
// 2026-09-12 Python公式HTTPS配布から取得し、実行ファイルのPSF署名も検証した固定配布物。
const PYTHON_SHA256: &str = "4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3";

// CTranslate2はWindowsのverbatim接頭辞付きモデルパスを開けない。
fn runtime_path(root: &Path) -> Result<std::path::PathBuf> {
    let resolved=std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        let text=resolved.to_string_lossy();
        if let Some(unc)=text.strip_prefix(r"\\?\UNC\") { return Ok(format!(r"\\{unc}").into()); }
        if let Some(local)=text.strip_prefix(r"\\?\") { return Ok(local.into()); }
    }
    Ok(resolved)
}

pub fn is_ready(root: &Path, model: &str) -> bool {
    !root.as_os_str().is_empty()
        && root.join("python/python.exe").is_file()
        && root.join(format!("ready-{model}.json")).is_file()
}
pub fn command(root: &Path, settings: &Settings, input: &Path) -> Result<Command> {
    settings.validate()?;
    if !is_ready(root, &settings.asr_model) {
        return Err(
            "文字起こしの準備が必要です。設定画面の「文字起こし環境を導入」で準備してください"
                .into(),
        );
    }
    let resolved = runtime_path(root)?;
    let mut command = adapter_command(&resolved);
    command
        .arg("--audio")
        .arg(input)
        .arg("--model")
        .arg(&settings.asr_model)
        .arg("--provider")
        .arg("local");
    command.current_dir(root).env("PYTHONUTF8", "1");
    hide(&mut command);
    Ok(command)
}
/// MSIXのファイル仮想化で親が書いた.pyを子Pythonが開けない場合にも動く。
/// 同梱ソースを直接渡し、__file__は従来のモデル保存先を保持するためだけに使う。
fn adapter_command(root: &Path) -> Command {
    let mut command = Command::new(root.join("python/python.exe"));
    command
        .args(["-I", "-c"])
        .arg(format!(
            "import sys\nfrom pathlib import Path\n__file__ = sys.argv.pop(1)\nsys.path.insert(0, str(Path(__file__).parent / 'python' / 'Lib' / 'site-packages'))\n{TRANSCRIBE}"
        ))
        .arg(root.join("transcribe.py"))
        .current_dir(root)
        .env("PYTHONUTF8", "1");
    hide(&mut command);
    command
}
pub fn hide(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}
/// 認識中断・タイムアウト時にCLIやMCPサーバーを残さない。
pub struct ProcessTree {
    #[cfg(windows)]
    job: windows::Win32::Foundation::HANDLE,
}
impl ProcessTree {
    pub fn attach(child: &mut std::process::Child) -> Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows::Win32::{
                Foundation::{CloseHandle, HANDLE},
                System::JobObjects::*,
            };
            let result = (|| {
                let job = unsafe { CreateJobObjectW(None, None) }.map_err(|e| e.to_string())?;
                let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let configured = unsafe {
                    SetInformationJobObject(
                        job,
                        JobObjectExtendedLimitInformation,
                        &limits as *const _ as *const _,
                        std::mem::size_of_val(&limits) as u32,
                    )
                }
                .and_then(|_| unsafe {
                    AssignProcessToJobObject(job, HANDLE(child.as_raw_handle()))
                });
                if let Err(e) = configured {
                    let _ = unsafe { CloseHandle(job) };
                    return Err(e.to_string());
                }
                Ok(Self { job })
            })();
            if result.is_err() {
                let _ = child.kill();
                let _ = child.wait();
            }
            result
        }
        #[cfg(not(windows))]
        {
            let _ = child;
            Ok(Self {})
        }
    }
}
impl Drop for ProcessTree {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.job) };
        }
    }
}
fn status(app: &App, phase: &str, message: impl Into<String>) {
    if let Ok(mut a) = app.0.lock() {
        a.machine_settings.setup.phase = phase.into();
        a.machine_settings.setup.message = message.into();
    }
}
pub fn check_installation(app: &App) -> Result<Value> {
    let (root,model)={
        let mut a=app.0.lock().map_err(|e|e.to_string())?;
        if a.busy || a.auto_asr_running || a.machine_settings.setup.running { return Err("録音・認識・導入の終了後に確認してください".into()); }
        let root=a.machine_settings.runtime_root.clone();
        if root.as_os_str().is_empty() { return Err("文字起こしの保存先が初期化されていません".into()); }
        a.machine_settings.setup=SetupStatus { running:true,phase:"checking".into(),message:"ライブラリと選択モデルの読込を確認しています".into() };
        (root,a.machine_settings.settings.asr_model.clone())
    };
    let app=app.clone();
    std::thread::spawn(move || {
        let result=(|| {
            let root=runtime_path(&root)?;
            let mut command=Command::new(root.join("python/python.exe"));
            command.args(["-I","-c","import sys; from pathlib import Path; root=Path(sys.argv[1]); sys.path.insert(0,str(root/'python'/'Lib'/'site-packages')); import pykakasi; from faster_whisper import WhisperModel; WhisperModel(sys.argv[2],device='cpu',compute_type='int8',download_root=str(Path('C:/AI/Models/Speech/Whisper/ScriptSync')),local_files_only=True); print('ready')"]).arg(&root).arg(&model);
            run_logged(command,&root,"installation-check",Duration::from_secs(180))?;
            std::fs::write(root.join(format!("ready-{model}.json")),b"{\"verified\":true}").map_err(|e|e.to_string())
        })();
        if result.is_err() { let _=std::fs::remove_file(root.join(format!("ready-{model}.json"))); }
        if let Ok(mut a)=app.0.lock() {
            a.machine_settings.setup=SetupStatus { running:false,phase:if result.is_ok(){"ready"}else{"failed"}.into(),message:match result { Ok(())=>"Whisperのライブラリと選択モデルは利用できます".into(),Err(e)=>format!("Whisperの導入確認に失敗しました。設定画面から導入してください：{e}") } };
        }
    });
    Ok(json!({"started":true}))
}
fn verified_removal_root(root: &Path) -> Result<std::path::PathBuf> {
    if !root.is_absolute() || root.file_name().and_then(|s|s.to_str())!=Some("asr") || root.parent().and_then(|p|p.file_name()).and_then(|s|s.to_str())!=Some("jp.kotorinchunchun.scriptsync") {
        return Err("専用の文字起こし環境以外は削除できません".into());
    }
    let parent=std::fs::canonicalize(root.parent().ok_or("削除先が不正です")?).map_err(|e|e.to_string())?;
    let resolved=std::fs::canonicalize(root).map_err(|e|e.to_string())?;
    if resolved!=parent.join("asr") { return Err("文字起こし環境のリンク先は削除できません".into()); }
    for path in [root,root.parent().ok_or("削除先が不正です")?] {
        if is_link(&std::fs::symlink_metadata(path).map_err(|e|e.to_string())?) { return Err("文字起こし環境のリンク先は削除できません".into()); }
    }
    Ok(resolved)
}
fn is_link(metadata:&std::fs::Metadata)->bool {
    #[cfg(windows)]
    { use std::os::windows::fs::MetadataExt; if metadata.file_attributes()&0x400!=0 { return true; } }
    metadata.file_type().is_symlink()
}
// HFのsnapshotリンクは参照先を辿らず、リンク項目そのものだけを除去する。
fn remove_runtime_tree(path:&Path)->Result<()> {
    let metadata=std::fs::symlink_metadata(path).map_err(|e|e.to_string())?;
    if is_link(&metadata) {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            return if metadata.file_attributes()&0x10!=0 { std::fs::remove_dir(path) } else { std::fs::remove_file(path) }.map_err(|e|e.to_string());
        }
        #[cfg(not(windows))]
        return std::fs::remove_file(path).map_err(|e|e.to_string());
    }
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path).map_err(|e|e.to_string())? { remove_runtime_tree(&entry.map_err(|e|e.to_string())?.path())?; }
        std::fs::remove_dir(path).map_err(|e|e.to_string())
    } else { std::fs::remove_file(path).map_err(|e|e.to_string()) }
}
pub fn uninstall(app:&App)->Result<Value> {
    let mut a=app.0.lock().map_err(|e|e.to_string())?;
    if a.busy || a.auto_asr_running || a.machine_settings.setup.running { return Err("録音・認識・導入の終了後にアンインストールしてください".into()); }
    let root=&a.machine_settings.runtime_root;
    if root.exists() { let resolved=verified_removal_root(root)?; remove_runtime_tree(&resolved).map_err(|e|format!("アンインストールに失敗しました：{e}"))?; }
    a.machine_settings.setup=SetupStatus{running:false,phase:"removed".into(),message:"Whisperをアンインストールしました。再導入できます".into()};
    Ok(a.machine_settings.snapshot())
}
pub fn start_setup(app: &App) -> Result<Value> {
    let (root, settings) = {
        let mut a = app.0.lock().map_err(|e| e.to_string())?;
        if a.machine_settings.setup.running {
            return Ok(a.machine_settings.snapshot());
        }
        if a.busy || a.auto_asr_running {
            return Err("録音・文字起こしを終了してから導入してください".into());
        }
        if a.machine_settings.runtime_root.as_os_str().is_empty() {
            return Err("文字起こしの保存先が初期化されていません".into());
        }
        a.machine_settings.setup = SetupStatus {
            running: true,
            phase: "starting".into(),
            message: "文字起こし環境を準備しています".into(),
        };
        (
            a.machine_settings.runtime_root.clone(),
            a.machine_settings.settings.clone(),
        )
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let result = setup(&app, &root, &settings);
        if let Ok(mut a) = app.0.lock() {
            a.machine_settings.setup = match result {
                Ok(()) => SetupStatus {
                    running: false,
                    phase: "ready".into(),
                    message: "導入が完了しました。録音済みテイクの再認識を実行できます".into(),
                },
                Err(e) => SetupStatus {
                    running: false,
                    phase: "failed".into(),
                    message: format!("導入に失敗しました：{e}。設定画面から再試行できます"),
                },
            };
        }
    });
    Ok(json!({"started":true}))
}
fn setup(app: &App, root: &Path, settings: &Settings) -> Result<()> {
    settings.validate()?;
    if !cfg!(all(windows, target_arch = "x86_64")) {
        return Err("自動導入はWindows x64に対応しています".into());
    }
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let resolved = runtime_path(root)?;
    let root = resolved.as_path();
    let python = root.join("python/python.exe");
    if !python.is_file() {
        status(
            app,
            "download",
            "Python公式配布から実行環境を取得しています",
        );
        let package = root.join("python-3.12.10-embed-amd64.zip");
        download_python(&package, |bytes, total| {
            status(
                app,
                "download",
                format!(
                    "Pythonを取得中：{} / {} MiB",
                    bytes / 1_048_576,
                    total / 1_048_576
                ),
            )
        })?;
        status(
            app,
            "verify",
            "Python配布物のハッシュと発行元署名を検証しています",
        );
        if crate::storage::hash_file(&package)? != PYTHON_SHA256 {
            return Err("Python配布物のSHA-256が一致しません".into());
        }
        let stage = root.join(format!("python-preparing-{}", crate::domain::id()));
        extract_package(&package, &stage)?;
        verify_python(&stage.join("python.exe"))?;
        std::fs::write(
            stage.join("python312._pth"),
            "python312.zip\n.\nLib/site-packages\nimport site\n",
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(&stage, root.join("python")).map_err(|e| e.to_string())?;
    }
    verify_python(&python)?;
    // 途中失敗・旧導入物も再試行で検索パスを修復する。
    if root.join("python/python312.zip").is_file() {
        std::fs::write(root.join("python/python312._pth"),
            "python312.zip\n.\nLib/site-packages\nimport site\n").map_err(|e| e.to_string())?;
    }
    if !root
        .join("python/Lib/site-packages/pip/__main__.py")
        .is_file()
    {
        status(
            app,
            "pip",
            "ライブラリ導入ツールをPyPIから取得・検証しています",
        );
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;
        let metadata: Value = client
            .get("https://pypi.org/pypi/pip/25.1.1/json")
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| e.to_string())?
            .json()
            .map_err(|e| e.to_string())?;
        let wheel = metadata["urls"]
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .find(|r| r["filename"] == "pip-25.1.1-py3-none-any.whl")
            })
            .ok_or("pipの配布物がありません")?;
        let url = wheel["url"]
            .as_str()
            .filter(|s| s.starts_with("https://files.pythonhosted.org/"))
            .ok_or("pipの配布元が不正です")?;
        let file = root.join("pip-25.1.1.whl");
        download_file(url, &file, 8 * 1_048_576, |_, _| {})?;
        if Some(crate::storage::hash_file(&file)?.as_str()) != wheel["digests"]["sha256"].as_str() {
            return Err("pip配布物のSHA-256が一致しません".into());
        }
        extract_package(&file, &root.join("python/Lib/site-packages"))?;
    }
    status(
        app,
        "libraries",
        "文字起こしライブラリを取得しています（数分かかる場合があります）",
    );
    let mut pip = Command::new(&python);
    pip.args([
        "-I",
        "-m",
        "pip",
        "--isolated",
        "install",
        "--disable-pip-version-check",
        "--only-binary=:all:",
        "--index-url",
        "https://pypi.org/simple",
        "--target",
    ])
    .arg(root.join("python/Lib/site-packages"))
    .args(["faster-whisper==1.2.1", "pykakasi==2.3.0"]);
    run_logged(pip, root, "libraries", Duration::from_secs(1200))?;
    let mut verify = Command::new(&python);
    verify.args(["-I", "-c", "import sys; sys.path.insert(0, sys.argv[1]); import faster_whisper, pykakasi; print(faster_whisper.__file__)"])
        .arg(root.join("python/Lib/site-packages"));
    run_logged(verify, root, "library-check", Duration::from_secs(60))
        .map_err(|e| format!("ライブラリの読込確認に失敗しました。設定画面から導入を再試行してください：{e}"))?;
    status(
        app,
        "model",
        format!(
            "認識モデル {} を取得・検証しています（モデルにより数分以上かかります）",
            settings.asr_model
        ),
    );
    let mut prepare = adapter_command(root);
    prepare
        .arg("--prepare")
        .arg("--model")
        .arg(&settings.asr_model);
    run_logged(prepare, root, "model", Duration::from_secs(3600))?;
    std::fs::write(
        root.join(format!("ready-{}.json", settings.asr_model)),
        serde_json::to_vec(
            &json!({"python":"3.12.10","faster_whisper":"1.2.1","model":settings.asr_model}),
        )
        .unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn download_python(path: &Path, progress: impl Fn(u64, u64)) -> Result<()> {
    download_file(PYTHON_URL, path, 64 * 1_048_576, progress)
}
fn download_file(url: &str, path: &Path, maximum: u64, progress: impl Fn(u64, u64)) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .timeout(Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("取得：{e}"))?;
    let total = response.content_length().unwrap_or(0);
    if total > maximum {
        return Err("配布物のサイズが上限を超えています".into());
    }
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut bytes = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = response.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > maximum {
            return Err("取得サイズが上限を超えています".into());
        }
        file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        progress(bytes, total);
    }
    file.sync_all().map_err(|e| e.to_string())?;
    if bytes == 0 || (total != 0 && bytes != total) {
        return Err("Pythonの取得が完了していません".into());
    }
    Ok(())
}
fn extract_package(file: &Path, target: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(file).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if archive.len() > 5000 {
        return Err("配布物の項目が多すぎます".into());
    }
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let path = entry
            .enclosed_name()
            .ok_or("配布物に不正なパスがあります")?;
        if entry.is_symlink() {
            return Err("配布物のリンクは展開できません".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("配布物サイズが不正です")?;
        if total > 128 * 1_048_576 {
            return Err("配布物の展開サイズが上限を超えています".into());
        }
        let output = target.join(path);
        if entry.is_dir() {
            std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        } else {
            std::fs::create_dir_all(output.parent().ok_or("不正な展開先です")?)
                .map_err(|e| e.to_string())?;
            let mut out = std::fs::File::create(output).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn verify_python(path: &Path) -> Result<()> {
    if !path.is_file() {
        return Err("Python実行ファイルがありません".into());
    }
    #[cfg(windows)]
    {
        let signer = authenticode::get_signer_name(path).map_err(|e| e.to_string())?;
        if signer != "Python Software Foundation" {
            return Err(format!("Pythonの発行元が一致しません：{signer}"));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("署名検証はWindowsが必要です".into())
    }
}
fn run_logged(mut command: Command, root: &Path, name: &str, limit: Duration) -> Result<()> {
    let log = root.join(format!("setup-{name}.log"));
    let file = std::fs::File::create(&log).map_err(|e| e.to_string())?;
    command
        .current_dir(root)
        .stdout(Stdio::from(file.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(file));
    hide(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| format!("{name}を起動できません：{e}"))?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if status.success() || (name == "python-install" && status.code() == Some(3010)) {
                return Ok(());
            }
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            let tail = text
                .chars()
                .rev()
                .take(1600)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();
            return Err(format!("{name}が失敗しました（{}）：{tail}", log.display()));
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{name}が時間切れです（{}）", log.display()));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uninstall_removes_only_owned_runtime_and_rejects_busy() {
        let base=std::env::temp_dir().join(format!("scriptsync-remove-{}",crate::domain::id()));
        let root=base.join("jp.kotorinchunchun.scriptsync/asr");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("model.bin"),"fixture").unwrap();
        std::fs::write(base.join("recording.wav"),"preserve").unwrap();
        assert!(verified_removal_root(&base).is_err());
        assert!(verified_removal_root(Path::new("asr")).is_err());
        let app=App::default();
        {let mut a=app.0.lock().unwrap();a.machine_settings.runtime_root=root.clone();a.busy=true;}
        assert!(uninstall(&app).is_err());assert!(root.exists());
        app.0.lock().unwrap().busy=false;
        uninstall(&app).unwrap();assert!(!root.exists());assert!(base.join("recording.wav").exists());
        std::fs::remove_dir_all(base).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn uninstall_removes_junction_without_touching_external_files() {
        let base=std::env::temp_dir().join(format!("scriptsync-link-remove-{}",crate::domain::id()));
        let root=base.join("jp.kotorinchunchun.scriptsync/asr");
        let external=base.join("external");
        std::fs::create_dir_all(&root).unwrap();std::fs::create_dir_all(&external).unwrap();
        std::fs::write(external.join("sentinel.txt"),"保持").unwrap();
        let mut junction=Command::new("pwsh");
        junction.args(["-NoProfile","-NonInteractive","-Command","New-Item -ItemType Junction -Path $env:SCRIPTSYNC_TEST_LINK -Target $env:SCRIPTSYNC_TEST_TARGET -ErrorAction Stop | Out-Null"])
            .env("SCRIPTSYNC_TEST_LINK",root.join("snapshot")).env("SCRIPTSYNC_TEST_TARGET",&external);
        hide(&mut junction);assert!(junction.status().unwrap().success());
        let resolved=verified_removal_root(&root).unwrap();remove_runtime_tree(&resolved).unwrap();
        assert!(!root.exists());assert_eq!(std::fs::read_to_string(external.join("sentinel.txt")).unwrap(),"保持");
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn adapter_runs_embedded_source_without_a_script_file() {
        let root =
            std::env::temp_dir().join(format!("scriptsync-missing-script-{}", crate::domain::id()));
        assert!(!root.join("transcribe.py").exists());
        let command = adapter_command(&root);
        let args: Vec<_> = command
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert_eq!(&args[..2], ["-I", "-c"]);
        assert!(args[2].contains("sys.path.insert(0, str(Path(__file__).parent"));
        assert!(args[2].ends_with(TRANSCRIBE));
        assert_eq!(args[3], root.join("transcribe.py").to_string_lossy());
        // Windowsのコマンドライン上限に十分な余白を保つ。
        assert!(args.iter().map(|s| s.encode_utf16().count()).sum::<usize>() < 16_000);
    }
    #[test]
    fn runtime_path_is_absolute_and_not_verbatim() {
        let path=runtime_path(&std::env::temp_dir()).unwrap();
        assert!(path.is_absolute());
        assert!(!path.to_string_lossy().starts_with(r"\\?\"));
    }
    #[test]
    fn unavailable_runtime_never_uses_development_checkout() {
        assert!(!is_ready(Path::new(""), "small"));
        assert!(
            command(Path::new(""), &Settings::default(), Path::new("x.wav"))
                .unwrap_err()
                .contains("設定画面")
        );
    }
    #[test]
    fn verify_missing_download_is_clear() {
        assert!(verify_python(Path::new("absent-installer.exe"))
            .unwrap_err()
            .contains("ありません"));
    }
    #[test]
    #[ignore = "Python公式ZIPの取得とWindows署名検証。ネットワークが必要。実行はしない"]
    fn official_python_download_signature() {
        let root =
            std::env::temp_dir().join(format!("scriptsync-python-test-{}", crate::domain::id()));
        std::fs::create_dir_all(&root).unwrap();
        let package = root.join("python.zip");
        let result = (|| {
            download_python(&package, |_, _| {})?;
            assert_eq!(crate::storage::hash_file(&package)?, PYTHON_SHA256);
            extract_package(&package, &root.join("python"))?;
            verify_python(&root.join("python/python.exe"))
        })();
        let _ = std::fs::remove_dir_all(root);
        result.unwrap();
    }
    #[test]
    #[ignore = "ユーザー専用Pythonとモデルを実際に導入する。SCRIPTSYNC_ASR_SETUP_ROOTを明示し、導入物は保持する"]
    fn provision_runtime_and_recognize_silence() {
        let root = std::path::PathBuf::from(
            std::env::var("SCRIPTSYNC_ASR_SETUP_ROOT").expect("導入先を明示してください"),
        );
        assert!(root.is_absolute());
        let app = App::default();
        let settings = Settings::default();
        setup(&app, &root, &settings).unwrap();
        assert!(is_ready(&root, "small"));
        let audio = root.join("validation-silence.wav");
        let mut wav = hound::WavWriter::create(
            &audio,
            hound::WavSpec {
                channels: 1,
                sample_rate: 48000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for _ in 0..48000 {
            wav.write_sample(0i16).unwrap();
        }
        wav.finalize().unwrap();
        let output = command(&root, &settings, &audio).unwrap().output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["engine"], "faster-whisper");
        assert_eq!(result["duration"], 1.0);
        println!("導入・認識成功：{}", root.display());
    }
}
