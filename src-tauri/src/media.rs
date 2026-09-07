use crate::{app::App, audio, domain::*, storage::*};
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
pub fn prepared(root: &Path, file: &str, expected: &str) -> Result<PathBuf> {
    let source = safe(root, file)?;
    if hash_file(&source)? != expected {
        return Err("音声が差し替えられています".into());
    }
    let reader = hound::WavReader::open(&source).map_err(|e| e.to_string())?;
    if reader.spec() == audio::spec() {
        return Ok(source);
    }
    let relative = format!("previews/{expected}.wav");
    let target = safe(root, &relative)?;
    if !target.exists() {
        let bytes = audio::preview(&source)?;
        write_new(&target, &bytes)?;
    }
    Ok(target)
}
pub fn response(
    app: &App,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let answer = (|| -> Result<tauri::http::Response<Vec<u8>>> {
        let token = request.uri().path().trim_start_matches('/');
        let path = app
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .media
            .get(token)
            .cloned()
            .ok_or("音声セッションがありません")?;
        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        let range = request
            .headers()
            .get("range")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("bytes=0-");
        let (start, end) = byte_range(range, size)?;
        let mut bytes = vec![0; (end - start + 1) as usize];
        file.seek(SeekFrom::Start(start))
            .and_then(|_| file.read_exact(&mut bytes))
            .map_err(|e| e.to_string())?;
        tauri::http::Response::builder()
            .status(206)
            .header("Content-Type", "audio/wav")
            .header("Accept-Ranges", "bytes")
            .header("Access-Control-Allow-Origin", "*")
            .header("Content-Range", format!("bytes {start}-{end}/{size}"))
            .header("Content-Length", bytes.len())
            .body(bytes)
            .map_err(|e| e.to_string())
    })();
    answer.unwrap_or_else(|e| {
        tauri::http::Response::builder()
            .status(416)
            .body(e.into_bytes())
            .unwrap()
    })
}
pub fn byte_range(range: &str, size: u64) -> Result<(u64, u64)> {
    if size == 0 {
        return Err("空音声です".into());
    }
    let pair = range
        .strip_prefix("bytes=")
        .and_then(|s| s.split_once('-'))
        .ok_or("範囲が不正です")?;
    let (start, end) = if pair.0.is_empty() {
        let suffix = pair.1.parse::<u64>().map_err(|e| e.to_string())?;
        (size.saturating_sub(suffix), size - 1)
    } else {
        (
            pair.0.parse::<u64>().map_err(|e| e.to_string())?,
            if pair.1.is_empty() {
                size - 1
            } else {
                pair.1
                    .parse::<u64>()
                    .map_err(|e| e.to_string())?
                    .min(size - 1)
            },
        )
    };
    if start >= size || end < start {
        return Err("範囲外です".into());
    }
    Ok((start, end.min(start + 4 * 1024 * 1024 - 1)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_are_bounded() {
        assert_eq!(byte_range("bytes=1-3", 10).unwrap(), (1, 3));
        assert_eq!(byte_range("bytes=-2", 10).unwrap(), (8, 9));
        assert!(byte_range("bytes=10-", 10).is_err());
        assert_eq!(
            byte_range("bytes=0-", 10_000_000).unwrap().1,
            4 * 1024 * 1024 - 1
        );
    }
}
