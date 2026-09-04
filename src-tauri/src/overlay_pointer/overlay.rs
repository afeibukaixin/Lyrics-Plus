use std::time::{Duration, Instant};

use crate::overlay_effect::{HORIZONTAL_OVERLAY_SURFACE_INSET, VERTICAL_OVERLAY_SURFACE_INSET};
use crate::overlay_model::OverlayOrientation;
use crate::overlay_placement::{
    NotchPointerSamplePayload, NOTCH_POINTER_SAMPLE_EVENT, OVERLAY_HOVER_EVENT,
    OVERLAY_POINTER_MONITOR_INTERVAL, UNLOCK_HANDLE_BACKGROUND_GAP, UNLOCK_HANDLE_HIDE_DELAY,
    UNLOCK_HANDLE_HOVER_EVENT,
};
use crate::overlay_pointer::geometry::{
    point_in_window_bounds, should_hover_overlay, stable_overlay_hover, unlock_handle_position,
};
use crate::AppState;
use tauri::{Emitter, Manager};

pub(crate) fn position_unlock_handle(app: &tauri::AppHandle) {
    let (Some(overlay), Some(handle)) = (
        app.get_webview_window("lyrics-overlay"),
        app.get_webview_window("lyrics-unlock-handle"),
    ) else {
        return;
    };
    let (Ok(position), Ok(size), Ok(handle_size)) = (
        overlay.outer_position(),
        overlay.outer_size(),
        handle.outer_size(),
    ) else {
        return;
    };
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let orientation = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .orientation;
    let placement = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .toolbar_placement
        .normalized(orientation);
    let scale = overlay.scale_factor().unwrap_or(1.0);
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let surface_inset = (match orientation {
        OverlayOrientation::Horizontal => HORIZONTAL_OVERLAY_SURFACE_INSET,
        OverlayOrientation::Vertical => VERTICAL_OVERLAY_SURFACE_INSET,
    } * scale)
        .round() as u32;
    let background_gap = (UNLOCK_HANDLE_BACKGROUND_GAP * scale).round() as u32;
    let _ = handle.set_position(unlock_handle_position(
        placement,
        position,
        size,
        handle_size,
        surface_inset,
        background_gap,
    ));
}

pub(crate) fn sync_unlock_handle(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let settings = state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    if settings.visible && !crate::surface_is_destroying(app, "lyrics-overlay") {
        crate::cancel_surface_destroy(app, "lyrics-unlock-handle");
    }
    if crate::surface_is_destroying(app, "lyrics-overlay") {
        let _ = crate::hide_surface(app, "lyrics-unlock-handle");
        return;
    }
    let Some(overlay) = app.get_webview_window("lyrics-overlay") else {
        let _ = crate::hide_surface(app, "lyrics-unlock-handle");
        return;
    };
    let should_show = settings.visible && settings.locked && overlay.is_visible().unwrap_or(false);
    if !should_show {
        let _ = crate::hide_surface(app, "lyrics-unlock-handle");
        return;
    }
    if crate::surface_is_destroying(app, "lyrics-unlock-handle") {
        return;
    }
    if app.get_webview_window("lyrics-unlock-handle").is_none() {
        if let Err(error) = crate::create_unlock_handle(app) {
            log::warn!("Failed to create overlay unlock handle: {error}");
            return;
        }
    }
    position_unlock_handle(app);
    super::wake_overlay_pointer_monitor(app);
}

pub(crate) fn start_overlay_pointer_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_inside_at: Option<Instant> = None;
        let mut last_handle_hovered: Option<bool> = None;
        let mut last_overlay_hovered: Option<bool> = None;

        loop {
            let has_visible_consumer = ["lyrics-overlay", "lyrics-notch"].iter().any(|label| {
                app.get_webview_window(label)
                    .and_then(|window| window.is_visible().ok())
                    .unwrap_or(false)
            });
            if !has_visible_consumer {
                last_inside_at = None;
                last_handle_hovered = None;
                last_overlay_hovered = None;
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
            let settings = state
                .overlay_settings
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone();

            if let Some(notch) = app.get_webview_window("lyrics-notch") {
                if notch.is_visible().unwrap_or(false) {
                    // 只上报坐标，实际 hover 区域由前端根据当前 Visual Island rect 判断。
                    if let (Ok(cursor), Ok(position), Ok(scale_factor)) = (
                        app.cursor_position(),
                        notch.outer_position(),
                        notch.scale_factor(),
                    ) {
                        let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
                            scale_factor
                        } else {
                            1.0
                        };
                        let _ = notch.emit(
                            NOTCH_POINTER_SAMPLE_EVENT,
                            NotchPointerSamplePayload {
                                client_x: (cursor.x - f64::from(position.x)) / scale,
                                client_y: (cursor.y - f64::from(position.y)) / scale,
                            },
                        );
                    }
                }
            }

            let Some(overlay) = app.get_webview_window("lyrics-overlay") else {
                continue;
            };
            let overlay_visible = overlay.is_visible().unwrap_or(false);

            let overlay_sample = match (
                app.cursor_position(),
                overlay.outer_position(),
                overlay.outer_size(),
            ) {
                (Ok(cursor), Ok(position), Ok(size)) => Some((cursor, position, size)),
                _ => None,
            };
            let sampled_overlay_hover = overlay_visible
                && overlay_sample
                    .as_ref()
                    .is_some_and(|(cursor, position, size)| {
                        should_hover_overlay(&settings, *cursor, *position, *size)
                    });
            let overlay_hovered = stable_overlay_hover(
                last_overlay_hovered,
                sampled_overlay_hover,
                overlay_visible
                    && settings.visible
                    && !settings.locked
                    && crate::primary_mouse_button_pressed(),
            );
            if last_overlay_hovered != Some(overlay_hovered) {
                let _ = overlay.emit(OVERLAY_HOVER_EVENT, overlay_hovered);
                last_overlay_hovered = Some(overlay_hovered);
            }

            if !settings.visible || !settings.locked || !overlay_visible {
                last_inside_at = None;
                if let Some(handle) = app.get_webview_window("lyrics-unlock-handle") {
                    if handle.is_visible().unwrap_or(false) {
                        let _ = handle.hide();
                    }
                    if last_handle_hovered != Some(false) {
                        let _ = handle.emit(UNLOCK_HANDLE_HOVER_EVENT, false);
                        last_handle_hovered = Some(false);
                    }
                }
                continue;
            }

            let Some(handle) = app.get_webview_window("lyrics-unlock-handle") else {
                continue;
            };

            let sample = (overlay_sample, handle.outer_position(), handle.outer_size());
            let (should_show, hovered) = match sample {
                (
                    Some((cursor, overlay_position, overlay_size)),
                    Ok(handle_position),
                    Ok(handle_size),
                ) => {
                    let now = Instant::now();
                    let inside_overlay =
                        point_in_window_bounds(cursor, overlay_position, overlay_size);
                    if inside_overlay {
                        last_inside_at = Some(now);
                    }
                    let within_hide_delay = last_inside_at.is_some_and(|last_inside| {
                        now.duration_since(last_inside) < UNLOCK_HANDLE_HIDE_DELAY
                    });
                    (
                        inside_overlay || within_hide_delay,
                        inside_overlay
                            && point_in_window_bounds(cursor, handle_position, handle_size),
                    )
                }
                _ => {
                    // 读取系统鼠标或窗口边界失败时优先保留解锁入口，下一轮继续重试。
                    last_inside_at = None;
                    (true, false)
                }
            };

            if should_show != handle.is_visible().unwrap_or(false) {
                if should_show {
                    position_unlock_handle(&app);
                    let _ = handle.show();
                } else {
                    let _ = handle.hide();
                }
            }
            let hovered = should_show && hovered;
            if last_handle_hovered != Some(hovered) {
                let _ = handle.emit(UNLOCK_HANDLE_HOVER_EVENT, hovered);
                last_handle_hovered = Some(hovered);
            }
        }
    });
}
