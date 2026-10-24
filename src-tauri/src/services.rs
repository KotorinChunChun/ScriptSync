//! 接続ゲートウェイ経由のAI/音声合成。秘密は環境変数またはユーザー専用設定から読み、プロジェクトに含めない。
use crate::{domain::Result,machine_settings::Settings};
use serde_json::{json,Value};
use std::time::Duration;
pub fn validate_url(value:&str)->Result<()> {let u=reqwest::Url::parse(value).map_err(|_|"接続先URLが不正です")?;if !matches!(u.scheme(),"http"|"https")||u.host_str().is_none()||!u.username().is_empty()||u.password().is_some()||u.query().is_some()||u.fragment().is_some(){return Err("接続先はhttp(s)のURLを指定してください".into())}Ok(())}
fn token()->Result<String>{
 if let Ok(value)=std::env::var("SCRIPTSYNC_GATEWAY_TOKEN"){if !value.trim().is_empty(){return Ok(value)}}
 let root=std::env::var_os("SCRIPTSYNC_SECRET_ROOT").map(std::path::PathBuf::from).unwrap_or_else(||"C:/AI/env/ScriptSync".into());
 let value=std::fs::read_to_string(root.join("gateway-token.txt")).map_err(|_|"接続トークンをSCRIPTSYNC_GATEWAY_TOKENまたはC:/AI/env/ScriptSync/gateway-token.txtへ設定してください")?;let value=value.trim().to_string();if value.is_empty(){return Err("接続トークンが空です".into())}Ok(value)
}
pub fn request(settings:&Settings,path:&str,body:Option<Value>)->Result<Vec<u8>>{
 validate_url(&settings.service_url)?;
 let client=reqwest::blocking::Client::builder().connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(300)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_|"接続クライアントを準備できません")?;
 let url=format!("{}{path}",settings.service_url.trim_end_matches('/'));
 let req=if let Some(body)=body{client.post(url).json(&body)}else{client.get(url)};
 let response=req.bearer_auth(token()?).send().map_err(|_|"サービスに接続できません。起動・URL・認証を確認してください")?;
 if !response.status().is_success(){return Err(format!("接続サーバーがHTTP {}を返しました",response.status().as_u16()))}
 if response.content_length().is_some_and(|n|n>50_000_000){return Err("応答が大きすぎます".into())}
 use std::io::Read;let mut bytes=Vec::new();response.take(50_000_001).read_to_end(&mut bytes).map_err(|_|"応答を取得できません")?;if bytes.len()>50_000_000{return Err("応答が大きすぎます".into())}Ok(bytes)
}
pub fn improve(settings:&Settings,prompt:&str)->Result<String>{let bytes=request(settings,"/v1/text-improve",Some(json!({"provider":settings.ai_provider,"model":settings.codex_model,"prompt":prompt})))?;let data:Value=serde_json::from_slice(&bytes).map_err(|_|"AIの応答が不正です")?;let text=data["text"].as_str().filter(|s|!s.trim().is_empty()&&s.len()<=160_000).ok_or("AIの本文が空か長すぎます")?;Ok(text.trim().into())}
#[cfg(test)]mod tests{use super::*;#[test]fn endpoints_are_not_credentials(){for url in ["http://localhost:4318","https://192.168.11.15:4318"]{assert!(validate_url(url).is_ok())}for url in ["file:///secret","http://user:secret@localhost","https://a/?key=x","https://a/#token"]{assert!(validate_url(url).is_err())}}}
