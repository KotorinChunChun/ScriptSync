//! プロジェクトの表示名と、所有する入口・データフォルダーの連動改名。
use crate::{domain::Result,storage::{Store,json_new}};
use std::{fs,path::PathBuf};
fn valid_name(name:&str)->Result<String>{
 let name=name.trim();let stem=name.split('.').next().unwrap_or("").to_ascii_uppercase();
 if name.is_empty()||name.len()>200||name.ends_with('.')||name.chars().any(|c|c.is_control()||"<>:\"/\\|?*".contains(c))||matches!(stem.as_str(),"CON"|"PRN"|"AUX"|"NUL")||(1..=9).any(|n|stem==format!("COM{n}")||stem==format!("LPT{n}")) {return Err("ファイル名として使用できるプロジェクト名を入力してください".into());}
 Ok(name.to_owned())
}
impl Store {
 pub fn rename_project(&mut self,name:&str)->Result<Option<String>>{
  let name=valid_name(name)?;
  let old_file=self.project_file.as_ref().ok_or("プロジェクトファイルを開いてから名前を変更してください")?.canonicalize().map_err(|e|e.to_string())?;
  let old_root=self.root.canonicalize().map_err(|e|e.to_string())?;
  let inside=old_file.parent()==Some(old_root.as_path());
  let parent=if inside{old_root.parent()}else{old_file.parent()}.ok_or("保存先が不正です")?;
  let new_root=parent.join(format!("{name}.scriptsync.data"));
  let new_file=if inside{new_root.join(format!("{name}.scriptsync"))}else{parent.join(format!("{name}.scriptsync"))};
  if new_root==old_root&&new_file==old_file {
   let before=self.project.title.clone();self.project.title=name;
   if let Err(error)=self.save(){self.project.title=before;return Err(error);}return Ok(None);
  }
  if new_root.exists()||new_file.exists(){return Err("同名のプロジェクトファイルまたはデータフォルダーが既にあります".into());}
  let temp_name=format!("project-rename-{}.tmp",crate::domain::id());
  let staged=old_root.join(&temp_name);
  let data=if inside{".".to_owned()}else{new_root.file_name().unwrap().to_string_lossy().into_owned()};
  json_new(&staged,&serde_json::json!({"version":1,"data_directory":data}))?;
  if let Err(error)=self.move_root(&new_root){let _=fs::remove_file(&staged);return Err(format!("データフォルダーの名前を変更できません: {error}"));}
  let staged=new_root.join(&temp_name);
  // create_new相当の公開。競合する入口を置き換えない。
  if let Err(error)=fs::hard_link(&staged,&new_file){
   let rollback=self.move_root(&old_root);
   let _=fs::remove_file(if rollback.is_ok(){old_root.join(&temp_name)}else{staged});
   if rollback.is_err(){self.root=new_root;self.project_file=Some(if inside{self.root.join(old_file.file_name().unwrap())}else{old_file});return Err(format!("入口の作成に失敗しました。データは {} に保持しています: {error}",self.root.display()));}
   return Err(format!("プロジェクト名を変更できません: {error}"));
  }
  let _=fs::remove_file(&staged);
  let old_project=self.project.clone();self.root=new_root.clone();self.project_file=Some(new_file.clone());self.project.title=name;
  if let Err(error)=self.save(){
   self.project=old_project;
   if self.move_root(&old_root).is_ok(){
    let created=if inside{old_root.join(new_file.file_name().unwrap())}else{new_file};let _=fs::remove_file(created);
    self.root=old_root;self.project_file=Some(old_file);
    return Err(format!("保存に失敗したため名前変更を取り消しました: {error}"));
   }
   return Err(format!("保存に失敗しました。データは {}、入口は {} に保持しています: {error}",self.root.display(),new_file.display()));
  }
  let obsolete:PathBuf=if inside{new_root.join(old_file.file_name().unwrap())}else{old_file};
  match fs::remove_file(&obsolete){Ok(())=>Ok(None),Err(error)=>Ok(Some(format!("名前変更は完了しました。旧入口 {} の削除だけに失敗しました: {error}",obsolete.display())))}
 }
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn rename_moves_owned_files_and_reopens_without_losing_audio(){
  let dir=std::env::temp_dir().join(crate::domain::id());fs::create_dir_all(&dir).unwrap();
  let old=dir.join("旧.scriptsync");let mut store=Store::create_project_file(old.clone()).unwrap();
  fs::create_dir_all(store.root.join("takes")).unwrap();fs::write(store.root.join("takes/keep.wav"),b"fixture").unwrap();let id=store.project.id.clone();
  store.rename_project("新しい名前").unwrap();let path=store.project_file.clone().unwrap();
  assert!(!old.exists());assert!(!dir.join("旧.scriptsync.data").exists());assert_eq!(fs::read(store.root.join("takes/keep.wav")).unwrap(),b"fixture");
  assert!(store.rename_project("../bad").is_err());fs::write(dir.join("重複.scriptsync"),b"keep").unwrap();assert!(store.rename_project("重複").is_err());
  drop(store);let reopened=Store::open_project_file(path).unwrap();assert_eq!(reopened.project.title,"新しい名前");assert_eq!(reopened.project.id,id);drop(reopened);fs::remove_dir_all(dir).unwrap();
 }
 #[test]
 fn legacy_inside_manifest_and_save_failure_are_safe(){
  let dir=std::env::temp_dir().join(crate::domain::id());let root=dir.join("legacy");let store=Store::open(root.clone(),true).unwrap();drop(store);
  let mut store=Store::open_project_file(root.join("project.json")).unwrap();store.rename_project("移行済み").unwrap();let path=store.project_file.clone().unwrap();assert!(path.exists());drop(store);
  let mut store=Store::open_project_file(path.clone()).unwrap();let root=store.root.clone();let title=store.project.title.clone();
  store.project.generation-=1;assert!(store.rename_project("保存失敗").is_err());assert_eq!(store.root,root);assert_eq!(store.project.title,title);assert!(path.exists());assert!(!dir.join("保存失敗.scriptsync.data").exists());drop(store);fs::remove_dir_all(dir).unwrap();
 }
}
