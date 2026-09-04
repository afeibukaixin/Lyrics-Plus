use std::time::{Duration, Instant};

use crate::overlay_placement::{OVERLAY_POINTER_MONITOR_INTERVAL, UNLOCK_HANDLE_HIDE_DELAY};
use crate::overlay_pointer::geometry::point_in_window_bounds;
use crate::AppState;
use tauri::{Emitter, Manager};

pub(crate) fn position_list_unlock_handle(app: &tauri::AppHandle) {
    let (Some(list), Some(handle)) = (
        app.get_webview_window("lyrics-list"),
        app.get_webview_window("lyrics-list-unlock-handle"),
    ) else {
        return;
    };
    let (Ok(position), Ok(size), Ok(handle_size), Ok(scale)) = (
        list.outer_position(),
        list.outer_size(),
        handle.outer_size(),
        list.scale_factor(),
    ) else {
        return;
    };
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let inset = (12.0 * scale).round() as u32;
    let x = position.x.saturating_add(
        size.width
            .saturating_sub(handle_size.width)
            .saturating_sub(inset) as i32,
    );
    let y = position.y.saturating_add(inset as i32);
    let _ = handle.set_position(tauri::PhysicalPosition::new(x, y));
}

pub(crate) const LIST_UNLOCK_HANDLE_HOVER_EVENT: &str = "lyrics-list-unlock-handle://hover";

pub(crate) fn sync_list_unlock_handle(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let preferences = state.config.snapshot().lyrics.displays.list_window;
    if preferences.enabled && !crate::surface_is_destroying(app, "lyrics-list") {
        crate::cancel_surface_destroy(app, "lyrics-list-unlock-handle");
    }
    if crate::surface_is_destroying(app, "lyrics-list") {
        let _ = crate::hide_surface(app, "lyrics-list-unlock-handle");
        return;
    }
    let Some(list) = app.get_webview_window("lyrics-list") else {
        let _ = crate::hide_surface(app, "lyrics-list-unlock-handle");
        return;
    };
    let should_show =
        preferences.enabled && preferences.locked && list.is_visible().unwrap_or(false);
    if !should_show {
        let _ = crate::hide_surface(app, "lyrics-list-unlock-handle");
        return;
    }
    if crate::surface_is_destroying(app, "lyrics-list-unlock-handle") {
        return;
    }
    if app
        .get_webview_window("lyrics-list-unlock-handle")
        .is_none()
    {
        if let Err(error) = crate::create_list_unlock_handle(app) {
            log::warn!("Failed to create lyrics list unlock handle: {error}");
            return;
        }
    }
    position_list_unlock_handle(app);
    super::wake_overlay_pointer_monitor(app);
}

fn schedule_list_unlock_handle_sync(app: &tauri::AppHandle) {
    let target = app.clone();
    if let Err(error) = app.run_on_main_thread(move || sync_list_unlock_handle(&target)) {
        log::warn!("Failed to schedule lyrics list unlock handle synchronization: {error}");
    }
}

pub(crate) fn start_list_unlock_handle_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_inside_at: Option<Instant> = None;
        let mut last_handle_hovered: Option<bool> = None;

        loop {
            let list_visible = app
                .get_webview_window("lyrics-list")
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(false);
            if !list_visible {
                last_inside_at = None;
                last_handle_hovered = None;
                if let Some(handle) = app.get_webview_window("lyrics-list-unlock-handle") {
                    let _ = handle.hide();
                }
                if let Some(state) = app.try_state::<AppState>() {
                    let wake = state.pointer_monitor_wake.clone();
                    wake.notified().await;
                } else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                continue;
            }
            tokio::time::sleep(OVERLAY_POINTER_MONITOR_INTERVAL).await;

            let Some(state) = app.try_state::<AppState>() else {
                continue;
            };
            let preferences = state.config.snapshot().lyrics.displays.list_window;
            let Some(list) = app.get_webview_window("lyrics-list") else {
                continue;
            };
            let visible = list.is_visible().unwrap_or(false);
            if !visible || !preferences.enabled || !preferences.locked {
                last_inside_at = None;
                if let Some(handle) = app.get_webview_window("lyrics-list-unlock-handle") {
                    if handle.is_visible().unwrap_or(false) {
                        let _ = handle.hide();
                    }
                    if last_handle_hovered != Some(false) {
                        let _ = handle.emit(LIST_UNLOCK_HANDLE_HOVER_EVENT, false);
                        last_handle_hovered = Some(false);
                    }
                }
                continue;
            }

            let Some(handle) = app.get_webview_window("lyrics-list-unlock-handle") else {
                schedule_list_unlock_handle_sync(&app);
                continue;
            };
            let sample = (
                app.cursor_position(),
                list.outer_position(),
                list.outer_size(),
                handle.outer_position(),
                handle.outer_size(),
            );
            let (should_show, hovered) = match sample {
                (
                    Ok(cursor),
                    Ok(list_position),
                    Ok(list_size),
                    Ok(handle_position),
                    Ok(handle_size),
                ) => {
                    let now = Instant::now();
                    let inside_list = point_in_window_bounds(cursor, list_position, list_size);
                    if inside_list {
                        last_inside_at = Some(now);
                    }
                    let within_hide_delay = last_inside_at.is_some_and(|last_inside| {
                        now.duration_since(last_inside) < UNLOCK_HANDLE_HIDE_DELAY
                    });
                    (
                        inside_list || within_hide_delay,
                        inside_list && point_in_window_bounds(cursor, handle_position, handle_size),
                    )
                }
                _ => {
                    last_inside_at = None;
                    (true, false)
                }
            };

            if should_show != handle.is_visible().unwrap_or(false) {
                if should_show {
                    position_list_unlock_handle(&app);
                    let _ = handle.show();
                } else {
                    let _ = handle.hide();
                }
            }
            let hovered = should_show && hovered;
            if last_handle_hovered != Some(hovered) {
                let _ = handle.emit(LIST_UNLOCK_HANDLE_HOVER_EVENT, hovered);
                last_handle_hovered = Some(hovered);
            }
        }
    });
}
