use std::time::Duration;

use crate::overlay_model::OverlayOrientation;
use crate::overlay_placement::geometry::toolbar_placement_after_move;
use crate::AppState;
use tauri::{Emitter, Manager};

use super::state::ToolbarPlacement;

pub(crate) const UNLOCK_HANDLE_BACKGROUND_GAP: f64 = 6.0;
pub(crate) const OVERLAY_POINTER_MONITOR_INTERVAL: Duration = Duration::from_millis(50);
pub(crate) const UNLOCK_HANDLE_HIDE_DELAY: Duration = Duration::from_millis(200);
pub(crate) const UNLOCK_HANDLE_HOVER_EVENT: &str = "unlock-handle://hover";
pub(crate) const OVERLAY_HOVER_EVENT: &str = "overlay://hover";
pub(crate) const NOTCH_POINTER_SAMPLE_EVENT: &str = "notch://pointer-sample";
pub(crate) const OVERLAY_TOOLBAR_PLACEMENT_EVENT: &str = "overlay://toolbar-placement";

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NotchPointerSamplePayload {
    pub(crate) client_x: f64,
    pub(crate) client_y: f64,
}

pub(crate) fn set_overlay_toolbar_placement(app: &tauri::AppHandle, placement: ToolbarPlacement) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let changed = {
        let mut overlay_placement = state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if overlay_placement.toolbar_placement == placement {
            false
        } else {
            overlay_placement.toolbar_placement = placement;
            true
        }
    };
    if changed {
        let _ = app.emit(OVERLAY_TOOLBAR_PLACEMENT_EVENT, placement);
        if let Some(window) = app.get_webview_window("lyrics-overlay") {
            let style = state
                .overlay_style
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            crate::sync_overlay_vibrancy(&window, &style);
        }
    }
}

pub(crate) fn reset_overlay_toolbar_placement(
    app: &tauri::AppHandle,
    orientation: OverlayOrientation,
) {
    set_overlay_toolbar_placement(app, ToolbarPlacement::for_orientation(orientation));
}

fn overlay_toolbar_move_result(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) -> Option<(ToolbarPlacement, tauri::PhysicalPosition<i32>)> {
    let (Ok(Some(monitor)), Ok(size)) = (window.current_monitor(), window.outer_size()) else {
        return None;
    };
    let state = app.state::<AppState>();
    let orientation = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .orientation;
    let placement = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .toolbar_placement;
    let scale = monitor.scale_factor();
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let work_area = monitor.work_area();
    Some(toolbar_placement_after_move(
        orientation,
        placement,
        position,
        size,
        scale,
        work_area.position,
        work_area.size,
    ))
}

pub(crate) fn adjust_overlay_toolbar_for_move(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) -> tauri::PhysicalPosition<i32> {
    let Some((next_placement, next_position)) = overlay_toolbar_move_result(app, window, position)
    else {
        return position;
    };
    set_overlay_toolbar_placement(app, next_placement);
    next_position
}

/// 原生拖动期间只更新工具栏方位，不修改窗口坐标，避免破坏系统拖动的抓点和流畅性。
pub(crate) fn update_overlay_toolbar_placement_during_drag(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) {
    let Some((next_placement, _)) = overlay_toolbar_move_result(app, window, position) else {
        return;
    };
    set_overlay_toolbar_placement(app, next_placement);
}

pub(crate) fn set_overlay_drag_active(app: &tauri::AppHandle, active: bool) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .drag_active = active;
}

pub(crate) fn overlay_drag_active(app: &tauri::AppHandle) -> bool {
    app.try_state::<AppState>()
        .map(|state| {
            state
                .overlay_placement
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .drag_active
        })
        .unwrap_or(false)
}

/// 原生拖动结束后再统一吸附、调整工具栏位置并保存，避免拖动中重设窗口位置导致抓点漂移。
pub(crate) fn settle_overlay_position_at(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) {
    let snapped = crate::snapped_position(window, position);
    let adjusted = adjust_overlay_toolbar_for_move(app, window, snapped);
    if adjusted != position {
        crate::set_overlay_position(app, window, adjusted);
    }
    crate::persist_overlay_state_at(app, window, adjusted);
}
