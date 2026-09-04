#![cfg(not(target_os = "macos"))]

use tauri::{Manager, WebviewWindowBuilder};

use super::notch::notch_monitor_id;
use super::platform::apply_joining_other_apps_fullscreen;
use crate::AppState;

#[cfg(not(target_os = "macos"))]
fn restore_status_bar_position(app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };
    let monitor_id = state
        .storage
        .get_preference("lyrics-status-bar.last-monitor")
        .ok()
        .flatten()
        .or_else(|| {
            app.primary_monitor()
                .ok()
                .flatten()
                .map(|monitor| notch_monitor_id(&monitor))
        });
    let raw = monitor_id
        .as_deref()
        .and_then(|id| {
            state
                .storage
                .get_preference(&format!("lyrics-status-bar.position.{id}"))
                .ok()
                .flatten()
        })
        .or_else(|| {
            state
                .storage
                .get_preference("lyrics-status-bar.position")
                .ok()
                .flatten()
        });
    let Some(raw) = raw else {
        return false;
    };
    let Some((x, y)) = raw.split_once(',') else {
        return false;
    };
    let (Ok(x), Ok(y)) = (x.parse::<i32>(), y.parse::<i32>()) else {
        return false;
    };
    let position = tauri::PhysicalPosition::new(x, y);
    let visible = app.available_monitors().ok().is_some_and(|monitors| {
        monitors.iter().any(|monitor| {
            let origin = monitor.position();
            let size = monitor.size();
            position.x >= origin.x
                && position.y >= origin.y
                && position.x < origin.x.saturating_add(size.width as i32)
                && position.y < origin.y.saturating_add(size.height as i32)
        })
    });
    visible && window.set_position(position).is_ok()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn position_status_bar_window_default(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) {
    let Some(monitor) = app.primary_monitor().ok().flatten() else {
        return;
    };
    let scale = monitor.scale_factor().max(1.0);
    let size = window
        .outer_size()
        .unwrap_or(tauri::PhysicalSize::new(360, 36));
    let right_gap = (96.0 * scale).round() as i32;
    let top_gap = (3.0 * scale).round() as i32;
    let x = monitor.position().x + monitor.size().width as i32 - size.width as i32 - right_gap;
    let y = monitor.position().y + top_gap;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

#[cfg(not(target_os = "macos"))]
pub(super) fn create_status_bar_lyrics_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("lyrics-status-bar").is_some() {
        return Ok(());
    }
    let config = app.state::<AppState>().config.snapshot();
    let appearance = &config.lyrics.displays.status_bar.appearance;
    let height = appearance.font_size as f64 + 12.0;
    let window = WebviewWindowBuilder::new(
        app,
        "lyrics-status-bar",
        crate::webview_url(app, "index.html?view=lyrics-status-bar"),
    )
    .title("Lyrics Plus 菜单栏歌词")
    .inner_size(appearance.width as f64, height.max(26.0))
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .focusable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .build()?;
    apply_joining_other_apps_fullscreen(&window)?;
    window.set_ignore_cursor_events(true)?;
    if !restore_status_bar_position(app, &window) {
        position_status_bar_window_default(app, &window);
    }
    Ok(())
}
