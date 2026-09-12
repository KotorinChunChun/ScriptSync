use super::*;

fn fixture() -> (std::path::PathBuf, Manifest, Vec<u8>) {
    let root = std::env::temp_dir().join(format!("scriptsync-exchange-{}", id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut output = Cursor::new(Vec::new());
    { let mut wav = hound::WavWriter::new(&mut output, hound::WavSpec { channels:1,sample_rate:48000,bits_per_sample:16,sample_format:hound::SampleFormat::Int }).unwrap();
      for _ in 0..4800 { wav.write_sample(1000i16).unwrap(); } wav.finalize().unwrap(); }
    let bytes = output.into_inner();
    let audio = AudioInfo { path:"audio/1-1.wav".into(),mime:"audio/wav".into(),sha256:hash_bytes(&bytes),duration:0.1,sample_rate:48000,channels:1,bits_per_sample:16 };
    let manifest = Manifest { format:"scriptsyncweb-exchange-1".into(),title:"試験".into(),source:"本文".into(),source_file_name:None,script_path:None,exported_at:"2026-09-12T00:00:00Z".into(),scope:"all".into(),lines:vec![ExchangeLine {order:1,text:"本文".into(),selected:Some("web-1".into()),takes:vec![ExchangeTake {id:"web-1".into(),audio,created:None,original:None}]}] };
    (root,manifest,bytes)
}
fn archive(root: &Path, manifest: &Manifest, bytes: &[u8], extra: Option<&str>) -> std::path::PathBuf {
    archive_with_password(root,manifest,bytes,extra,None)
}
fn archive_with_password(root: &Path, manifest: &Manifest, bytes: &[u8], extra: Option<&str>, password: Option<&str>) -> std::path::PathBuf {
    let path = root.join(format!("{}.zip",id()));
    let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    let options = if let Some(password)=password { options.with_aes_encryption(zip::AesMode::Aes256,password) } else { options };
    zip.start_file("project.json",options).unwrap();zip.write_all(&serde_json::to_vec(manifest).unwrap()).unwrap();
    zip.start_file("audio/1-1.wav",options).unwrap();zip.write_all(bytes).unwrap();
    if let Some(original)=&manifest.lines[0].takes[0].original { zip.start_file(&original.path,options).unwrap();zip.write_all(bytes).unwrap(); }
    if let Some(name) = extra { zip.start_file(name,options).unwrap();zip.write_all(b"bad").unwrap(); }
    zip.finish().unwrap();path
}
#[test]
fn aes_requires_correct_key_and_keeps_original_as_attachment() {
    let (root,mut manifest,bytes)=fixture();
    let mut original=manifest.lines[0].takes[0].audio.clone();original.path="audio/original.wav".into();
    manifest.lines[0].takes[0].original=Some(original);
    let path=archive_with_password(&root,&manifest,&bytes,None,Some("123456"));
    assert!(inspect_zip(&path,None).is_err());
    assert!(inspect_zip(&path,Some("654321")).is_err());
    let exchange=inspect_zip(&path,Some("123456")).unwrap();
    let mut store=Store::open(root.join("project"),true).unwrap();
    let segment=Segment::new("本文".into());let sid=segment.id.clone();store.project.segments.push(segment);
    assert_eq!(merge_audio(&mut store,&exchange,&[LineMapping {order:1,segment_id:sid}]).unwrap().added,1);
    let take=&store.project.segments[0].takes[0];
    assert_eq!(std::fs::read(store.root.join(format!("takes/{}/original.wav",take.id))).unwrap(),bytes);
    assert_eq!(store.project.segments[0].takes.len(),1);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn rejects_tamper_and_traversal() {
    let (root,mut manifest,bytes)=fixture();
    assert!(inspect_zip(&archive(&root,&manifest,&bytes,Some("../outside")),None).is_err());
    manifest.lines[0].takes[0].audio.sha256="0".repeat(64);
    assert!(inspect_zip(&archive(&root,&manifest,&bytes,None),None).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn import_deduplicates_and_preserves_selected() {
    let (root,manifest,bytes)=fixture();
    let exchange=inspect_zip(&archive(&root,&manifest,&bytes,None),None).unwrap();
    let mut store=Store::open(root.join("project"),true).unwrap();
    let segment=Segment::new("本文".into());let sid=segment.id.clone();store.project.segments.push(segment);
    let mappings=[LineMapping {order:1,segment_id:sid}];
    assert_eq!(merge_audio(&mut store,&exchange,&mappings).unwrap().added,1);
    assert!(store.project.segments[0].selected.is_none());
    assert_eq!(merge_audio(&mut store,&exchange,&mappings).unwrap().duplicates,1);
    assert_eq!(store.project.segments[0].takes.len(),1);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn failed_save_rolls_back_import_and_missing_mapping_rejects() {
    let (root,manifest,bytes)=fixture();
    let exchange=inspect_zip(&archive(&root,&manifest,&bytes,None),None).unwrap();
    let mut store=Store::open(root.join("project"),true).unwrap();
    let segment=Segment::new("本文".into());let sid=segment.id.clone();store.project.segments.push(segment);
    assert!(merge_audio(&mut store,&exchange,&[]).is_err());
    let next=store.root.join(format!("revisions/{:020}.json",store.project.generation+1));
    std::fs::write(next,b"existing").unwrap();
    assert!(merge_audio(&mut store,&exchange,&[LineMapping {order:1,segment_id:sid}]).is_err());
    assert!(store.project.segments[0].takes.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn export_uses_confirmed_source_and_preserves_reading_order() {
    let (root,_,_)=fixture();
    let mut project=Project::new("作品".into());
    project.script_sources.push(ScriptSource { id:"source".into(),name:"台本.md".into(),kind:"markdown".into(),confirmed_text:"先\n後".into(),draft_text:"未確定本文".into(),lines:vec![],draft_lines:vec![],blank_rows:vec![],approved_text:None });
    for text in ["後","先"] { let mut segment=Segment::new(text.into());segment.source_id="source".into();project.segments.push(segment); }
    let path=root.join("export.zip");
    export_script(&project,"source",&path,"2026-09-12T00:00:00Z").unwrap();
    let inspected=inspect_zip(&path,None).unwrap();
    assert_eq!(inspected.manifest.source,"先\n後");
    assert_eq!(inspected.manifest.source_file_name.as_deref(),Some("台本.md"));
    assert_eq!(inspected.manifest.lines.iter().map(|line|line.text.as_str()).collect::<Vec<_>>(),vec!["後","先"]);
    assert!(inspected.manifest.lines.iter().all(|line|line.takes.is_empty()&&line.selected.is_none()));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn export_rejects_web_limits_before_creating_zip_and_counts_utf16() {
    let (root,_,_)=fixture();
    let mut project=Project::new("作品".into());
    project.script_sources.push(ScriptSource { id:"source".into(),name:"原稿.md".into(),kind:"markdown".into(),confirmed_text:"本文".into(),draft_text:"本文".into(),lines:vec![],draft_lines:vec![],blank_rows:vec![],approved_text:None });
    let path=root.join("limits.zip");
    assert!(export_script(&project,"source",&path,"2026-09-12T00:00:00Z").is_err());assert!(!path.exists());
    let mut segment=Segment::new("本文".into());segment.source_id="source".into();
    project.segments=vec![segment.clone();1001];
    assert!(export_script(&project,"source",&path,"2026-09-12T00:00:00Z").is_err());assert!(!path.exists());
    project.segments=vec![segment];project.title="😀".repeat(51);
    assert!(export_script(&project,"source",&path,"2026-09-12T00:00:00Z").is_err());assert!(!path.exists());
    project.title="😀".repeat(50);project.segments[0].text="😀".repeat(10001);
    assert!(export_script(&project,"source",&path,"2026-09-12T00:00:00Z").is_err());assert!(!path.exists());
    project.segments[0].text="😀".repeat(10000);
    export_script(&project,"source",&path,"2026-09-12T00:00:00Z").unwrap();
    assert!(inspect_zip(&path,None).is_ok());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn import_enforces_web_utf16_limits_too() {
    let (root,mut manifest,bytes)=fixture();
    manifest.title="😀".repeat(51);
    assert!(inspect_zip(&archive(&root,&manifest,&bytes,None),None).is_err());
    manifest.title="作品".into();manifest.lines[0].text="😀".repeat(10001);
    assert!(inspect_zip(&archive(&root,&manifest,&bytes,None),None).is_err());
    manifest.lines[0].text="本文".into();manifest.source_file_name=Some("😀".repeat(128));
    assert!(inspect_zip(&archive(&root,&manifest,&bytes,None),None).is_err());
    manifest.source_file_name=None;manifest.lines[0].takes=vec![manifest.lines[0].takes[0].clone();201];
    assert!(validate_manifest_limits(&manifest).unwrap_err().contains("200件"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn export_guard_blocks_drafts_and_reconfirms_without_losing_takes() {
    let mut p=Project::new("確認".into());
    let sid=id();
    let payload=json!({"base_revision":0,"sources":[{"id":sid,"name":"a.md","kind":"markdown","text":"一行目\n二行目","approved_text":"一行目\n二行目","lines":[{"line":0,"enabled":true},{"line":1,"enabled":true}]}]});
    crate::script_sources::update_draft(&mut p,&payload).unwrap();
    assert!(ensure_export_ready(&p,&sid).is_err());
    crate::script_sources::commit(&mut p,&payload).unwrap();
    assert!(ensure_export_ready(&p,&sid).is_ok());
    let confirmed=p.clone();
    p.segments[0].recording_draft=Some("文字起こし結果".into());
    assert!(ensure_export_ready(&p,&sid).is_err());
    p=confirmed.clone();p.script_sources[0].draft_text="変更".into();assert!(ensure_export_ready(&p,&sid).is_err());
    p=confirmed.clone();p.script_sources[0].draft_lines[0]["enabled"]=json!(false);assert!(ensure_export_ready(&p,&sid).is_err());
    p=confirmed.clone();p.script_sources[0].name="変更名.md".into();assert!(ensure_export_ready(&p,&sid).is_err());
    p=confirmed.clone();p.pending_source_deletions.push(sid.clone());assert!(ensure_export_ready(&p,&sid).is_err());
    p=confirmed.clone();
    assert!(validate_export_request(&p,&json!({"source_id":sid,"project_id":"other"})).is_err());
    assert!(validate_export_request(&p,&json!({"source_id":sid,"base_generation":p.generation+1})).is_err());
    let mut edited=payload.clone();edited["base_revision"]=json!(p.script_revision);edited["sources"][0]["text"]=json!("変更");edited["sources"][0]["approved_text"]=json!("変更");edited["sources"][0]["lines"]=json!([{"line":0,"enabled":true}]);
    crate::script_sources::update_draft(&mut p,&edited).unwrap();assert!(ensure_export_ready(&p,&sid).is_err());
    crate::script_sources::commit(&mut p,&edited).unwrap();assert!(ensure_export_ready(&p,&sid).is_ok());
    p.script_export_fingerprint=None;assert!(ensure_export_ready(&p,&sid).is_ok());
    p.segments[0].text="確定前の変更".into();assert!(ensure_export_ready(&p,&sid).is_err());
    p=confirmed.clone();p.script_sources[0].draft_lines[0]["previous_id"]=json!("別行");assert!(ensure_export_ready(&p,&sid).is_err());
}
