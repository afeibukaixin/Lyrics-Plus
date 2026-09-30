use serde::Deserialize;
use tauri::{Manager, PhysicalPosition, PhysicalSize};
use tauri_plugin_window_state::AppHandleExt;

use crate::window_placement::{
    primary_monitor, LyricsWindowPlacements, WindowPlacement, WorkArea, LIST_POSITION_KEY,
};
use crate::{monitor_id, AppState};

const LAST_MONITOR_KEY: &str = "lyrics-list.last-monitor";
const MIGRATED_KEY: &str = "lyrics-list.position-migrated";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyListPosition {
    x: i32,
    y: i32,
    work_x: i32,
    work_y: i32,
    work_width: u32,
    work_height: u32,
    scale_factor: f64,
}

#[derive(Deserialize)]
struct LegacyWindowState {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

fn legacy_window_state(app: &tauri::AppHandle) -> Option<LegacyWindowState> {
    let path = app.path().app_config_dir().ok()?.join(app.filename());
    let raw = std::fs::read_to_string(path).ok()?;
    let saved: serde_json::Value = serde_json::from_str(&raw).ok()?;
    serde_json::from_value(saved.get("lyrics-list")?.clone()).ok()
}

fn legacy_list_placement(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    monitors: &[tauri::Monitor],
    primary: Option<&tauri::Monitor>,
) -> Option<WindowPlacement> {
    let state = app.state::<AppState>();
    let old_id = state
        .storage
        .get_preference(LAST_MONITOR_KEY)
        .ok()
        .flatten();
    let saved = old_id.as_ref().and_then(|id| {
        state
            .storage
            .get_preference(&format!("lyrics-list.position.{id}"))
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str::<LegacyListPosition>(&raw).ok())
    });
    if let (Some(id), Some(saved)) = (old_id, saved) {
        let selected = monitors.iter().find(|monitor| monitor_id(monitor) == id);
        let size = window.outer_size().ok()?;
        return Some(WindowPlacement::capture(
            id.clone(),
            selected.is_some() && primary.is_some_and(|main| monitor_id(main) != id),
            PhysicalPosition::new(saved.x, saved.y),
            size,
            WorkArea {
                position: PhysicalPosition::new(saved.work_x, saved.work_y),
                size: PhysicalSize::new(saved.work_width, saved.work_height),
                scale_factor: saved.scale_factor,
            },
        ));
    }
    let migrated = state
        .storage
        .get_preference(MIGRATED_KEY)
        .ok()
        .flatten()
        .is_some();
    if migrated {
        return None;
    }
    let _ = state.storage.set_preference(MIGRATED_KEY, "1");
    let old = legacy_window_state(app)?;
    let selected = monitors
        .iter()
        .find(|monitor| {
            let origin = monitor.position();
            let size = monitor.size();
            (old.x as i64) < origin.x as i64 + size.width as i64
                && old.x as i64 + old.width as i64 > origin.x as i64
                && (old.y as i64) < origin.y as i64 + size.height as i64
                && old.y as i64 + old.height as i64 > origin.y as i64
        })
        .or(primary)
        .or_else(|| monitors.first())?;
    let id = monitor_id(selected);
    Some(WindowPlacement::capture(
        id.clone(),
        primary.is_some_and(|main| monitor_id(main) != id),
        PhysicalPosition::new(old.x, old.y),
        PhysicalSize::new(old.width, old.height),
        WorkArea::of(selected),
    ))
}

fn load_list_placement(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    monitors: &[tauri::Monitor],
    primary: Option<&tauri::Monitor>,
) -> Option<WindowPlacement> {
    app.state::<AppState>()
        .storage
        .get_preference(LIST_POSITION_KEY)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .or_else(|| legacy_list_placement(app, window, monitors, primary))
}

fn save_list_placement(app: &tauri::AppHandle) {
    let placements = app.state::<LyricsWindowPlacements>();
    let record = placements
        .list
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .record
        .clone();
    let Some(record) = record else {
        return;
    };
    let Ok(raw) = serde_json::to_string(&record) else {
        return;
    };
    if let Err(error) = app
        .state::<AppState>()
        .storage
        .set_preference(LIST_POSITION_KEY, &raw)
    {
        log::warn!("保存歌词列表位置失败：{error}");
    }
}

fn fit_window_to_monitor(window: &tauri::WebviewWindow, monitor: &tauri::Monitor) {
    let Ok(size) = window.outer_size() else {
        return;
    };
    let work = monitor.work_area();
    let fitted = PhysicalSize::new(
        size.width.min(work.size.width),
        size.height.min(work.size.height),
    );
    if fitted != size {
        let _ = window.set_size(fitted);
    }
}

fn set_list_position(window: &tauri::WebviewWindow, position: PhysicalPosition<i32>) {
    if window.outer_position().ok() == Some(position) {
        return;
    }
    if let Err(error) = window.set_position(position) {
        log::warn!("移动歌词列表失败：{error}");
    }
}

pub(crate) fn restore_list_lyrics_position(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let monitors = window.available_monitors().unwrap_or_default();
    let primary = primary_monitor(window, &monitors);
    let Some(main) = primary.as_ref() else {
        return;
    };
    let placements = app.state::<LyricsWindowPlacements>();
    if placements
        .list
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .record
        .is_none()
    {
        let saved = load_list_placement(app, window, &monitors, Some(main));
        placements
            .list
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .initialize(saved);
    }
    let mut runtime = placements
        .list
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
    fit_window_to_monitor(window, &target);
    let Ok(size) = window.outer_size() else {
        return;
    };
    let work = target.work_area();
    let default = PhysicalPosition::new(
        work.position.x + (work.size.width.saturating_sub(size.width) / 2) as i32,
        work.position.y + (work.size.height.saturating_sub(size.height) / 2) as i32,
    );
    let mut runtime = placements
        .list
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let position = runtime.place(&target, size, default);
    drop(runtime);
    set_list_position(window, position);
    save_list_placement(app);
}

pub(crate) fn reconcile_list_lyrics_placement(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> bool {
    let monitors = window.available_monitors().unwrap_or_default();
    if monitors.is_empty() {
        return false;
    }
    let primary = primary_monitor(window, &monitors);
    let changed = app
        .state::<LyricsWindowPlacements>()
        .list
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .displays_changed(&monitors, primary.as_ref());
    if changed {
        restore_list_lyrics_position(app, window);
    }
    changed
}

pub(crate) fn list_lyrics_window_moved(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: PhysicalPosition<i32>,
) {
    if !window.is_visible().unwrap_or(false) || reconcile_list_lyrics_placement(app, window) {
        return;
    }
    let user_dragging =
        crate::primary_mouse_button_pressed() && window.is_focused().unwrap_or(false);
    let placements = app.state::<LyricsWindowPlacements>();
    let mut runtime = placements
        .list
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if runtime.ignore_move(position, user_dragging) {
        let expected = runtime.expected_position();
        drop(runtime);
        if let Some(expected) = expected {
            if window.outer_position().ok().is_some_and(|actual| {
                actual.x.abs_diff(expected.x) > 2 || actual.y.abs_diff(expected.y) > 2
            }) {
                set_list_position(window, expected);
            }
        }
        return;
    }
    if cfg!(target_os = "macos") && !user_dragging {
        let monitors = window.available_monitors().unwrap_or_default();
        let primary = primary_monitor(window, &monitors);
        let expected = window.outer_size().ok().and_then(|size| {
            runtime
                .target(&monitors, primary.as_ref())
                .and_then(|monitor| {
                    runtime
                        .record
                        .as_ref()
                        .map(|record| record.position_on(monitor, size))
                })
        });
        drop(runtime);
        if expected.is_some_and(|target| target != position) {
            restore_list_lyrics_position(app, window);
        }
        return;
    }
    let (Ok(actual), Ok(Some(monitor)), Ok(size)) = (
        window.outer_position(),
        window.current_monitor(),
        window.outer_size(),
    ) else {
        return;
    };
    if actual.x.abs_diff(position.x) > 2 || actual.y.abs_diff(position.y) > 2 {
        return;
    }
    let primary = window.primary_monitor().ok().flatten();
    runtime.capture_user(&monitor, primary.as_ref(), actual, size);
    drop(runtime);
    save_list_placement(app);
}

pub(crate) fn list_lyrics_window_resized(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    if !window.is_visible().unwrap_or(false) || reconcile_list_lyrics_placement(app, window) {
        return;
    }
    let user_resizing =
        crate::primary_mouse_button_pressed() && window.is_focused().unwrap_or(false);
    if (!user_resizing && cfg!(target_os = "macos"))
        || app
            .state::<LyricsWindowPlacements>()
            .list
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .suppress_automatic_event(user_resizing)
    {
        return;
    }
    if let (Ok(position), Ok(Some(monitor)), Ok(size)) = (
        window.outer_position(),
        window.current_monitor(),
        window.outer_size(),
    ) {
        let primary = window.primary_monitor().ok().flatten();
        app.state::<LyricsWindowPlacements>()
            .list
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .capture_user(&monitor, primary.as_ref(), position, size);
        save_list_placement(app);
    }
}

pub(crate) fn reset_list_lyrics_position(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.storage.remove_preference(LIST_POSITION_KEY)?;
    state
        .storage
        .remove_preferences_with_prefix("lyrics-list.position.")?;
    state.storage.remove_preference(LAST_MONITOR_KEY)?;
    state.storage.set_preference(MIGRATED_KEY, "1")?;
    app.state::<LyricsWindowPlacements>()
        .list
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .reset();
    if let Some(window) = app.get_webview_window("lyrics-list") {
        restore_list_lyrics_position(app, &window);
    }
    Ok(())
}
