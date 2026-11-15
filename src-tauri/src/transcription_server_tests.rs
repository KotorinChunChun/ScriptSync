use super::*;
use crate::{app::App, domain::id, storage::Store};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::PathBuf,
    thread,
    time::Instant,
};

const TEST_TOKEN: &str = "synthetic-test-token";

fn health(mode: &str) -> Value {
    json!({
        "ok": true, "ready": true, "acceptingJobs": true, "serverVersion": "0.1.17",
        "protocolVersion": 3, "authMode": mode, "token": TEST_TOKEN,
        "capabilities":{"openaiAudio":true,"modelDiscovery":true}
    })
}

fn catalog() -> Value {
    let row = |engine: &str, model: &str, task: &str, availability: &str, language: &str| {
        json!({
            "id":format!("{engine}/{model}"),
            "x_meeting":{
                "engineId":engine,"modelId":model,"task":task,"availability":availability,
                "runtimeReady":true,"supported":true,"supportedLanguages":[language]
            }
        })
    };
    json!({"object":"list","data":[
        row("whisper","tiny","transcription","download_on_first_use","ja"),
        row("openai","gpt-transcribe","transcription","provider","ja"),
        row("diarization","sortformer","diarization","installed","ja"),
        row("speech","en-only","transcription","installed","en"),
        row("haas","haas-ja-v1.0","transcription","not_installed","ja")
    ]})
}

fn mock(
    state: Value,
    models: bool,
    post: Option<(u16, Value, Vec<u8>)>,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let handle = thread::spawn(move || {
        let mut steps = vec![("GET", "/api/health", 200, state, None)];
        if models {
            steps.push(("GET", "/v1/models", 200, catalog(), None));
        }
        if let Some((status, value, audio)) = post {
            steps.push((
                "POST",
                "/v1/audio/transcriptions",
                status,
                value,
                Some(audio),
            ));
        }
        for (method, path, status, value, audio) in steps {
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("模擬サーバーの接続待ちに失敗しました: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line.trim_end(), format!("{method} {path} HTTP/1.1"));
            let mut headers = std::collections::HashMap::new();
            loop {
                line.clear();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                let (name, value) = line.split_once(':').unwrap();
                headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
            }
            if let Some(audio) = audio {
                assert!(
                    headers
                        .get("x-local-token")
                        .is_some_and(|value| value == TEST_TOKEN)
                );
                assert!(headers["content-type"].starts_with("multipart/form-data; boundary="));
                let mut bytes = vec![0; headers["content-length"].parse().unwrap()];
                reader.read_exact(&mut bytes).unwrap();
                assert!(bytes.windows(audio.len()).any(|part| part == audio));
                let body = String::from_utf8_lossy(&bytes);
                for field in [
                    "name=\"file\"; filename=\"take.wav\"",
                    "name=\"model\"\r\n\r\nwhisper/tiny",
                    "name=\"language\"\r\n\r\nja",
                    "name=\"response_format\"\r\n\r\njson",
                ] {
                    assert!(body.contains(field), "multipartの必須項目がありません");
                }
            } else {
                assert!(!headers.contains_key("x-local-token"));
            }
            let body = value.to_string();
            write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    (url, handle)
}

fn test_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("scriptsync-server-{}", id()));
    fs::create_dir(&root).unwrap();
    root
}

fn wav(path: &Path) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..4_800 {
        writer.write_sample(0_i16).unwrap();
    }
    writer.finalize().unwrap();
    fs::read(path).unwrap()
}

fn workflow(root: &Path, input: &Path, url: &str, model: &str) -> Value {
    let app = App::default();
    let config = root.join("settings/machine-settings.json");
    app.0.lock().unwrap().machine_settings =
        crate::machine_settings::MachineSettings::load(config.clone(), root.join("asr"));
    let mut settings = app.dispatch("machine_settings", json!({})).unwrap()["settings"].clone();
    assert_eq!(settings["asr_provider"], "local");
    settings["asr_provider"] = PROVIDER.into();
    settings["asr_server_url"] = url.into();
    settings["asr_server_model"] = model.into();
    app.dispatch("settings_save", json!({"settings":settings}))
        .unwrap();
    let saved = crate::machine_settings::MachineSettings::load(config.clone(), root.join("asr"));
    assert_eq!(saved.settings.asr_provider, PROVIDER);
    assert_eq!(saved.settings.asr_server_model, model);

    let path = root.join("project.scriptsync");
    app.dispatch("new", json!({"path":path})).unwrap();
    let initial = app.snapshot().unwrap()["project"].clone();
    let baseline = "確定済みの台本です。";
    app.dispatch("script_commit", json!({
        "project_id":initial["id"],"base_revision":initial["script_revision"],"base_generation":initial["generation"],
        "sources":[{"id":id(),"name":"合成台本.md","kind":"markdown","text":baseline,"approved_text":baseline,
                   "lines":[{"line":0,"enabled":true}]}],"deleted_source_ids":[]
    })).unwrap();
    let confirmed = app.snapshot().unwrap()["project"].clone();
    let segment = confirmed["segments"][0]["id"].as_str().unwrap();
    app.dispatch("import_audio", json!({"segment":segment,"path":input}))
        .unwrap();
    let imported = app.snapshot().unwrap()["project"].clone();
    let take = imported["segments"][0]["takes"][0]["id"].as_str().unwrap();
    app.enqueue_recording_asr(segment, take, false).unwrap();
    assert_eq!(app.snapshot().unwrap()["project"], imported);
    app.dispatch("recording_asr", json!({"segment":segment,"take":take}))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(640);
    while app.0.lock().unwrap().auto_asr_running {
        assert!(
            Instant::now() < deadline,
            "文字起こしの完了待ちが時間切れです"
        );
        thread::sleep(Duration::from_millis(30));
    }
    let recognized = app.snapshot().unwrap()["project"].clone();
    let result = recognized["segments"][0]["transcriptions"][0].clone();
    assert_eq!(result["provider"], PROVIDER);
    assert_eq!(result["take_id"], take);
    assert_eq!(result["needs_review"], true);
    assert!(recognized["segments"][0]["recording_draft"].is_null());
    let recognized_text = result["raw"]
        .as_str()
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| panic!("文字起こし失敗: {}", result["error"]));
    assert_eq!(result["status"], "succeeded");
    assert_ne!(recognized_text, baseline);

    // 録音画面の「修正案へ反映」が呼ぶ通常の保存処理。
    app.dispatch(
        "recording_draft",
        json!({"segment":segment,"text":recognized_text}),
    )
    .unwrap();
    app.dispatch("save_project", json!({})).unwrap();
    app.dispatch("close_project", json!({})).unwrap();
    let reopened = Store::open_project_file(path).unwrap();
    assert_eq!(
        reopened.project.segments[0].recording_draft.as_deref(),
        Some(recognized_text)
    );
    assert_eq!(reopened.project.segments[0].text, baseline);
    assert_eq!(reopened.project.script_sources[0].confirmed_text, baseline);
    assert_eq!(
        reopened.project.script_revision,
        confirmed["script_revision"].as_u64().unwrap()
    );
    assert!(!fs::read_to_string(config).unwrap().contains(TEST_TOKEN));
    result
}

#[test]
fn lan_transcription_reaches_only_the_recording_draft() {
    let root = test_root();
    let path = root.join("fixture.wav");
    let audio = wav(&path);
    let (url, server) = mock(
        health("lan"),
        true,
        Some((
            200,
            json!({"text":"合成の認識結果です。","token":TEST_TOKEN}),
            audio,
        )),
    );
    let result = workflow(&root, &path, &url, DEFAULT_MODEL);
    assert_eq!(result["raw"], "合成の認識結果です。");
    assert!(!result.to_string().contains(TEST_TOKEN));
    server.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_unready_unsupported_and_unauthenticated_requests() {
    for (field, value, expected) in [
        ("ready", json!(false), "受け付け"),
        ("acceptingJobs", json!(false), "受け付け"),
        ("protocolVersion", json!(4), "プロトコル"),
        ("authMode", json!("bearer"), "認証方式"),
        ("token", Value::Null, "一時トークン"),
    ] {
        let mut state = health("local");
        state[field] = value;
        let (url, server) = mock(state, false, None);
        assert!(check(&url).err().unwrap().contains(expected));
        server.join().unwrap();
    }
    let (url, server) = mock(health("local"), true, None);
    let info = check(&url).unwrap();
    assert_eq!(info.models.len(), 1);
    assert_eq!(info.models[0].id, DEFAULT_MODEL);
    assert!(!serde_json::to_string(&info).unwrap().contains(TEST_TOKEN));
    server.join().unwrap();

    let root = test_root();
    let input = root.join("fixture.wav");
    let audio = wav(&input);
    let hash = hash_bytes(&audio);
    let (url, server) = mock(health("local"), true, None);
    assert!(
        transcribe(&url, "openai/gpt-transcribe", &input, &hash)
            .unwrap_err()
            .contains("モデル")
    );
    server.join().unwrap();
    let (url, server) = mock(
        health("local"),
        true,
        Some((403, json!({"error":{"message":TEST_TOKEN}}), audio)),
    );
    let error = transcribe(&url, DEFAULT_MODEL, &input, &hash).unwrap_err();
    assert!(error.contains("HTTP 403") && !error.contains(TEST_TOKEN));
    server.join().unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "受付可能な実機のTranscriptionServerと合成音声WAVが必要"]
fn real_server_transcription_reaches_only_the_recording_draft() {
    let url = std::env::var("SCRIPTSYNC_TEST_ASR_URL").unwrap_or_else(|_| DEFAULT_URL.into());
    let model = std::env::var("SCRIPTSYNC_TEST_ASR_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into());
    let audio = PathBuf::from(
        std::env::var_os("SCRIPTSYNC_TEST_ASR_WAV")
            .expect("合成音声WAVの絶対パスを指定してください"),
    );
    assert!(audio.is_absolute());
    let root = test_root();
    let app = App::default();
    let info = app
        .dispatch("asr_server_check", json!({"url":url}))
        .unwrap();
    assert_eq!(info["protocol"], 3);
    assert!(
        info["models"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["id"] == model)
    );
    let result = workflow(&root, &audio, &url, &model);
    assert_eq!(result["model"], model);
    println!(
        "TranscriptionServer {} / {} / {}: 音声認識、修正案の保存と再読込、確定台本の保持を確認",
        info["version"], info["auth_mode"], model
    );
    fs::remove_dir_all(root).unwrap();
}
