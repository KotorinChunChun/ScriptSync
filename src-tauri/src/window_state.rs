use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

const MIN_WIDTH: u32 = 960;
const MIN_HEIGHT: u32 = 650;
const STATE_FILE: &str = "window-state.json";
const MIN_VISIBLE_TITLEBAR_WIDTH: i32 = 64;
const MIN_VISIBLE_TITLEBAR_HEIGHT: i32 = 32;

#[derive(Clone, Copy, Deserialize, Serialize)]
struct SavedWindowState {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy)]
struct MonitorBounds {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

fn state_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("読みあわせ");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    Ok(directory.join(STATE_FILE))
}

fn usable_monitor(
    state: SavedWindowState,
    monitors: impl IntoIterator<Item = MonitorBounds>,
) -> Option<MonitorBounds> {
    let right = state
        .x
        .saturating_add(state.width.min(i32::MAX as u32) as i32);
    let bottom = state
        .y
        .saturating_add(state.height.min(i32::MAX as u32) as i32);
    monitors.into_iter().find(|monitor| {
        let monitor_right = monitor
            .x
            .saturating_add(monitor.width.min(i32::MAX as u32) as i32);
        let monitor_bottom = monitor
            .y
            .saturating_add(monitor.height.min(i32::MAX as u32) as i32);
        let horizontal = right
            .min(monitor_right)
            .saturating_sub(state.x.max(monitor.x));
        let vertical = bottom
            .min(monitor_bottom)
            .saturating_sub(state.y.max(monitor.y));
        horizontal >= MIN_VISIBLE_TITLEBAR_WIDTH && vertical >= MIN_VISIBLE_TITLEBAR_HEIGHT
    })
}

fn clamp_to_monitor(state: SavedWindowState, monitor: MonitorBounds) -> SavedWindowState {
    let width = state.width.max(MIN_WIDTH).min(monitor.width);
    let height = state.height.max(MIN_HEIGHT).min(monitor.height);
    let right = monitor
        .x
        .saturating_add(monitor.width.saturating_sub(width).min(i32::MAX as u32) as i32);
    let bottom = monitor
        .y
        .saturating_add(monitor.height.saturating_sub(height).min(i32::MAX as u32) as i32);
    SavedWindowState {
        x: state.x.clamp(monitor.x, right),
        y: state.y.clamp(monitor.y, bottom),
        width,
        height,
    }
}

pub fn restore_before_show(window: &WebviewWindow) {
    let Ok(path) = state_path(&window.app_handle()) else {
        return;
    };
    let Ok(bytes) = fs::read(path) else {
        return;
    };
    let Ok(saved) = serde_json::from_slice::<SavedWindowState>(&bytes) else {
        return;
    };
    let Ok(monitors) = window.available_monitors() else {
        return;
    };
    let monitors = monitors.into_iter().map(|monitor| {
        let position = monitor.position();
        let size = monitor.size();
        MonitorBounds {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        }
    });
    let Some(saved) =
        usable_monitor(saved, monitors).map(|monitor| clamp_to_monitor(saved, monitor))
    else {
        return;
    };
    let _ = window.set_size(PhysicalSize::new(saved.width, saved.height));
    let _ = window.set_position(PhysicalPosition::new(saved.x, saved.y));
}

pub fn save(window: &WebviewWindow) {
    let (Ok(position), Ok(size), Ok(path)) = (
        window.outer_position(),
        window.inner_size(),
        state_path(&window.app_handle()),
    ) else {
        return;
    };
    let state = SavedWindowState {
        x: position.x,
        y: position.y,
        width: size.width.max(MIN_WIDTH),
        height: size.height.max(MIN_HEIGHT),
    };
    let Ok(bytes) = serde_json::to_vec(&state) else {
        return;
    };
    let temporary = path.with_extension("json.tmp");
    if fs::write(&temporary, bytes).is_ok() {
        let _ = fs::rename(temporary, path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_at_least_the_configured_minimum_size() {
        let state = SavedWindowState {
            x: 0,
            y: 0,
            width: 1,
            height: 2,
        };
        assert_eq!(state.width.max(MIN_WIDTH), MIN_WIDTH);
        assert_eq!(state.height.max(MIN_HEIGHT), MIN_HEIGHT);
    }

    #[test]
    fn ignores_a_window_with_only_a_one_pixel_intersection() {
        let state = SavedWindowState {
            x: 1919,
            y: 0,
            width: 100,
            height: 100,
        };
        let monitor = MonitorBounds {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert!(usable_monitor(state, [monitor]).is_none());
    }

    #[test]
    fn restores_a_visible_window_inside_its_monitor() {
        let state = SavedWindowState {
            x: 1800,
            y: 100,
            width: 1000,
            height: 700,
        };
        let monitor = MonitorBounds {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let restored = clamp_to_monitor(state, usable_monitor(state, [monitor]).unwrap());
        assert_eq!(restored.x, 920);
        assert_eq!(restored.width, 1000);
    }
}
