use crate::{app::App,domain::*,storage::*};
use serde_json::json;
#[test]
fn draft_validation_does_not_persist_and_new_source_commits_atomically(){
 let root=std::env::temp_dir().join(id());let store=Store::open(root.clone(),true).unwrap();
 let app=App::default();app.0.lock().unwrap().store=Some(store);
 let before=app.snapshot().unwrap()["project"].clone();
 let source=id();let payload=json!({"project_id":before["id"],"base_revision":before["script_revision"],"base_generation":before["generation"],"sources":[{"id":source,"name":"追加.md","kind":"markdown","text":"こんにちは","approved_text":"こんにちは","lines":[{"line":0,"enabled":true}]}],"deleted_source_ids":[]});
 app.dispatch("script_validate_draft",payload.clone()).unwrap();
 assert_eq!(app.snapshot().unwrap()["project"],before);
 app.dispatch("save_project",json!({})).unwrap();
 app.dispatch("close_project",json!({})).unwrap();
 let store=Store::open(root.clone(),false).unwrap();assert!(store.project.script_sources.is_empty());
 app.0.lock().unwrap().store=Some(store);
 let mut commit=payload;commit["base_generation"]=app.snapshot().unwrap()["project"]["generation"].clone();
 app.dispatch("script_commit",commit).unwrap();
 assert_eq!(app.snapshot().unwrap()["project"]["script_sources"][0]["confirmed_text"],"こんにちは");
 app.dispatch("close_project",json!({})).unwrap();
 let store=Store::open(root.clone(),false).unwrap();assert_eq!(store.project.segments.len(),1);drop(store);drop(app);std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "ネイティブ画面確認用の非機密fixtureを指定先へ新規作成して保持する"]
fn create_native_v004_fixture() {
 let path=std::path::PathBuf::from(std::env::var("SCRIPTSYNC_V004_FIXTURE").expect("fixture保存先が必要です"));
 assert!(!path.exists());
 let store=Store::create_project_file(path).unwrap();let root=store.root.clone();let app=App::default();app.0.lock().unwrap().store=Some(store);
 let before=app.snapshot().unwrap()["project"].clone();let source=id();
 app.dispatch("script_commit",json!({"project_id":before["id"],"base_revision":before["script_revision"],"base_generation":before["generation"],"sources":[{"id":source,"name":"確認原稿.md","kind":"markdown","text":"最初の確認用文章です。\n次の確認用文章です。","approved_text":"最初の確認用文章です。\n次の確認用文章です。","lines":[{"line":0,"enabled":true},{"line":1,"enabled":true}]}],"deleted_source_ids":[]})).unwrap();
 let wav=root.join("fixture.wav");let mut writer=hound::WavWriter::create(&wav,hound::WavSpec{channels:1,sample_rate:48000,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();
 for n in 0..96000 { writer.write_sample(((n as f64*440.0*std::f64::consts::TAU/48000.0).sin()*1500.0) as i16).unwrap(); } writer.finalize().unwrap();
 let mut inner=app.0.lock().unwrap();let store=inner.store.as_mut().unwrap();
 for segment in &mut store.project.segments { for _ in 0..2 { let take=crate::app::take_from_file(&root,&wav,"synthetic".into()).unwrap();segment.takes.push(take); } segment.selected=Some(segment.takes[0].id.clone()); }
 store.save().unwrap();
}

#[test]
#[ignore = "ネイティブ検証fixtureの確定原稿をWeb互換試験へ出力して保持する"]
fn export_native_v004_fixture() {
 let path=std::path::PathBuf::from(std::env::var("SCRIPTSYNC_V004_FIXTURE").expect("fixtureが必要です"));
 let output=path.parent().unwrap().join("desktop.zip");let manifest:serde_json::Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
 let project:Project=serde_json::from_slice(&std::fs::read(path.parent().unwrap().join(manifest["data_directory"].as_str().unwrap()).join("project.json")).unwrap()).unwrap();
 crate::exchange::export_script(&project,&project.script_sources[0].id,&output,"2026-09-12T09:30:00.000Z").unwrap();
}
