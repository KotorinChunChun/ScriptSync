use std::sync::{Mutex, OnceLock};
use tauri::Emitter;
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static MONITOR: Mutex<Option<(u32, std::thread::JoinHandle<()>)>> = Mutex::new(None);
pub fn install(handle: tauri::AppHandle) {
    let _ = APP.set(handle);
}
#[tauri::command]
pub fn continuous_hotkeys(enabled: bool) -> Result<(), String> {
    if !enabled {
        stop();
        return Ok(());
    }
    let mut monitor = MONITOR.lock().map_err(|e| e.to_string())?;
    if monitor.is_some() {
        return Ok(());
    }
    let app = APP.get().ok_or("キー監視が初期化されていません")?.clone();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let thread = std::thread::Builder::new()
        .name("scriptsync-hotkeys".into())
        .spawn(move || unsafe {
            use windows_sys::Win32::{
                System::Threading::GetCurrentThreadId,
                UI::{
                    Input::KeyboardAndMouse::{
                        MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_F1,
                    },
                    WindowsAndMessaging::{GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, WM_HOTKEY},
                },
            };
            let mut message: MSG = std::mem::zeroed();
            PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
            let mut registered = Vec::new();
            for key in 0..4 {
                if RegisterHotKey(
                    std::ptr::null_mut(),
                    key + 1,
                    MOD_NOREPEAT,
                    VK_F1 as u32 + key as u32,
                ) == 0
                {
                    for id in registered {
                        UnregisterHotKey(std::ptr::null_mut(), id);
                    }
                    let _ = tx.send(Err(format!(
                        "F{}を登録できません。他のアプリで使用中の可能性があります",
                        key + 1
                    )));
                    return;
                }
                registered.push(key + 1);
            }
            let _ = tx.send(Ok(GetCurrentThreadId()));
            while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                if message.message == WM_HOTKEY && (1..=4).contains(&message.wParam) {
                    let _ = app.emit("scriptsync-hotkey", format!("F{}", message.wParam));
                }
            }
            for id in registered {
                UnregisterHotKey(std::ptr::null_mut(), id);
            }
        })
        .map_err(|e| e.to_string())?;
    match rx.recv().map_err(|e| e.to_string())? {
        Ok(id) => {
            *monitor = Some((id, thread));
            Ok(())
        }
        Err(e) => {
            let _ = thread.join();
            Err(e)
        }
    }
}
pub fn stop() {
    if let Ok(mut monitor) = MONITOR.lock() {
        if let Some((id, thread)) = monitor.take() {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW(
                    id,
                    windows_sys::Win32::UI::WindowsAndMessaging::WM_QUIT,
                    0,
                    0,
                );
            }
            let _ = thread.join();
        }
    }
}
