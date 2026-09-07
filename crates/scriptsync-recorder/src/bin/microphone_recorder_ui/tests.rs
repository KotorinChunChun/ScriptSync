use super::*;
fn state() -> Arc<Mutex<State>> {
    let directory =
        std::env::temp_dir().join(format!("vm-recorder-ui-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    Arc::new(Mutex::new(State {
        session: Session::new("fixture".into()),
        directory: directory.canonicalize().unwrap(),
        devices: vec![],
        duration_ms: 1000,
        recording: None,
        prompt: None,
        levels: (0., 0., 0),
        message: String::new(),
    }))
}
fn http(state: Arc<Mutex<State>>, request: &str) -> String {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = listener.local_addr().unwrap();
    let request = request.replace("HOST", &address.to_string());
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let response = handle(
            &mut stream,
            &address.to_string(),
            "secret",
            &state,
            &AtomicBool::new(false),
        )
        .unwrap_or_else(|error| (400, "text/plain", error.into_bytes()));
        write_response(&mut stream, response).unwrap()
    });
    let mut client = TcpStream::connect(address).unwrap();
    client.write_all(request.as_bytes()).unwrap();
    let mut output = String::new();
    client.read_to_string(&mut output).unwrap();
    worker.join().unwrap();
    output
}
#[test]
fn http_requires_token_and_does_not_record_on_page_load() {
    let state = state();
    let response = http(state.clone(), "GET / HTTP/1.1\r\nHost: HOST\r\n\r\n");
    assert!(response.contains("HTTP/1.1 200"));
    assert!(state.lock().unwrap().recording.is_none());
    let body = r#"{"action":"state"}"#;
    let invalid = http(
        state.clone(),
        &format!(
            "POST /api HTTP/1.1\r\nHost: HOST\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert!(invalid.contains("400"));
    let valid=http(state.clone(),&format!("POST /api HTTP/1.1\r\nHost: HOST\r\nX-Recorder-Token: secret\r\nContent-Length: {}\r\n\r\n{body}",body.len()));
    assert!(valid.contains("200"));
    assert!(valid.contains("室内音"));
    assert!(state.lock().unwrap().recording.is_none());
    let foreign=http(state.clone(),&format!("POST /api HTTP/1.1\r\nHost: HOST\r\nOrigin: https://other.example\r\nX-Recorder-Token: secret\r\nContent-Length: {}\r\n\r\n{body}",body.len()));
    assert!(foreign.contains("400"));
    let root = state.lock().unwrap().directory.clone();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn synthetic_shortening_preserves_original_and_blocks_external_paths() {
    let state = state();
    let root = state.lock().unwrap().directory.clone();
    let source = root.join("source.wav");
    let mut writer = hound::WavWriter::create(
        &source,
        hound::WavSpec {
            channels: 1,
            sample_rate: 1000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for sample in vec![0f32; 500].into_iter().chain(vec![0.2; 100]) {
        writer.write_sample(sample).unwrap()
    }
    writer.finalize().unwrap();
    let before = std::fs::read(&source).unwrap();
    state.lock().unwrap().session.prompts[0].takes.push(Take {
        id: "fixture".into(),
        recorded_at: "test".into(),
        file: "source.wav".into(),
        endpoint_id: "fixture".into(),
        duration_ms: 600,
        status: "success".into(),
        quality: None,
        error: None,
        derived: vec![],
    });
    execute(
        Command {
            action: "shorten".into(),
            index: Some(0),
            endpoint: None,
        },
        &state,
        &AtomicBool::new(false),
    )
    .unwrap();
    let current = state.lock().unwrap();
    let derived = &current.session.prompts[0].takes[0].derived[0];
    assert_eq!(derived.edit.output_frames, 200);
    assert_eq!(before, std::fs::read(&source).unwrap());
    assert!(safe_audio(&root, "../source.wav").is_err());
    drop(current);
    std::fs::remove_dir_all(root).unwrap();
}
