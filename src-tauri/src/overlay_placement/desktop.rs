use serde::{Deserialize, Serialize};
use tauri::{Manager, PhysicalPosition, PhysicalSize};

use crate::{monitor_id, AppState};

use super::state::ToolbarPlacement;

const DESKTOP_PLACEMENT_KEY: &str = "desktop-overlay-placement.v1";
const SNAP_DISTANCE: u32 = 12;

/// 偏移量使用逻辑像素，缩放比例变化时仍保持用户设置的边距。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum AxisPlacement {
    Free { offset: f64 },
    Start { gap: f64 },
    End { gap: f64 },
}

impl AxisPlacement {
    fn capture(value: i32, start: i32, length: u32, window_length: u32, scale: f64) -> (Self, i32) {
        let end = start as i64 + length.saturating_sub(window_length) as i64;
        let value = (value as i64).clamp(start as i64, end);
        let start_gap = value - start as i64;
        let end_gap = end - value;
        if start_gap <= SNAP_DISTANCE as i64 {
            (Self::Start { gap: 0.0 }, start)
        } else if end_gap <= SNAP_DISTANCE as i64 {
            (Self::End { gap: 0.0 }, end as i32)
        } else {
            (
                Self::Free {
                    offset: start_gap as f64 / scale,
                },
                value as i32,
            )
        }
    }

    fn project(&self, start: i32, length: u32, window_length: u32, scale: f64) -> i32 {
        let end = start as i64 + length.saturating_sub(window_length) as i64;
        let value = match self {
            Self::Free { offset } => start as i64 + (offset * scale).round() as i64,
            Self::Start { gap } => start as i64 + (gap * scale).round() as i64,
            Self::End { gap } => end - (gap * scale).round() as i64,
        };
        value
            .clamp(start as i64, end)
            .clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }

    fn valid(&self) -> bool {
        let value = match self {
            Self::Free { offset } => *offset,
            Self::Start { gap } | Self::End { gap } => *gap,
        };
        value.is_finite() && value >= 0.0
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopPlacement {
    version: u8,
    monitor_id: String,
    horizontal: AxisPlacement,
    vertical: AxisPlacement,
    width: f64,
    height: f64,
    pub(crate) toolbar_placement: ToolbarPlacement,
}

impl DesktopPlacement {
    fn valid(&self) -> bool {
        self.version == 1
            && !self.monitor_id.is_empty()
            && self.horizontal.valid()
            && self.vertical.valid()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }

    fn position(&self, monitor: &tauri::Monitor, size: PhysicalSize<u32>) -> PhysicalPosition<i32> {
        let work = monitor.work_area();
        let scale = safe_scale(monitor);
        PhysicalPosition::new(
            self.horizontal
                .project(work.position.x, work.size.width, size.width, scale),
            self.vertical
                .project(work.position.y, work.size.height, size.height, scale),
        )
    }

    fn size(&self, monitor: &tauri::Monitor) -> PhysicalSize<u32> {
        let scale = safe_scale(monitor);
        let work = monitor.work_area();
        PhysicalSize::new(
            ((self.width * scale).round() as u32).clamp(1, work.size.width.max(1)),
            ((self.height * scale).round() as u32).clamp(1, work.size.height.max(1)),
        )
    }
}

fn safe_scale(monitor: &tauri::Monitor) -> f64 {
    let scale = monitor.scale_factor();
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

pub(crate) fn load_desktop_placement(
    storage: &crate::storage::Storage,
) -> Option<DesktopPlacement> {
    storage
        .get_preference(DESKTOP_PLACEMENT_KEY)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<DesktopPlacement>(&raw).ok())
        .filter(DesktopPlacement::valid)
}

fn persist(app: &tauri::AppHandle, record: &DesktopPlacement) {
    let Ok(raw) = serde_json::to_string(record) else {
        return;
    };
    if let Err(error) = app
        .state::<AppState>()
        .storage
        .set_preference(DESKTOP_PLACEMENT_KEY, &raw)
    {
        log::warn!("保存桌面歌词位置失败：{error}");
    }
}

fn remember_monitor(app: &tauri::AppHandle, monitor: &tauri::Monitor) {
    let id = monitor_id(monitor);
    let state = app.state::<AppState>();
    *state
        .overlay_monitor
        .write()
        .unwrap_or_else(|error| error.into_inner()) = Some(id.clone());
    // 逐显示器的歌词尺寸设置仍使用这个键；它不再参与窗口定位。
    let _ = state.storage.set_preference("overlay.last_monitor", &id);
}

pub(crate) fn overlay_target_monitor(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Option<tauri::Monitor> {
    let monitors = window.available_monitors().ok()?;
    let saved_id = app
        .state::<AppState>()
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .record
        .as_ref()
        .map(|record| record.monitor_id.clone());
    saved_id
        .as_ref()
        .and_then(|id| {
            monitors
                .iter()
                .find(|monitor| monitor_id(monitor) == *id)
                .cloned()
        })
        .or_else(|| window.primary_monitor().ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| monitors.first().cloned())
}

pub(crate) fn overlay_position_for_size(
    app: &tauri::AppHandle,
    monitor: &tauri::Monitor,
    size: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    let record = app
        .state::<AppState>()
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .record
        .clone();
    if let Some(record) = record {
        return record.position(monitor, size);
    }
    let work = monitor.work_area();
    let maximum_y = work.position.y as i64 + work.size.height.saturating_sub(size.height) as i64;
    PhysicalPosition::new(
        work.position.x + (work.size.width.saturating_sub(size.width) / 2) as i32,
        ((work.position.y as f64 + 72.0 * safe_scale(monitor)).round() as i64).min(maximum_y)
            as i32,
    )
}

pub(crate) fn begin_overlay_window(app: &tauri::AppHandle) -> u64 {
    let state = app.state::<AppState>();
    let mut placement = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    placement.window_generation = placement.window_generation.wrapping_add(1);
    placement.latest_fit_sequence = 0;
    placement.layout_ready = false;
    placement.window_generation
}

pub(crate) fn overlay_layout_ready(app: &tauri::AppHandle) -> bool {
    app.state::<AppState>()
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .layout_ready
}

pub(crate) fn capture_overlay_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: PhysicalPosition<i32>,
) -> Option<PhysicalPosition<i32>> {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())?;
    capture_overlay_position_on_monitor(app, window, &monitor, position)
}

pub(crate) fn capture_overlay_position_on_monitor(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
    position: PhysicalPosition<i32>,
) -> Option<PhysicalPosition<i32>> {
    let size = window.outer_size().ok()?;
    let work = monitor.work_area();
    let scale = safe_scale(monitor);
    let (horizontal, x) = AxisPlacement::capture(
        position.x,
        work.position.x,
        work.size.width,
        size.width,
        scale,
    );
    let (vertical, y) = AxisPlacement::capture(
        position.y,
        work.position.y,
        work.size.height,
        size.height,
        scale,
    );
    let state = app.state::<AppState>();
    let mut runtime = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let record = DesktopPlacement {
        version: 1,
        monitor_id: monitor_id(monitor),
        horizontal,
        vertical,
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
        toolbar_placement: runtime.toolbar_placement,
    };
    runtime.record = Some(record.clone());
    drop(runtime);
    persist(app, &record);
    remember_monitor(app, monitor);
    crate::position_unlock_handle(app);
    Some(PhysicalPosition::new(x, y))
}

pub(crate) fn record_overlay_size(
    app: &tauri::AppHandle,
    monitor: &tauri::Monitor,
    size: PhysicalSize<u32>,
) {
    let state = app.state::<AppState>();
    let mut runtime = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let scale = safe_scale(monitor);
    let toolbar = runtime.toolbar_placement;
    let Some(record) = runtime.record.as_mut() else {
        return;
    };
    record.width = size.width as f64 / scale;
    record.height = size.height as f64 / scale;
    record.monitor_id = monitor_id(monitor);
    record.toolbar_placement = toolbar;
    let saved = record.clone();
    drop(runtime);
    persist(app, &saved);
    remember_monitor(app, monitor);
}

pub(crate) fn persist_toolbar_placement(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let mut runtime = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let toolbar = runtime.toolbar_placement;
    let Some(record) = runtime.record.as_mut() else {
        return;
    };
    record.toolbar_placement = toolbar;
    let saved = record.clone();
    drop(runtime);
    persist(app, &saved);
}

pub(crate) fn reset_desktop_placement(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.storage.remove_preference(DESKTOP_PLACEMENT_KEY)?;
    let mut runtime = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    runtime.record = None;
    runtime.latest_fit_sequence = 0;
    Ok(())
}

pub(crate) fn adopt_system_monitor(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let Some(monitor) = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
    else {
        return;
    };
    let connected = window.available_monitors().unwrap_or_default();
    let old_id = app
        .state::<AppState>()
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .record
        .as_ref()
        .map(|record| record.monitor_id.clone());
    if old_id
        .as_ref()
        .is_some_and(|id| connected.iter().any(|entry| monitor_id(entry) == *id))
    {
        return;
    }
    let Ok(position) = window.outer_position() else {
        return;
    };
    if let Some(visible_position) = capture_overlay_position(app, window, position) {
        if visible_position != position {
            crate::set_overlay_position(app, window, visible_position);
        }
    }
    remember_monitor(app, &monitor);
}

pub(crate) async fn fallback_overlay_layout(app: tauri::AppHandle, generation: u64) {
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let state = app.state::<AppState>();
    let fit_lock = state.overlay_fit_lock.clone();
    let _fit = fit_lock.lock().await;
    let Some(window) = app.get_webview_window("lyrics-overlay") else {
        return;
    };
    let record = {
        let runtime = state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if runtime.window_generation != generation || runtime.layout_ready {
            return;
        }
        runtime.record.clone()
    };
    let Some(monitor) = overlay_target_monitor(&app, &window) else {
        return;
    };
    let size = record
        .as_ref()
        .map(|saved| saved.size(&monitor))
        .unwrap_or_else(|| window.outer_size().unwrap_or(PhysicalSize::new(190, 156)));
    let position = overlay_position_for_size(&app, &monitor, size);
    let old_size = window.outer_size().unwrap_or(size);
    let old_position = window.outer_position().unwrap_or(position);
    if let Err(error) = crate::set_window_frame(
        &window,
        old_size,
        old_position,
        size,
        position,
        safe_scale(&monitor),
    ) {
        log::warn!("桌面歌词备用定位失败：{error}");
        return;
    }
    state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .layout_ready = true;
    if record.is_none() {
        let _ = capture_overlay_position_on_monitor(&app, &window, &monitor, position);
    } else {
        record_overlay_size(&app, &monitor, size);
    }
    log::warn!("桌面歌词首轮测量超时，使用保存尺寸显示");
    let _ = crate::reconcile_overlay_visibility(&app);
}
