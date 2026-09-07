use audio_mixer_backend::recording::sample_session::{
    derive_shortened, quality, read_wav, Derived, Session, Take,
};
#[path = "microphone_recorder_ui/mod.rs"]
mod recorder_ui;
use std::{
    io::{self, BufRead, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
use std::{path::PathBuf, process::ExitCode};

use audio_mixer_backend::windows::{
    devices::{list_devices, AudioDeviceInfo, DeviceDirection},
    microphone_recording::{
        record_microphone_to_wav, record_microphone_to_wav_controlled, MicrophoneRecordingRequest,
        DEFAULT_MICROPHONE_RECORDING_DURATION_MS, DEFAULT_MICROPHONE_STARTUP_TIMEOUT_MS,
    },
};

const USAGE: &str = "Usage:
  microphone-recorder --list
  microphone-recorder --interactive [--directory DIR] [--device NAME | --endpoint-id ID]
  microphone-recorder --interactive --console  (番号入力で操作)
  microphone-recorder (--device NAME | --endpoint-id ID) --output FILE.wav [options]

Options:
  --duration-ms N         録音時間（既定: 10000）
  --startup-timeout-ms N  WASAPI起動待ち時間（既定: 3000）
  --help                  このヘルプを表示";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Help,
    List,
    Interactive {
        console: bool,
        directory: PathBuf,
        device_name: Option<String>,
        endpoint_id: Option<String>,
        duration_ms: u64,
    },
    Record(RecordOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordOptions {
    device_name: Option<String>,
    endpoint_id: Option<String>,
    output_path: PathBuf,
    duration_ms: u64,
    startup_timeout_ms: u64,
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("エラー: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), String> {
    match parse_args(arguments)? {
        Action::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Action::List => {
            let devices =
                list_devices(DeviceDirection::Capture).map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&devices).map_err(|error| error.to_string())?
            );
            Ok(())
        }
        Action::Interactive {
            console,
            directory,
            device_name,
            endpoint_id,
            duration_ms,
        } => {
            if console {
                interactive(directory, device_name, endpoint_id, duration_ms)
            } else {
                recorder_ui::run(directory, device_name, endpoint_id, duration_ms)
            }
        }
        Action::Record(options) => {
            let devices =
                list_devices(DeviceDirection::Capture).map_err(|error| error.to_string())?;
            let device = select_device(
                &devices,
                options.endpoint_id.as_deref(),
                options.device_name.as_deref(),
            )?;
            eprintln!(
                "録音開始: {} / {} ms / {}",
                device.friendly_name,
                options.duration_ms,
                options.output_path.display()
            );
            let mut request = MicrophoneRecordingRequest::new(
                device.endpoint_id.clone(),
                options.output_path,
                options.duration_ms,
            );
            request.startup_timeout_ms = options.startup_timeout_ms;
            let report = record_microphone_to_wav(&request).map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
            );
            Ok(())
        }
    }
}

fn parse_args(arguments: impl IntoIterator<Item = String>) -> Result<Action, String> {
    let mut arguments = arguments.into_iter();
    let mut list = false;
    let mut interactive = false;
    let mut console = false;
    let mut directory = None;
    let mut device_name = None;
    let mut endpoint_id = None;
    let mut output_path = None;
    let mut duration_ms = DEFAULT_MICROPHONE_RECORDING_DURATION_MS;
    let mut startup_timeout_ms = DEFAULT_MICROPHONE_STARTUP_TIMEOUT_MS;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(Action::Help),
            "--list" => list = true,
            "--interactive" => interactive = true,
            "--console" => console = true,
            "--directory" => {
                directory = Some(PathBuf::from(next_value(&mut arguments, "--directory")?))
            }
            "--device" => device_name = Some(next_value(&mut arguments, "--device")?),
            "--endpoint-id" => endpoint_id = Some(next_value(&mut arguments, "--endpoint-id")?),
            "--output" => {
                output_path = Some(PathBuf::from(next_value(&mut arguments, "--output")?))
            }
            "--duration-ms" => {
                duration_ms = parse_u64(
                    next_value(&mut arguments, "--duration-ms")?,
                    "--duration-ms",
                )?
            }
            "--startup-timeout-ms" => {
                startup_timeout_ms = parse_u64(
                    next_value(&mut arguments, "--startup-timeout-ms")?,
                    "--startup-timeout-ms",
                )?
            }
            other => return Err(format!("不明な引数です: {other}\n\n{USAGE}")),
        }
    }

    if interactive {
        if list || output_path.is_some() || (device_name.is_some() && endpoint_id.is_some()) {
            return Err(
                "対話モードは--list/--outputと併用できず、マイク指定はどちらか一方です".into(),
            );
        }
        if duration_ms == 0 || duration_ms > 86_400_000 {
            return Err("録音時間は1〜86400000 msです".into());
        }
        let directory = match directory {
            Some(path) => path,
            None => default_session_directory()?,
        };
        return Ok(Action::Interactive {
            console,
            directory,
            device_name,
            endpoint_id,
            duration_ms,
        });
    }
    if directory.is_some() || console {
        return Err("--directoryには--interactiveが必要です".into());
    }

    if list {
        if device_name.is_some() || endpoint_id.is_some() || output_path.is_some() {
            return Err("--listは録音用引数と同時に指定できません".into());
        }
        return Ok(Action::List);
    }
    if device_name.is_some() == endpoint_id.is_some() {
        return Err("--deviceまたは--endpoint-idのどちらか一方を指定してください".into());
    }
    let output_path = output_path.ok_or_else(|| "--outputを指定してください".to_string())?;
    Ok(Action::Record(RecordOptions {
        device_name,
        endpoint_id,
        output_path,
        duration_ms,
        startup_timeout_ms,
    }))
}

fn default_session_directory() -> Result<PathBuf, String> {
    if let Ok(current) = std::env::current_dir() {
        for root in current.ancestors() {
            if root.join("src/src-tauri/Cargo.toml").is_file() && root.join("AGENTS.md").is_file() {
                return Ok(root.join("dev/private/audio-samples"));
            }
        }
    }
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("VoiceMixer/audio-samples"))
        .ok_or_else(|| "--directoryで固定保存先を指定してください".into())
}

fn input_channel() -> mpsc::Receiver<String> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) => {
                    if send.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    receive
}

fn sample_path(directory: &std::path::Path, file: &str) -> Result<PathBuf, String> {
    if std::path::Path::new(file).components().count() != 1
        || !matches!(
            std::path::Path::new(file).components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        return Err("台帳の音声パスが不正です".into());
    }
    Ok(directory.join(file))
}

fn interactive(
    directory: PathBuf,
    device_name: Option<String>,
    endpoint_id: Option<String>,
    duration_ms: u64,
) -> Result<(), String> {
    let lines = input_channel();
    let devices = list_devices(DeviceDirection::Capture).map_err(|e| e.to_string())?;
    let mut session = if directory.join("session.json").exists() {
        Session::load(&directory)?
    } else {
        Session::new(String::new())
    };
    if device_name.is_some() || endpoint_id.is_some() {
        session.endpoint_id =
            select_device(&devices, endpoint_id.as_deref(), device_name.as_deref())?
                .endpoint_id
                .clone();
    } else if session.endpoint_id.is_empty() {
        for (index, device) in devices.iter().enumerate() {
            println!("{}: {}", index + 1, device.friendly_name);
        }
        print!("マイク番号 > ");
        io::stdout().flush().map_err(|e| e.to_string())?;
        let index = lines
            .recv()
            .map_err(|e| e.to_string())?
            .trim()
            .parse::<usize>()
            .map_err(|_| "番号を指定してください")?;
        session.endpoint_id = devices
            .get(index.wrapping_sub(1))
            .ok_or("マイク番号が範囲外です")?
            .endpoint_id
            .clone();
    }
    select_device(&devices, Some(&session.endpoint_id), None)?;
    session.save(&directory)?;
    println!("保存先: {}\n番号: 録音／再録音、s または Enter: 録音停止、p 番号: 原音再生、d 番号: 短縮版再生、t 番号: 無音を0.1秒に短縮、a 番号: 最新テイクを採用、q: 終了", directory.display());
    println!("設定: settings 無音閾値dB 最短無音ms 発声余白ms 小音量閾値dB（例 settings -45 101 10 -40）");
    loop {
        for (index, prompt) in session.prompts.iter().enumerate() {
            println!(
                "{:2}. [{}] {}",
                index + 1,
                if prompt.accepted_take.is_some() {
                    "済"
                } else {
                    "未"
                },
                prompt.text
            );
        }
        print!("> ");
        io::stdout().flush().map_err(|e| e.to_string())?;
        let Ok(line) = lines.recv() else {
            break;
        };
        let words: Vec<_> = line.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }
        if words[0] == "q" {
            break;
        }
        if words[0] == "settings" {
            if let Some((threshold, minimum, padding, low)) = parse_settings(&words) {
                session.silence.threshold_db = threshold;
                session.silence.minimum_ms = minimum;
                session.silence.edge_padding_ms = padding;
                session.low_level_db = low;
                session.save(&directory)?;
            } else {
                eprintln!("設定値が不正です。閾値は-120〜0 dB、余白は0〜50 msです。");
            }
            continue;
        }
        let (command, number) = if words.len() == 1 {
            ("r", words[0])
        } else {
            (words[0], words[1])
        };
        let Ok(index) = number.parse::<usize>() else {
            eprintln!("番号を指定してください");
            continue;
        };
        let Some(prompt) = session.prompts.get_mut(index.wrapping_sub(1)) else {
            eprintln!("番号が範囲外です");
            continue;
        };
        let result: Result<(), String> = (|| {
            match command {
                "r" => {
                    let id = uuid::Uuid::new_v4().to_string();
                    let file = format!("prompt-{index}-{id}.wav");
                    println!(
                        "{}\n録音開始。Enter または s で停止（上限 {} 秒）",
                        prompt.text,
                        duration_ms as f64 / 1000.0
                    );
                    let stop = AtomicBool::new(false);
                    let request = MicrophoneRecordingRequest::new(
                        &session.endpoint_id,
                        directory.join(&file),
                        duration_ms,
                    );
                    let mut last_ms = 0;
                    let report = record_microphone_to_wav_controlled(
                        &request,
                        &stop,
                        |peak, rms, elapsed| {
                            match lines.try_recv() {
                                Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {
                                    stop.store(true, Ordering::Relaxed)
                                }
                                Err(mpsc::TryRecvError::Empty) => {}
                            }
                            if elapsed >= last_ms + 250 {
                                let warning = if peak >= 0.999 {
                                    " クリッピング"
                                } else if !prompt.room_tone
                                    && peak < 10.0f32.powf(session.low_level_db / 20.0)
                                {
                                    " 入力が小さい"
                                } else {
                                    ""
                                };
                                eprint!(
                                    "\r{:.1}秒 Peak {:.1} dBFS / RMS {:.1} dBFS{}    ",
                                    elapsed as f64 / 1000.0,
                                    20.0 * peak.max(1e-9).log10(),
                                    20.0 * rms.max(1e-9).log10(),
                                    warning
                                );
                                last_ms = elapsed;
                            }
                        },
                    );
                    eprintln!();
                    let mut take = Take {
                        id: id.clone(),
                        recorded_at: chrono::Utc::now().to_rfc3339(),
                        file: file.clone(),
                        endpoint_id: session.endpoint_id.clone(),
                        duration_ms,
                        status: "failed".into(),
                        quality: None,
                        error: None,
                        derived: Vec::new(),
                    };
                    match report {
                        Ok(report) => {
                            take.duration_ms = report.wav.audio_duration_ms;
                            let (_, samples) = read_wav(&directory.join(&file))?;
                            let q = quality(&samples, session.low_level_db, prompt.room_tone);
                            println!("Peak {:.1} dBFS / RMS {:.1} dBFS / クリップ {} sample（連続最大 {}）", 20.0 * q.peak.max(1e-9).log10(), 20.0 * q.rms.max(1e-9).log10(), q.clipped_samples, q.longest_clip_run);
                            if q.low_level_warning || q.clipped_samples > 0 {
                                println!("品質警告：原本を保存しました。再録音、または a {index} でこのテイクを採用してください。");
                                take.status = "warning".into();
                            } else {
                                take.status = "success".into();
                                prompt.accepted_take = Some(id);
                            }
                            take.quality = Some(q);
                        }
                        Err(error) => {
                            take.error = Some(error.to_string());
                            eprintln!("録音失敗: {error}");
                        }
                    }
                    prompt.takes.push(take);
                }
                "a" => {
                    let take = prompt
                        .takes
                        .iter()
                        .rev()
                        .find(|t| t.status != "failed")
                        .ok_or("採用可能な録音がありません")?;
                    prompt.accepted_take = Some(take.id.clone());
                }
                "p" | "d" | "t" => {
                    let take = prompt
                        .takes
                        .iter_mut()
                        .rev()
                        .find(|t| t.status != "failed")
                        .ok_or("録音がありません")?;
                    let source = sample_path(&directory, &take.file)?;
                    if command == "t" {
                        let file = format!("short-{}.wav", uuid::Uuid::new_v4());
                        let edit =
                            derive_shortened(&source, &directory.join(&file), &session.silence)?;
                        println!(
                            "短縮版を保存: {}（{} → {} frames）。原音・採用テイクは保持します。",
                            file, edit.original_frames, edit.output_frames
                        );
                        take.derived.push(Derived {
                            file,
                            source_take_id: take.id.clone(),
                            edit,
                        });
                    } else {
                        let path = if command == "d" {
                            sample_path(
                                &directory,
                                &take.derived.last().ok_or("短縮版がありません")?.file,
                            )?
                        } else {
                            source
                        };
                        preview(&path, &lines)?;
                    }
                }
                _ => return Err("不明な操作です".into()),
            }
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("{error}");
        }
        session.save(&directory)?;
    }
    Ok(())
}

fn parse_settings(words: &[&str]) -> Option<(f32, u32, u32, f32)> {
    if words.len() != 5 {
        return None;
    }
    let threshold: f32 = words[1].parse().ok()?;
    let minimum: u32 = words[2].parse().ok()?;
    let padding: u32 = words[3].parse().ok()?;
    let low: f32 = words[4].parse().ok()?;
    if !(-120.0..=0.0).contains(&threshold)
        || !(-120.0..=0.0).contains(&low)
        || padding > 50
        || minimum < 100
    {
        return None;
    }
    Some((threshold, minimum, padding, low))
}

fn preview(path: &std::path::Path, lines: &mpsc::Receiver<String>) -> Result<(), String> {
    use audio_mixer_backend::{
        audio::buffer::AudioRingBuffer, recording::codecs::decode_audio_file,
        windows::wasapi_render::start_render,
    };
    let audio = decode_audio_file(path).map_err(|e| e.to_string())?;
    let device = list_devices(DeviceDirection::Render)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|d| d.is_default)
        .ok_or("既定の再生デバイスがありません")?;
    let ring = AudioRingBuffer::new(audio.samples.len().max(2));
    ring.push_slice(&audio.samples);
    let render = start_render(device.endpoint_id, ring).map_err(|e| e.to_string())?;
    render
        .wait_until_running(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    println!("確認再生中（Enter で停止）");
    let _ = lines.recv_timeout(Duration::from_secs_f64(
        audio.samples.len() as f64 / 96000.0 + 0.1,
    ));
    render.stop();
    Ok(())
}

fn next_value(
    arguments: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, String> {
    arguments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{option}の値を指定してください"))
}

fn parse_u64(value: String, option: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("{option}には0以上の整数を指定してください: {value}"))
}

fn select_device<'a>(
    devices: &'a [AudioDeviceInfo],
    endpoint_id: Option<&str>,
    device_name: Option<&str>,
) -> Result<&'a AudioDeviceInfo, String> {
    if let Some(endpoint_id) = endpoint_id {
        return devices
            .iter()
            .find(|device| device.endpoint_id.eq_ignore_ascii_case(endpoint_id))
            .ok_or_else(|| format!("Capture Endpointが見つかりません: {endpoint_id}"));
    }
    let device_name = device_name.ok_or_else(|| "マイク名がありません".to_string())?;
    let matches: Vec<_> = devices
        .iter()
        .filter(|device| device.friendly_name.eq_ignore_ascii_case(device_name))
        .collect();
    match matches.as_slice() {
        [device] => Ok(*device),
        [] => Err(format!("Captureデバイスが見つかりません: {device_name}")),
        _ => Err(format!(
            "同名のCaptureデバイスが複数あります。--endpoint-idを指定してください: {device_name}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_interactive_and_rejects_mixed_output() {
        let action =
            parse_args(["--interactive", "--directory", "samples"].map(String::from)).unwrap();
        assert!(
            matches!(action, Action::Interactive { directory, .. } if directory == PathBuf::from("samples"))
        );
        assert!(parse_args(["--interactive", "--output", "x.wav"].map(String::from)).is_err());
        assert!(parse_args(["--interactive", "--duration-ms", "0"].map(String::from)).is_err());
    }

    #[test]
    fn validates_settings_and_manifest_paths() {
        assert!(parse_settings(&["settings", "-45", "100", "10", "-40"]).is_some());
        assert!(parse_settings(&["settings", "NaN", "100", "10", "-40"]).is_none());
        assert!(sample_path(std::path::Path::new("samples"), "../secret.wav").is_err());
        assert!(sample_path(std::path::Path::new("samples"), "C:\\secret.wav").is_err());
        assert!(sample_path(std::path::Path::new("samples"), "take.wav").is_ok());
    }

    #[test]
    fn parses_recording_options_without_ui() {
        let action = parse_args([
            "--device".into(),
            "MIC (BRIDGE CAST V2)".into(),
            "--output".into(),
            "capture.wav".into(),
            "--duration-ms".into(),
            "2500".into(),
        ])
        .unwrap();
        assert_eq!(
            action,
            Action::Record(RecordOptions {
                device_name: Some("MIC (BRIDGE CAST V2)".into()),
                endpoint_id: None,
                output_path: PathBuf::from("capture.wav"),
                duration_ms: 2500,
                startup_timeout_ms: DEFAULT_MICROPHONE_STARTUP_TIMEOUT_MS,
            })
        );
    }

    #[test]
    fn rejects_ambiguous_device_selector() {
        let error = parse_args([
            "--device".into(),
            "mic".into(),
            "--endpoint-id".into(),
            "endpoint".into(),
            "--output".into(),
            "capture.wav".into(),
        ])
        .unwrap_err();
        assert!(error.contains("どちらか一方"));
    }
}
