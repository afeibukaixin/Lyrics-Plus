use serde::{Deserialize, Serialize};

use crate::window_placement::{
    primary_monitor, LyricsWindowPlacements, WindowPlacement, WorkArea, OVERLAY_POSITION_KEY,
};

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

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedOverlayPlacement {
    placement: WindowPlacement,
    toolbar_placement: ToolbarPlacement,
    horizontal_anchor: HorizontalAnchor,
}

fn legacy_overlay_placement(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    monitors: &[tauri::Monitor],
    primary: Option<&tauri::Monitor>,
) -> Option<SavedOverlayPlacement> {
    let state = app.state::<AppState>();
    let old_id = state
        .storage
        .get_preference("overlay.last_monitor")
        .ok()
        .flatten()?;
    let raw = state
        .storage
        .get_preference(&format!("overlay.position.{old_id}"))
        .ok()
        .flatten()?;
    let bounds: StoredBounds = serde_json::from_str(&raw).ok()?;
    let matching = monitors
        .iter()
        .find(|monitor| monitor_id(monitor) == old_id);
    let fallback = matching.or(primary).or_else(|| monitors.first())?;
    let current_work = WorkArea::of(fallback);
    let work = WorkArea {
        position: tauri::PhysicalPosition::new(
            bounds.work_x.unwrap_or(current_work.position.x),
            bounds.work_y.unwrap_or(current_work.position.y),
        ),
        size: tauri::PhysicalSize::new(
            bounds.work_width.unwrap_or(current_work.size.width),
            bounds.work_height.unwrap_or(current_work.size.height),
        ),
        scale_factor: bounds.scale_factor.unwrap_or(current_work.scale_factor),
    };
    let current_size = window.outer_size().ok()?;
    let size = tauri::PhysicalSize::new(
        bounds.window_width.unwrap_or(current_size.width),
        bounds.window_height.unwrap_or(current_size.height),
    );
    let manual_monitor =
        matching.is_some() && primary.is_some_and(|main| monitor_id(main) != old_id);
    let placement = WindowPlacement::capture(
        old_id,
        manual_monitor,
        tauri::PhysicalPosition::new(bounds.x, bounds.y),
        size,
        work,
    );
    let orientation = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .orientation;
    Some(SavedOverlayPlacement {
        placement,
        toolbar_placement: bounds
            .toolbar_placement
            .unwrap_or_else(|| ToolbarPlacement::for_orientation(orientation))
            .normalized(orientation),
        horizontal_anchor: bounds.horizontal_anchor.unwrap_or(HorizontalAnchor::Free),
    })
}

fn load_overlay_placement(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    monitors: &[tauri::Monitor],
    primary: Option<&tauri::Monitor>,
) -> Option<SavedOverlayPlacement> {
    app.state::<AppState>()
        .storage
        .get_preference(OVERLAY_POSITION_KEY)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .or_else(|| legacy_overlay_placement(app, window, monitors, primary))
}

fn initialize_overlay_placement(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    monitors: &[tauri::Monitor],
    primary: Option<&tauri::Monitor>,
) {
    let placements = app.state::<LyricsWindowPlacements>();
    if placements
        .overlay
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .record
        .is_some()
    {
        return;
    }
    if let Some(saved) = load_overlay_placement(app, window, monitors, primary) {
        let orientation = app
            .state::<AppState>()
            .overlay_style
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .orientation;
        set_overlay_toolbar_placement(app, saved.toolbar_placement.normalized(orientation));
        set_overlay_horizontal_anchor(app, saved.horizontal_anchor);
        placements
            .overlay
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .initialize(Some(saved.placement));
    }
}

fn save_overlay_placement(app: &tauri::AppHandle) {
    let record = app
        .state::<LyricsWindowPlacements>()
        .overlay
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .record
        .clone();
    let Some(placement) = record else {
        return;
    };
    let state = app.state::<AppState>();
    let (toolbar_placement, horizontal_anchor) = {
        let state = state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (state.toolbar_placement, state.horizontal_anchor)
    };
    let saved = SavedOverlayPlacement {
        placement,
        toolbar_placement,
        horizontal_anchor,
    };
    let Ok(raw) = serde_json::to_string(&saved) else {
        return;
    };
    if let Err(error) = state.storage.set_preference(OVERLAY_POSITION_KEY, &raw) {
        log::warn!("保存桌面歌词位置失败：{error}");
        return;
    }
    // 旧键仍供逐显示器的歌词尺寸设置选择使用，不再作为定位数据源。
    let id = &saved.placement.monitor_id;
    let _ = state.storage.set_preference("overlay.last_monitor", id);
    *state
        .overlay_monitor
        .write()
        .unwrap_or_else(|error| error.into_inner()) = Some(id.to_owned());
    crate::position_unlock_handle(app);
}

fn place_overlay(app: &tauri::AppHandle, window: &tauri::WebviewWindow, startup: bool) {
    let monitors = window.available_monitors().unwrap_or_default();
    let primary = primary_monitor(window, &monitors);
    let Some(main) = primary.as_ref() else {
        return;
    };
    initialize_overlay_placement(app, window, &monitors, Some(main));
    let placements = app.state::<LyricsWindowPlacements>();
    let mut runtime = placements
        .overlay
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    runtime.displays_changed(&monitors, Some(main));
    let Some(target) = runtime.target(&monitors, Some(main)).cloned() else {
        return;
    };
    if let Ok(position) = window.outer_position() {
        runtime.mark_programmatic(position);
    }
    drop(runtime);
    if let Ok(current_size) = window.outer_size() {
        let work = target.work_area();
        let fitted = tauri::PhysicalSize::new(
            current_size.width.min(work.size.width),
            current_size.height.min(work.size.height),
        );
        if fitted != current_size {
            let _ = window.set_size(fitted);
        }
    }
    let Ok(size) = window.outer_size() else {
        return;
    };
    let work = target.work_area();
    let default = tauri::PhysicalPosition::new(
        work.position.x + (work.size.width.saturating_sub(size.width) / 2) as i32,
        work.position.y + 72,
    );
    let mut runtime = placements
        .overlay
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let position = runtime.place(&target, size, default);
    if startup {
        runtime.waiting_for_fit = true;
    }
    drop(runtime);
    if window.outer_position().ok() != Some(position) {
        set_overlay_position(app, window, position);
    }
    save_overlay_placement(app);
}

pub(crate) fn restore_overlay_position(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    place_overlay(app, window, true);
}

fn reconcile_overlay_placement(app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> bool {
    if overlay_drag_active(app) {
        return false;
    }
    let monitors = window.available_monitors().unwrap_or_default();
    if monitors.is_empty() {
        return false;
    }
    let primary = primary_monitor(window, &monitors);
    let changed = app
        .state::<LyricsWindowPlacements>()
        .overlay
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .displays_changed(&monitors, primary.as_ref());
    if changed {
        place_overlay(app, window, false);
    }
    changed
}

fn handle_overlay_move(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) {
    if reconcile_overlay_placement(app, window) {
        return;
    }
    let placements = app.state::<LyricsWindowPlacements>();
    let mut runtime = placements
        .overlay
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if runtime.waiting_for_fit || runtime.ignore_move(position, false) {
        if let Some(expected) = runtime.expected_position() {
            if window.outer_position().ok().is_some_and(|actual| {
                actual.x.abs_diff(expected.x) > 2 || actual.y.abs_diff(expected.y) > 2
            }) && !runtime.waiting_for_fit
            {
                drop(runtime);
                set_overlay_position(app, window, expected);
            }
        }
        return;
    }
    drop(runtime);
    // 桌面歌词的真实拖动在 start_overlay_drag 收尾时单独提交；其他 Moved 事件不写用户位置。
    if let (Some(monitor), Ok(size)) = (overlay_target_monitor(app, window), window.outer_size()) {
        if let Some(expected) = overlay_position_for_size(app, &monitor, size) {
            if window.outer_position().ok() != Some(expected) {
                set_overlay_position(app, window, expected);
            }
        }
    }
}

pub(crate) fn cancel_overlay_pending_fit(app: &tauri::AppHandle) {
    if let Some(placements) = app.try_state::<LyricsWindowPlacements>() {
        placements
            .overlay
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .waiting_for_fit = false;
    }
}

pub(crate) fn overlay_target_monitor(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Option<tauri::Monitor> {
    let monitors = window.available_monitors().ok()?;
    let primary = primary_monitor(window, &monitors);
    let placements = app.state::<LyricsWindowPlacements>();
    let runtime = placements
        .overlay
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    runtime.target(&monitors, primary.as_ref()).cloned()
}

pub(crate) fn overlay_position_for_size(
    app: &tauri::AppHandle,
    monitor: &tauri::Monitor,
    size: tauri::PhysicalSize<u32>,
) -> Option<tauri::PhysicalPosition<i32>> {
    app.state::<LyricsWindowPlacements>()
        .overlay
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .record
        .as_ref()
        .map(|record| record.position_on(monitor, size))
}

pub(crate) fn complete_overlay_fit(
    app: &tauri::AppHandle,
    monitor: &tauri::Monitor,
    size: tauri::PhysicalSize<u32>,
    position: tauri::PhysicalPosition<i32>,
) {
    let placements = app.state::<LyricsWindowPlacements>();
    let mut runtime = placements
        .overlay
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let manual = runtime
        .record
        .as_ref()
        .is_some_and(|record| record.manual_monitor && record.monitor_id == monitor_id(monitor));
    if let Some(record) = runtime.record.clone() {
        runtime.record = Some(record.relocated(monitor, size, position, manual));
    }
    runtime.mark_programmatic(position);
    runtime.waiting_for_fit = false;
    drop(runtime);
    save_overlay_placement(app);
}

pub(crate) fn persist_overlay_state_at(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) {
    let (Ok(Some(monitor)), Ok(size)) = (window.current_monitor(), window.outer_size()) else {
        return;
    };
    let primary = window.primary_monitor().ok().flatten();
    let placements = app.state::<LyricsWindowPlacements>();
    let mut runtime = placements
        .overlay
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    runtime.capture_user(&monitor, primary.as_ref(), position, size);
    runtime.waiting_for_fit = false;
    drop(runtime);
    save_overlay_placement(app);
}

pub(crate) fn persist_overlay_style_frame(
    app: &tauri::AppHandle,
    monitor: &tauri::Monitor,
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
) {
    let placements = app.state::<LyricsWindowPlacements>();
    let mut runtime = placements
        .overlay
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let id = monitor_id(monitor);
    let manual = runtime
        .record
        .as_ref()
        .is_some_and(|record| record.manual_monitor && record.monitor_id == id);
    runtime.record = Some(WindowPlacement::capture(
        id,
        manual,
        position,
        size,
        WorkArea::of(monitor),
    ));
    runtime.mark_programmatic(position);
    runtime.waiting_for_fit = false;
    drop(runtime);
    save_overlay_placement(app);
}
