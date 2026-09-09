//! 画面が再読込・消失した場合、録音を取消せず保存へ進める。
use crate::app::App;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

pub fn attach(app: &App) -> bool {
    crate::hotkeys::stop();
    let mut state = app.0.lock().unwrap();
    let recovering = state.busy || state.queue_stop.is_some();
    if let Some(stop) = &state.stop {
        stop.store(true, Ordering::Release);
    }
    if let Some(stop) = &state.queue_stop {
        stop.store(true, Ordering::Release);
    }
    state.ui_last_seen = Some(Instant::now());
    state.ui_interrupted = false;
    if recovering {
        state.message = "画面を復旧しています。録音を停止して保存します".into();
    }
    recovering
}

pub fn lost(app: &App) {
    crate::hotkeys::stop();
    if let Ok(mut state) = app.0.lock() {
        if let Some(stop) = &state.stop {
            stop.store(true, Ordering::Release);
        }
        if let Some(stop) = &state.queue_stop {
            stop.store(true, Ordering::Release);
        }
        state.ui_interrupted = true;
        state.ui_last_seen = None;
    }
}

pub fn heartbeat(app: &App) -> bool {
    if let Ok(mut state) = app.0.lock() {
        state.ui_last_seen = Some(Instant::now());
        state.ui_interrupted
    } else {
        true
    }
}
fn expired(app: &App, now: Instant) -> bool {
    app.0
        .lock()
        .ok()
        .and_then(|state| state.ui_last_seen)
        .is_some_and(|last| now.saturating_duration_since(last) > Duration::from_secs(8))
}

pub fn start_watchdog(app: &App) {
    let weak = std::sync::Arc::downgrade(&app.0);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let Some(inner) = weak.upgrade() else { break };
            let app = App(inner);
            if expired(&app, Instant::now()) {
                lost(&app);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::AtomicBool};
    #[test]
    fn reconnect_stops_capture_without_cancelling_saved_or_pending_take() {
        let app = App::default();
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut s = app.0.lock().unwrap();
            s.busy = true;
            s.job = "recording".into();
            s.stop = Some(stop.clone());
            s.cancel_recording = Some(cancel.clone());
        }
        assert!(attach(&app));
        assert!(stop.load(Ordering::Acquire));
        assert!(!cancel.load(Ordering::Acquire));
        assert!(app.0.lock().unwrap().busy); // 保存完了前にロックを偽って解除しない。
    }
    #[test]
    fn missing_ui_expires_but_background_heartbeats_keep_session_alive() {
        let app = App::default();
        heartbeat(&app);
        assert!(!expired(&app, Instant::now()));
        app.0.lock().unwrap().ui_last_seen = Some(Instant::now() - Duration::from_secs(9));
        assert!(expired(&app, Instant::now()));
        lost(&app);
        assert!(app.0.lock().unwrap().ui_last_seen.is_none());
    }
}
