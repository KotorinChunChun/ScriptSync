//! Windows Authenticode署名の検証（WinVerifyTrust）。
//!
//! 「有効な署名があること」の確認に加え、証明書チェーンから発行者の
//! 簡易表示名を取り出せる。ダウンロードしたファイルの改ざん検知や、
//! 想定した配布元からのファイルであることの確認に使う。
//!
//! 使い方: このファイルをプロジェクトに `authenticode.rs` としてコピーし、
//! `mod authenticode;` で読み込む。Cargo.tomlには以下のフィーチャが必要
//! （`cargo add windows --features "..."` で一括追加できる）。
//!
//! ```toml
//! [dependencies]
//! windows = { version = "0.62", features = [
//!     "Win32_Foundation",
//!     "Win32_Security_WinTrust",
//!     "Win32_Security_Cryptography",
//!     "Win32_Security_Cryptography_Catalog",
//!     "Win32_Security_Cryptography_Sip",
//! ] }
//! ```
//!
//! ## 呼び出し方
//!
//! ```ignore
//! match authenticode::get_signer_name(&path) {
//!     Ok(signer) if signer == "Expected Publisher, Inc." => { /* OK */ }
//!     Ok(other) => { /* 署名は有効だが発行者が違う */ }
//!     Err(AuthenticodeError::Invalid) => { /* 署名なし・改ざん・ファイル自体が壊れている */ }
//! }
//! ```

use std::fmt;
use std::os::windows::ffi::OsStrExt as _;
use std::path::Path;

use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Security::Cryptography::{CERT_NAME_SIMPLE_DISPLAY_TYPE, CertGetNameStringW};
use windows::Win32::Security::WinTrust::{
    WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
    WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_SAFER_FLAG, WTD_STATEACTION_CLOSE,
    WTD_STATEACTION_VERIFY, WTD_UI_NONE, WTHelperGetProvCertFromChain,
    WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData, WinVerifyTrust,
};
use windows::core::PCWSTR;

#[derive(Debug)]
pub enum AuthenticodeError {
    /// 署名が存在しない、改ざんされている、証明書チェーンが信頼できない等。
    /// WinVerifyTrustは「ファイルが存在しない」場合もこの結果を返すため、
    /// 呼び出し側で事前に `path.exists()` を確認しておくこと（下記「罠」参照）。
    Invalid,
}

impl fmt::Display for AuthenticodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthenticodeError::Invalid => write!(f, "デジタル署名が無効です"),
        }
    }
}

impl std::error::Error for AuthenticodeError {}

/// 署名が有効であることだけを確認する（発行者は問わない）。
#[allow(dead_code)]
pub fn verify_signed(path: &Path) -> Result<(), AuthenticodeError> {
    get_signer_name(path).map(|_| ())
}

/// 署名を検証し、有効であれば発行者の簡易表示名（証明書のCN相当）を返す。
/// 無効な場合は発行者名に関わらず `Err` になる。
pub fn get_signer_name(path: &Path) -> Result<String, AuthenticodeError> {
    let wide_path: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let file_info = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wide_path.as_ptr()),
        hFile: HANDLE::default(),
        pgKnownSubject: std::ptr::null_mut(),
    };

    let mut trust_data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &file_info as *const _ as *mut _,
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_SAFER_FLAG,
        ..Default::default()
    };

    let mut action_guid = WINTRUST_ACTION_GENERIC_VERIFY_V2;

    // SAFETY: trust_data・file_info・action_guidはこの呼び出しの間ずっと有効な
    // スタック上の値を指しており、WinVerifyTrustはそれらを読むだけで所有権を
    // 奪わない。呼び出し後は必ずWTD_STATEACTION_CLOSEでステートを解放する。
    let verify_result = unsafe {
        WinVerifyTrust(
            HWND::default(),
            &mut action_guid,
            &mut trust_data as *mut _ as *mut _,
        )
    };

    // 発行者名の取得は、ステートをクローズする前に行う必要がある
    // （hWVTStateDataはクローズ後は無効になる）。
    let signer_name = if verify_result == 0 {
        // SAFETY: 直前のWinVerifyTrust(VERIFY)が成功しており、hWVTStateDataは
        // このプロセス内で有効なハンドルとして設定されている。
        unsafe { extract_signer_name(&trust_data) }
    } else {
        None
    };

    trust_data.dwStateAction = WTD_STATEACTION_CLOSE;
    unsafe {
        WinVerifyTrust(
            HWND::default(),
            &mut action_guid,
            &mut trust_data as *mut _ as *mut _,
        );
    }

    if verify_result != 0 {
        return Err(AuthenticodeError::Invalid);
    }

    signer_name.ok_or(AuthenticodeError::Invalid)
}

/// 検証済みトラストデータの証明書チェーンから、署名者の簡易表示名を取り出す。
///
/// # Safety
/// `trust_data.hWVTStateData` は `WTD_STATEACTION_VERIFY` の呼び出しが成功した
/// 直後の状態（`WTD_STATEACTION_CLOSE` する前）でなければならない。
unsafe fn extract_signer_name(trust_data: &WINTRUST_DATA) -> Option<String> {
    unsafe {
        let prov_data = WTHelperProvDataFromStateData(trust_data.hWVTStateData);
        if prov_data.is_null() {
            return None;
        }
        let signer = WTHelperGetProvSignerFromChain(prov_data, 0, false, 0);
        if signer.is_null() {
            return None;
        }
        let cert = WTHelperGetProvCertFromChain(signer, 0);
        if cert.is_null() {
            return None;
        }
        let cert_context = (*cert).pCert;
        if cert_context.is_null() {
            return None;
        }

        let len = CertGetNameStringW(cert_context, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0, None, None);
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        let written = CertGetNameStringW(
            cert_context,
            CERT_NAME_SIMPLE_DISPLAY_TYPE,
            0,
            None,
            Some(&mut buf),
        );
        if written == 0 {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    }
}
