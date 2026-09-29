fn snapped_position(
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) -> tauri::PhysicalPosition<i32> {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return position;
    };
    let Ok(size) = window.outer_size() else {
        return position;
    };
    let origin = monitor.position();
    let monitor_size = monitor.size();
    let right = origin.x + monitor_size.width as i32 - size.width as i32;
    let bottom = origin.y + monitor_size.height as i32 - size.height as i32;
    tauri::PhysicalPosition::new(
        snap_coordinate(position.x, origin.x, right),
        snap_coordinate(position.y, origin.y, bottom),
    )
}

fn snap_coordinate(value: i32, start: i32, end: i32) -> i32 {
    if value.abs_diff(start) <= 12 {
        start
    } else if value.abs_diff(end) <= 12 {
        end
    } else {
        value
    }
}

fn remember_overlay_geometry_monitor(app: &tauri::AppHandle, monitor: &tauri::Monitor) {
    let state = app.state::<AppState>();
    let id = crate::monitor_id(monitor);
    if let Err(error) = state.storage.set_preference("overlay.last_monitor", &id) {
        log::warn!("保存桌面歌词尺寸所属显示器失败：{error}");
    }
    *state
        .overlay_monitor
        .write()
        .unwrap_or_else(|error| error.into_inner()) = Some(id);
}

/// macOS 的窗口位置由 AppKit 保存；拖动结束仅同步逐屏尺寸偏好与解锁按钮。
pub(crate) fn persist_overlay_state_at(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    _position: tauri::PhysicalPosition<i32>,
) {
    if let Ok(Some(monitor)) = window.current_monitor() {
        remember_overlay_geometry_monitor(app, &monitor);
    }
    crate::position_unlock_handle(app);
}

pub(crate) fn persist_overlay_style_frame(
    app: &tauri::AppHandle,
    monitor: &tauri::Monitor,
    _position: tauri::PhysicalPosition<i32>,
    _size: tauri::PhysicalSize<u32>,
) {
    remember_overlay_geometry_monitor(app, monitor);
    if let Some(window) = app.get_webview_window("lyrics-overlay") {
        crate::windows::save_geometry_on_target(app, &window);
    }
    crate::position_unlock_handle(app);
}

pub(crate) fn overlay_target_monitor(
    _app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Option<tauri::Monitor> {
    window.current_monitor().ok().flatten()
}

pub(crate) fn cancel_overlay_pending_fit(_app: &tauri::AppHandle) {}

/// 用户重置位置时清除 AppKit 旧外框，并把新的主屏位置记为目标。
pub(crate) fn reset_native_overlay_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    crate::windows::clear_native_frame_autosave(
        app,
        crate::windows::OVERLAY_FRAME_AUTOSAVE_NAME,
    )
    .map_err(|error| error.to_string())?;
    let monitor = window
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "没有可用的显示器".to_string())?;
    let work = monitor.work_area();
    let size = window.outer_size().map_err(|error| error.to_string())?;
    let position = tauri::PhysicalPosition::new(
        work.position.x + (work.size.width.saturating_sub(size.width) / 2) as i32,
        work.position.y + 72,
    );
    window
        .set_position(position)
        .map_err(|error| error.to_string())?;
    crate::windows::reset_screen_affinity(app, window);
    crate::position_unlock_handle(app);
    Ok(())
}
