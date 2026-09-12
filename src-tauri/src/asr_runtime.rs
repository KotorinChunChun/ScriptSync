//! インストール済みアプリでも動く端末専用ASR環境と導入ワーカー。
use crate::{
    app::App,
    domain::Result,
    machine_settings::{Settings, SetupStatus},
};
use serde_json::{Value, json};
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
    let script = root.join("transcribe.py");
    // アプリの更新で認識アダプターだけを更新。導入物やモデルは変更しない。
    if std::fs::read(&script).ok().as_deref() != Some(TRANSCRIBE.as_bytes()) {
        std::fs::write(&script, TRANSCRIBE).map_err(|e| e.to_string())?;
    }
    let mut command = Command::new(root.join("python/python.exe"));
    command
        .arg("-I")
        .arg(&script)
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
    std::fs::write(root.join("transcribe.py"), TRANSCRIBE).map_err(|e| e.to_string())?;
    status(
        app,
        "model",
        format!(
            "認識モデル {} を取得・検証しています（モデルにより数分以上かかります）",
            settings.asr_model
        ),
    );
    let mut prepare = Command::new(&python);
    prepare
        .arg("-I")
        .arg(root.join("transcribe.py"))
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
        assert!(
            verify_python(Path::new("absent-installer.exe"))
                .unwrap_err()
                .contains("ありません")
        );
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
