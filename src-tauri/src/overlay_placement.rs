#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarPlacement {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl ToolbarPlacement {
    fn for_orientation(orientation: OverlayOrientation) -> Self {
        match orientation {
            OverlayOrientation::Horizontal => Self::Top,
            OverlayOrientation::Vertical => Self::Right,
        }
    }

    fn normalized(self, orientation: OverlayOrientation) -> Self {
        match (orientation, self) {
            (OverlayOrientation::Horizontal, Self::Top | Self::Bottom)
            | (OverlayOrientation::Vertical, Self::Left | Self::Right) => self,
            _ => Self::for_orientation(orientation),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredBounds {
    x: i32,
    y: i32,
    #[serde(default)]
    work_x: Option<i32>,
    #[serde(default)]
    work_y: Option<i32>,
    #[serde(default)]
    work_width: Option<u32>,
    #[serde(default)]
    work_height: Option<u32>,
    #[serde(default)]
    scale_factor: Option<f64>,
    #[serde(default)]
    relative_x: Option<f64>,
    #[serde(default)]
    relative_y: Option<f64>,
    #[serde(default)]
    toolbar_placement: Option<ToolbarPlacement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MonitorTopologyEntry {
    id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    work_x: i32,
    work_y: i32,
    work_width: u32,
    work_height: u32,
    scale_factor_bits: u64,
}

const PROGRAMMATIC_MOVE_SUPPRESSION: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(crate) struct OverlayPlacementState {
    preferred_monitor: Option<String>,
    topology: Vec<MonitorTopologyEntry>,
    pub(crate) toolbar_placement: ToolbarPlacement,
    drag_active: bool,
    expected_programmatic_position: Option<tauri::PhysicalPosition<i32>>,
    programmatic_move_started_at: Option<Instant>,
}

impl OverlayPlacementState {
    fn update_topology(&mut self, next: Vec<MonitorTopologyEntry>) -> bool {
        if self.topology.is_empty() {
            self.topology = next;
            return false;
        }
        if self.topology == next {
            return false;
        }
        self.topology = next;
        self.expected_programmatic_position = None;
        self.programmatic_move_started_at = None;
        true
    }

    fn consume_programmatic_move(&mut self, position: tauri::PhysicalPosition<i32>) -> bool {
        let expected = self.expected_programmatic_position.take();
        self.programmatic_move_started_at = None;
        let Some(expected) = expected else {
            return false;
        };
        expected.x.abs_diff(position.x) <= 2 && expected.y.abs_diff(position.y) <= 2
    }

    fn suppress_persistence(&mut self, now: Instant) -> bool {
        let active = self.programmatic_move_started_at.is_some_and(|started| {
            now.saturating_duration_since(started) <= PROGRAMMATIC_MOVE_SUPPRESSION
        });
        if !active {
            self.expected_programmatic_position = None;
            self.programmatic_move_started_at = None;
        }
        active
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct StoredOverlayGeometry {
    pub horizontal_max_width: Option<f64>,
    pub vertical_max_height: Option<f64>,
}

fn overlay_geometry(storage: &storage::Storage, monitor_id: Option<&str>) -> StoredOverlayGeometry {
    let geometry_key = monitor_id
        .map(|id| format!("overlay.geometry.{id}"))
        .unwrap_or_else(|| "overlay.geometry.default".into());
    if let Ok(Some(raw)) = storage.get_preference(&geometry_key) {
        if let Ok(geometry) = serde_json::from_str(&raw) {
            return geometry;
        }
    }
    let legacy_key = monitor_id
        .map(|id| format!("overlay.style.{id}"))
        .unwrap_or_else(|| "overlay.style.default".into());
    storage
        .get_preference(&legacy_key)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<OverlayStyleSettings>(&raw).ok())
        .map(|style| StoredOverlayGeometry {
            horizontal_max_width: style.horizontal_max_width,
            vertical_max_height: style.vertical_max_height,
        })
        .unwrap_or_default()
}

fn monitor_topology(monitors: &[tauri::Monitor]) -> Vec<MonitorTopologyEntry> {
    let mut topology = monitors
        .iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            let work_area = monitor.work_area();
            MonitorTopologyEntry {
                id: monitor_id(monitor),
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
                work_x: work_area.position.x,
                work_y: work_area.position.y,
                work_width: work_area.size.width,
                work_height: work_area.size.height,
                scale_factor_bits: monitor.scale_factor().to_bits(),
            }
        })
        .collect::<Vec<_>>();
    topology.sort_by(|left, right| {
        (&left.id, left.x, left.y, left.width, left.height).cmp(&(
            &right.id,
            right.x,
            right.y,
            right.width,
            right.height,
        ))
    });
    topology
}

fn centered_position(
    work_position: tauri::PhysicalPosition<i32>,
    work_size: tauri::PhysicalSize<u32>,
    window_size: tauri::PhysicalSize<u32>,
) -> tauri::PhysicalPosition<i32> {
    tauri::PhysicalPosition::new(
        work_position.x + work_size.width.saturating_sub(window_size.width) as i32 / 2,
        work_position.y + work_size.height.saturating_sub(window_size.height) as i32 / 2,
    )
}

fn monitor_contains_point(monitor: &tauri::Monitor, point: tauri::PhysicalPosition<f64>) -> bool {
    let position = monitor.position();
    let size = monitor.size();
    point.x >= position.x as f64
        && point.x < position.x as f64 + size.width as f64
        && point.y >= position.y as f64
        && point.y < position.y as f64 + size.height as f64
}

fn center_main_window_on_cursor(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    let monitors = window
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let cursor = app.cursor_position().ok();
    let monitor = cursor
        .and_then(|point| {
            monitors
                .iter()
                .find(|monitor| monitor_contains_point(monitor, point))
        })
        .cloned()
        .or_else(|| window.primary_monitor().ok().flatten())
        .or_else(|| monitors.first().cloned())
        .ok_or_else(|| "没有可用的显示器".to_string())?;
    let work_area = monitor.work_area();
    let window_size = window.outer_size().map_err(|error| error.to_string())?;
    window
        .set_position(centered_position(
            work_area.position,
            work_area.size,
            window_size,
        ))
        .map_err(|error| error.to_string())
}

pub(crate) fn show_main_window_centered(app: &tauri::AppHandle) -> Result<(), String> {
    show_main_window_at(app, Some("#/settings"))
}

fn should_show_main_window(notice_accepted: bool, silent_startup: bool) -> bool {
    !notice_accepted || !silent_startup
}

pub(crate) fn show_main_window_at(
    app: &tauri::AppHandle,
    route: Option<&str>,
) -> Result<(), String> {
    let runtime_started = app.try_state::<AppState>().is_some_and(|state| {
        *state
            .runtime_started
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    });
    if runtime_started {
        setup_tray(app).map_err(|error| error.to_string())?;
        sync_app_menu_bar_icon_visibility(app)?;
    }

    let existing = app.get_webview_window("main");
    let window = if let Some(window) = existing.as_ref() {
        window.clone()
    } else {
        let path = route
            .map(|route| format!("index.html{route}"))
            .unwrap_or_else(|| "index.html".to_string());
        WebviewWindowBuilder::new(app, "main", WebviewUrl::App(path.into()))
            .title("Lyrics Plus")
            .inner_size(980.0, 720.0)
            .min_inner_size(760.0, 560.0)
            .resizable(false)
            .maximizable(false)
            .visible(false)
            .build()
            .map_err(|error| error.to_string())?
    };
    if existing.is_some() {
        if let Some(route) = route {
            window
                .eval(format!("window.location.hash = {route:?}"))
                .map_err(|error| error.to_string())?;
        }
    }
    if !window.is_visible().unwrap_or(false) {
        if let Err(error) = center_main_window_on_cursor(app, &window) {
            log::warn!("Failed to center the main window; using its current position: {error}");
        }
    }
    if let Err(error) = window.unminimize() {
        log::warn!("Failed to unminimize the main window: {error}");
    }
    window.show().map_err(|error| error.to_string())?;
    set_surface_runtime_state(app, &window, SurfaceRuntimeState::Active);
    window.set_focus().map_err(|error| error.to_string())
}

pub(crate) fn mark_overlay_programmatic_position(
    app: &tauri::AppHandle,
    position: tauri::PhysicalPosition<i32>,
) {
    if let Some(state) = app.try_state::<AppState>() {
        let mut placement = state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        placement.expected_programmatic_position = Some(position);
        placement.programmatic_move_started_at = Some(Instant::now());
    }
}

fn set_overlay_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) {
    mark_overlay_programmatic_position(app, position);
    let _ = window.set_position(position);
}

pub(crate) fn move_overlay_to_primary(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let work_area = monitor.work_area();
        let window_width = window.outer_size().map(|size| size.width).unwrap_or(760);
        let x =
            work_area.position.x + (work_area.size.width.saturating_sub(window_width) / 2) as i32;
        let y = work_area.position.y + 72;
        set_overlay_position(app, window, tauri::PhysicalPosition::new(x, y));
    }
}

const UNLOCK_HANDLE_BACKGROUND_GAP: f64 = 6.0;
const OVERLAY_EDGE_SNAP_DISTANCE: i32 = 12;
const OVERLAY_POINTER_MONITOR_INTERVAL: Duration = Duration::from_millis(50);
const UNLOCK_HANDLE_HIDE_DELAY: Duration = Duration::from_millis(200);
const UNLOCK_HANDLE_HOVER_EVENT: &str = "unlock-handle://hover";
const OVERLAY_HOVER_EVENT: &str = "overlay://hover";
const NOTCH_POINTER_SAMPLE_EVENT: &str = "notch://pointer-sample";
const OVERLAY_TOOLBAR_PLACEMENT_EVENT: &str = "overlay://toolbar-placement";

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NotchPointerSamplePayload {
    client_x: f64,
    client_y: f64,
}

fn toolbar_placement_after_move(
    orientation: OverlayOrientation,
    placement: ToolbarPlacement,
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    scale: f64,
    work_position: tauri::PhysicalPosition<i32>,
    work_size: tauri::PhysicalSize<u32>,
) -> (ToolbarPlacement, tauri::PhysicalPosition<i32>) {
    let placement = placement.normalized(orientation);
    let inset = (match orientation {
        OverlayOrientation::Horizontal => HORIZONTAL_OVERLAY_SURFACE_INSET,
        OverlayOrientation::Vertical => VERTICAL_OVERLAY_SURFACE_INSET,
    } * scale)
        .round() as i32;
    match (orientation, placement) {
        (OverlayOrientation::Horizontal, ToolbarPlacement::Top)
            if position.y <= work_position.y.saturating_add(OVERLAY_EDGE_SNAP_DISTANCE) =>
        {
            (
                ToolbarPlacement::Bottom,
                tauri::PhysicalPosition::new(position.x, position.y.saturating_add(inset)),
            )
        }
        (OverlayOrientation::Horizontal, ToolbarPlacement::Bottom) => {
            let window_bottom = position.y as i64 + size.height as i64;
            let work_bottom = work_position.y as i64 + work_size.height as i64;
            if window_bottom >= work_bottom - OVERLAY_EDGE_SNAP_DISTANCE as i64 {
                (
                    ToolbarPlacement::Top,
                    tauri::PhysicalPosition::new(position.x, position.y.saturating_sub(inset)),
                )
            } else {
                (placement, position)
            }
        }
        (OverlayOrientation::Vertical, ToolbarPlacement::Right) => {
            let window_right = position.x as i64 + size.width as i64;
            let work_right = work_position.x as i64 + work_size.width as i64;
            if window_right >= work_right - OVERLAY_EDGE_SNAP_DISTANCE as i64 {
                (
                    ToolbarPlacement::Left,
                    tauri::PhysicalPosition::new(position.x.saturating_sub(inset), position.y),
                )
            } else {
                (placement, position)
            }
        }
        (OverlayOrientation::Vertical, ToolbarPlacement::Left) => {
            if position.x as i64 <= work_position.x as i64 + OVERLAY_EDGE_SNAP_DISTANCE as i64 {
                (
                    ToolbarPlacement::Right,
                    tauri::PhysicalPosition::new(position.x.saturating_add(inset), position.y),
                )
            } else {
                (placement, position)
            }
        }
        _ => (placement, position),
    }
}

fn set_overlay_toolbar_placement(app: &tauri::AppHandle, placement: ToolbarPlacement) {
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
            sync_overlay_vibrancy(&window, &style);
        }
    }
}

pub(crate) fn reset_overlay_toolbar_placement(
    app: &tauri::AppHandle,
    orientation: OverlayOrientation,
) {
    set_overlay_toolbar_placement(app, ToolbarPlacement::for_orientation(orientation));
}

fn adjust_overlay_toolbar_for_move(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) -> tauri::PhysicalPosition<i32> {
    let (Ok(Some(monitor)), Ok(size)) = (window.current_monitor(), window.outer_size()) else {
        return position;
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
    let (next_placement, next_position) = toolbar_placement_after_move(
        orientation,
        placement,
        position,
        size,
        scale,
        work_area.position,
        work_area.size,
    );
    set_overlay_toolbar_placement(app, next_placement);
    next_position
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
    let snapped = snapped_position(window, position);
    let adjusted = adjust_overlay_toolbar_for_move(app, window, snapped);
    if adjusted != position {
        set_overlay_position(app, window, adjusted);
    }
    persist_overlay_state_at(app, window, adjusted);
}

#[cfg(target_os = "macos")]
pub(crate) fn primary_mouse_button_pressed() -> bool {
    use objc2_app_kit::NSEvent;

    NSEvent::pressedMouseButtons() & 1 != 0
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn primary_mouse_button_pressed() -> bool {
    false
}
