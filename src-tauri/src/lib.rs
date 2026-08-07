mod artwork;
mod commands;
mod config;
mod lyrics;
mod player;
mod storage;

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use commands::{AppState, OverlayOrientation, OverlaySettings, OverlayStyleSettings};
use config::ConfigStore;
use player::{query_selected_player, PlayerSelection};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

#[cfg(target_os = "macos")]
pub(crate) fn apply_dock_icon_hidden(app: &tauri::AppHandle, hidden: bool) -> Result<(), String> {
    let main_window = app.get_webview_window("main");
    let main_was_visible = main_window
        .as_ref()
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    let main_was_focused = main_window
        .as_ref()
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false);

    app.set_dock_visibility(!hidden)
        .map_err(|error| format!("更新 Dock 显示状态失败：{error}"))?;

    if let Some(window) = main_window {
        if main_was_visible {
            if let Err(error) = window.show() {
                log::warn!("恢复主窗口显示状态失败：{error}");
            }
        }
        if main_was_focused {
            if let Err(error) = window.set_focus() {
                log::warn!("恢复主窗口焦点失败：{error}");
            }
        }
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apply_dock_icon_hidden(_app: &tauri::AppHandle, _hidden: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn cleanup_legacy_autostart(app: &tauri::AppHandle) {
    let Ok(home_dir) = app.path().home_dir() else {
        log::warn!("无法定位用户目录，未能清理旧开机启动项");
        return;
    };
    let launch_agent = home_dir
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", app.package_info().name));
    if launch_agent.exists() {
        if let Err(error) = std::fs::remove_file(&launch_agent) {
            log::warn!("无法清理旧开机启动项 {}：{error}", launch_agent.display());
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn cleanup_legacy_autostart(_app: &tauri::AppHandle) {}

pub(crate) fn create_overlay(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("lyrics-overlay").is_some() {
        return Ok(());
    }

    let style = app
        .try_state::<AppState>()
        .map(|state| {
            state
                .overlay_style
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
        })
        .unwrap_or_default();
    let (initial_width, initial_height) = initial_overlay_dimensions(&style);

    WebviewWindowBuilder::new(
        app,
        "lyrics-overlay",
        WebviewUrl::App("index.html?view=overlay".into()),
    )
    .title("Lyrics Plus Overlay")
    .inner_size(initial_width, initial_height)
    .min_inner_size(190.0, 76.0)
    .transparent(true)
    .accept_first_mouse(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .visible_on_all_workspaces(true)
    .skip_taskbar(true)
    .visible(false)
    .build()?;

    Ok(())
}

fn initial_overlay_dimensions(style: &OverlayStyleSettings) -> (f64, f64) {
    match style.orientation {
        OverlayOrientation::Horizontal => (style.horizontal_max_width.unwrap_or(760.0), 156.0),
        OverlayOrientation::Vertical => (190.0, style.vertical_max_height.unwrap_or(620.0)),
    }
}

fn create_unlock_handle(app: &tauri::App) -> tauri::Result<()> {
    if app.get_webview_window("lyrics-unlock-handle").is_some() {
        return Ok(());
    }
    WebviewWindowBuilder::new(
        app,
        "lyrics-unlock-handle",
        WebviewUrl::App("index.html?view=unlock-handle".into()),
    )
    .title("解锁桌面歌词")
    .inner_size(28.0, 28.0)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    // 这是覆盖在歌词浮窗上的点击入口，不应成为 macOS 的键盘焦点窗口。
    // 否则歌词换行触发位置同步时，可能抢走当前应用的焦点。
    .focusable(false)
    .always_on_top(true)
    .visible_on_all_workspaces(true)
    .skip_taskbar(true)
    .visible(false)
    .build()?;
    Ok(())
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show_main = MenuItem::with_id(app, "show-main", "打开 Lyrics Plus", true, None::<&str>)?;
    let toggle_overlay =
        MenuItem::with_id(app, "toggle-overlay", "显示/隐藏歌词", true, None::<&str>)?;
    let reset_overlay =
        MenuItem::with_id(app, "reset-overlay", "复位桌面歌词", true, None::<&str>)?;
    let toggle_lock =
        MenuItem::with_id(app, "toggle-lock", "锁定/解锁桌面歌词", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show_main,
            &toggle_overlay,
            &reset_overlay,
            &toggle_lock,
            &quit,
        ],
    )?;

    #[cfg(target_os = "macos")]
    let tray_icon = {
        let rgba = image::load_from_memory(include_bytes!("../icons/tray-icon.png"))
            .expect("invalid embedded tray icon")
            .into_rgba8();
        let (width, height) = rgba.dimensions();
        tauri::image::Image::new_owned(rgba.into_raw(), width, height)
    };
    #[cfg(not(target_os = "macos"))]
    let tray_icon = app
        .default_window_icon()
        .cloned()
        .expect("missing application icon");

    let tray_builder = TrayIconBuilder::new().icon(tray_icon);
    #[cfg(target_os = "macos")]
    let tray_builder = tray_builder.icon_as_template(true);

    tray_builder
        .tooltip("Lyrics Plus")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show-main" => {
                let _ = show_main_window_centered(app);
            }
            "toggle-overlay" => {
                let visible = app
                    .state::<AppState>()
                    .overlay_settings
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .visible;
                let _ = commands::update_overlay_visible(app, !visible);
            }
            "reset-overlay" => {
                let _ = commands::reset_overlay_bounds(app.clone());
            }
            "toggle-lock" => {
                let locked = app
                    .state::<AppState>()
                    .overlay_settings
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .locked;
                let _ = commands::update_overlay_locked(app, !locked);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn start_player_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let selection = app
                .try_state::<AppState>()
                .map(|state| *state.selection.read().unwrap_or_else(|e| e.into_inner()))
                .unwrap_or(PlayerSelection::Auto);
            let previous_auto_player = app
                .try_state::<AppState>()
                .map(|state| {
                    *state
                        .auto_player
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                })
                .unwrap_or(None);

            let (snapshot, next_auto_player) = tauri::async_runtime::spawn_blocking(move || {
                query_selected_player(selection, previous_auto_player)
            })
            .await
            .unwrap_or_else(|error| {
                (
                    player::PlaybackSnapshot::unavailable(
                        selection.preferred_kind(),
                        format!("播放器读取任务失败：{error}"),
                    ),
                    previous_auto_player,
                )
            });

            if let Some(state) = app.try_state::<AppState>() {
                *state
                    .last_snapshot
                    .write()
                    .unwrap_or_else(|e| e.into_inner()) = snapshot.clone();
                *state.auto_player.write().unwrap_or_else(|e| e.into_inner()) = next_auto_player;
            }
            let _ = app.emit("playback://snapshot", &snapshot);
            if let Some(window) = app.get_webview_window("lyrics-overlay") {
                if window.is_visible().unwrap_or(false) {
                    reconcile_overlay_placement(&app, &window);
                }
            }
            let any_window_visible = ["main", "lyrics-overlay"].iter().any(|label| {
                app.get_webview_window(label)
                    .and_then(|window| window.is_visible().ok())
                    .unwrap_or(false)
            });
            tokio::time::sleep(Duration::from_millis(if any_window_visible {
                750
            } else {
                2_000
            }))
            .await;
        }
    });
}

pub(crate) fn monitor_id(monitor: &tauri::Monitor) -> String {
    monitor.name().cloned().unwrap_or_else(|| {
        let position = monitor.position();
        let size = monitor.size();
        format!(
            "{}x{}-{}x{}",
            position.x, position.y, size.width, size.height
        )
    })
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
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    if !window.is_visible().unwrap_or(false) {
        center_main_window_on_cursor(app, &window)?;
        window.show().map_err(|error| error.to_string())?;
    }
    window.set_focus().map_err(|error| error.to_string())
}

fn set_overlay_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
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

const UNLOCK_HANDLE_EDGE_INSET: f64 = 6.0;
const UNLOCK_HANDLE_MONITOR_INTERVAL: Duration = Duration::from_millis(50);
const UNLOCK_HANDLE_HIDE_DELAY: Duration = Duration::from_millis(200);
const UNLOCK_HANDLE_HOVER_EVENT: &str = "unlock-handle://hover";

fn point_in_window_bounds(
    point: tauri::PhysicalPosition<f64>,
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
) -> bool {
    let right = position.x as f64 + size.width as f64;
    let bottom = position.y as f64 + size.height as f64;
    point.x >= position.x as f64
        && point.x < right
        && point.y >= position.y as f64
        && point.y < bottom
}

fn unlock_handle_position(
    orientation: OverlayOrientation,
    overlay_position: tauri::PhysicalPosition<i32>,
    overlay_size: tauri::PhysicalSize<u32>,
    handle_size: tauri::PhysicalSize<u32>,
    edge_inset: u32,
) -> tauri::PhysicalPosition<i32> {
    let available_width = overlay_size.width.saturating_sub(handle_size.width);
    let available_height = overlay_size.height.saturating_sub(handle_size.height);
    match orientation {
        OverlayOrientation::Horizontal => tauri::PhysicalPosition::new(
            overlay_position
                .x
                .saturating_add((available_width / 2) as i32),
            overlay_position
                .y
                .saturating_add(edge_inset.min(available_height) as i32),
        ),
        OverlayOrientation::Vertical => tauri::PhysicalPosition::new(
            overlay_position.x.saturating_add(
                available_width.saturating_sub(edge_inset.min(available_width)) as i32,
            ),
            overlay_position
                .y
                .saturating_add((available_height / 2) as i32),
        ),
    }
}

fn position_unlock_handle(app: &tauri::AppHandle) {
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
    let scale = overlay.scale_factor().unwrap_or(1.0);
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let edge_inset = (UNLOCK_HANDLE_EDGE_INSET * scale).round() as u32;
    let _ = handle.set_position(unlock_handle_position(
        orientation,
        position,
        size,
        handle_size,
        edge_inset,
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
    let Some(handle) = app.get_webview_window("lyrics-unlock-handle") else {
        return;
    };
    let should_show = settings.visible && settings.locked;
    let is_visible = handle.is_visible().unwrap_or(false);
    if should_show {
        position_unlock_handle(app);
    } else if is_visible {
        let _ = handle.hide();
        let _ = handle.emit(UNLOCK_HANDLE_HOVER_EVENT, false);
    }
}

fn start_unlock_handle_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_inside_at: Option<Instant> = None;
        let mut last_hovered: Option<bool> = None;

        loop {
            tokio::time::sleep(UNLOCK_HANDLE_MONITOR_INTERVAL).await;

            let Some(state) = app.try_state::<AppState>() else {
                continue;
            };
            let settings = state
                .overlay_settings
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            let (Some(overlay), Some(handle)) = (
                app.get_webview_window("lyrics-overlay"),
                app.get_webview_window("lyrics-unlock-handle"),
            ) else {
                continue;
            };

            if !settings.visible || !settings.locked {
                last_inside_at = None;
                if handle.is_visible().unwrap_or(false) {
                    let _ = handle.hide();
                }
                if last_hovered != Some(false) {
                    let _ = handle.emit(UNLOCK_HANDLE_HOVER_EVENT, false);
                    last_hovered = Some(false);
                }
                continue;
            }

            let sample = (
                app.cursor_position(),
                overlay.outer_position(),
                overlay.outer_size(),
                handle.outer_position(),
                handle.outer_size(),
            );
            let (should_show, hovered) = match sample {
                (
                    Ok(cursor),
                    Ok(overlay_position),
                    Ok(overlay_size),
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
            if last_hovered != Some(hovered) {
                let _ = handle.emit(UNLOCK_HANDLE_HOVER_EVENT, hovered);
                last_hovered = Some(hovered);
            }
        }
    });
}

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

fn relative_axis(position: i32, start: i32, work_length: u32, window_length: u32) -> f64 {
    let available = work_length.saturating_sub(window_length);
    if available == 0 {
        0.0
    } else {
        ((position as i64 - start as i64) as f64 / available as f64).clamp(0.0, 1.0)
    }
}

fn clamp_axis(position: i32, start: i32, work_length: u32, window_length: u32) -> i32 {
    let maximum = start as i64 + work_length.saturating_sub(window_length) as i64;
    (position as i64).clamp(start as i64, maximum.max(start as i64)) as i32
}

fn restored_overlay_position(
    bounds: &StoredBounds,
    work_position: tauri::PhysicalPosition<i32>,
    work_size: tauri::PhysicalSize<u32>,
    window_size: tauri::PhysicalSize<u32>,
    scale_factor: f64,
) -> tauri::PhysicalPosition<i32> {
    let same_work_area = bounds.work_x == Some(work_position.x)
        && bounds.work_y == Some(work_position.y)
        && bounds.work_width == Some(work_size.width)
        && bounds.work_height == Some(work_size.height)
        && bounds
            .scale_factor
            .is_some_and(|saved| (saved - scale_factor).abs() < 0.001);

    let (x, y) = if same_work_area {
        (bounds.x, bounds.y)
    } else {
        let available_width = work_size.width.saturating_sub(window_size.width);
        let available_height = work_size.height.saturating_sub(window_size.height);
        (
            bounds.relative_x.map_or(bounds.x, |relative| {
                work_position.x + (relative.clamp(0.0, 1.0) * available_width as f64).round() as i32
            }),
            bounds.relative_y.map_or(bounds.y, |relative| {
                work_position.y
                    + (relative.clamp(0.0, 1.0) * available_height as f64).round() as i32
            }),
        )
    };

    tauri::PhysicalPosition::new(
        clamp_axis(x, work_position.x, work_size.width, window_size.width),
        clamp_axis(y, work_position.y, work_size.height, window_size.height),
    )
}

fn stored_bounds(
    position: tauri::PhysicalPosition<i32>,
    window_size: tauri::PhysicalSize<u32>,
    monitor: &tauri::Monitor,
) -> StoredBounds {
    let work_area = monitor.work_area();
    StoredBounds {
        x: position.x,
        y: position.y,
        work_x: Some(work_area.position.x),
        work_y: Some(work_area.position.y),
        work_width: Some(work_area.size.width),
        work_height: Some(work_area.size.height),
        scale_factor: Some(monitor.scale_factor()),
        relative_x: Some(relative_axis(
            position.x,
            work_area.position.x,
            work_area.size.width,
            window_size.width,
        )),
        relative_y: Some(relative_axis(
            position.y,
            work_area.position.y,
            work_area.size.height,
            window_size.height,
        )),
    }
}

fn persist_overlay_state_at(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let id = monitor_id(&monitor);
    let state = app.state::<AppState>();
    if !state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .visible
    {
        return;
    }
    let mut active_monitor = state
        .overlay_monitor
        .write()
        .unwrap_or_else(|error| error.into_inner());
    let monitor_changed = active_monitor.as_deref() != Some(&id);
    if monitor_changed {
        *active_monitor = Some(id.clone());
    }
    drop(active_monitor);

    if monitor_changed {
        let geometry = overlay_geometry(&state.storage, Some(&id));
        let mut style = state.config.snapshot().overlay.appearance.into_style();
        style.horizontal_max_width = geometry.horizontal_max_width;
        style.vertical_max_height = geometry.vertical_max_height;
        *state
            .overlay_style
            .write()
            .unwrap_or_else(|error| error.into_inner()) = style.clone();
        let _ = app.emit("overlay://style", style);
    }

    let Ok(window_size) = window.outer_size() else {
        return;
    };
    let bounds = stored_bounds(position, window_size, &monitor);
    if let Ok(raw) = serde_json::to_string(&bounds) {
        let _ = state.storage.set_preference("overlay.last_monitor", &id);
        let _ = state
            .storage
            .set_preference(&format!("overlay.position.{id}"), &raw);
        state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .preferred_monitor = Some(id);
    }
    position_unlock_handle(app);
}

fn persist_overlay_state(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    if let Ok(position) = window.outer_position() {
        persist_overlay_state_at(app, window, position);
    }
}

fn overlay_intersects_any_monitor(
    window: &tauri::WebviewWindow,
    monitors: &[tauri::Monitor],
) -> bool {
    let (Ok(position), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return false;
    };
    let right = position.x as i64 + size.width as i64;
    let bottom = position.y as i64 + size.height as i64;
    monitors.iter().any(|monitor| {
        let monitor_position = monitor.position();
        let monitor_size = monitor.size();
        let monitor_right = monitor_position.x as i64 + monitor_size.width as i64;
        let monitor_bottom = monitor_position.y as i64 + monitor_size.height as i64;
        right > monitor_position.x as i64
            && (position.x as i64) < monitor_right
            && bottom > monitor_position.y as i64
            && (position.y as i64) < monitor_bottom
    })
}

fn apply_stored_overlay_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
) -> bool {
    let state = app.state::<AppState>();
    let key = format!("overlay.position.{}", monitor_id(monitor));
    let Some(bounds) = state
        .storage
        .get_preference(&key)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<StoredBounds>(&raw).ok())
    else {
        return false;
    };
    let Ok(window_size) = window.outer_size() else {
        return false;
    };
    let work_area = monitor.work_area();
    let position = restored_overlay_position(
        &bounds,
        work_area.position,
        work_area.size,
        window_size,
        monitor.scale_factor(),
    );
    set_overlay_position(app, window, position);
    true
}

fn refresh_overlay_topology(app: &tauri::AppHandle, monitors: &[tauri::Monitor]) -> bool {
    let state = app.state::<AppState>();
    let next = monitor_topology(monitors);
    let mut placement = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    placement.update_topology(next)
}

fn restore_preferred_overlay_placement(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    monitors: &[tauri::Monitor],
) {
    let preferred_monitor = app
        .state::<AppState>()
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .preferred_monitor
        .clone();
    if let Some(preferred_monitor) = preferred_monitor {
        if let Some(monitor) = monitors
            .iter()
            .find(|monitor| monitor_id(monitor) == preferred_monitor)
        {
            if apply_stored_overlay_position(app, window, monitor) {
                return;
            }
        } else {
            move_overlay_to_primary(app, window);
            return;
        }
    }
    if !overlay_intersects_any_monitor(window, monitors) {
        move_overlay_to_primary(app, window);
    }
}

fn reconcile_overlay_placement(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let monitors = window.available_monitors().unwrap_or_default();
    if monitors.is_empty() || !refresh_overlay_topology(app, &monitors) {
        return;
    }
    restore_preferred_overlay_placement(app, window, &monitors);
}

fn ignore_overlay_move(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) -> bool {
    let monitors = window.available_monitors().unwrap_or_default();
    let topology_changed = !monitors.is_empty() && refresh_overlay_topology(app, &monitors);
    let state = app.state::<AppState>();
    let mut placement = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let programmatic = placement.consume_programmatic_move(position);
    drop(placement);

    if topology_changed {
        restore_preferred_overlay_placement(app, window, &monitors);
    }
    topology_changed || programmatic
}

fn suppress_overlay_persistence(app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> bool {
    let monitors = window.available_monitors().unwrap_or_default();
    let topology_changed = !monitors.is_empty() && refresh_overlay_topology(app, &monitors);
    if topology_changed {
        restore_preferred_overlay_placement(app, window, &monitors);
        return true;
    }
    app.state::<AppState>()
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .suppress_persistence(Instant::now())
}

pub(crate) fn restore_overlay_position(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let state = app.state::<AppState>();
    let monitors = window.available_monitors().unwrap_or_default();
    let last_monitor = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .preferred_monitor
        .clone();

    if let Some(monitor) = last_monitor
        .as_ref()
        .and_then(|id| monitors.iter().find(|monitor| monitor_id(monitor) == *id))
    {
        if apply_stored_overlay_position(app, window, monitor) {
            return;
        }
    }
    move_overlay_to_primary(app, window);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .max_file_size(1_000_000)
                .rotation_strategy(RotationStrategy::KeepSome(3))
                .target(Target::new(TargetKind::Webview))
                .build(),
        )
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .skip_initial_state("main")
                .skip_initial_state("lyrics-overlay")
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            cleanup_legacy_autostart(app.handle());
            let storage = storage::Storage::new(app.handle())?;
            let app_dir = app.path().app_data_dir()?;
            let artwork =
                artwork::ArtworkService::new(app.path().app_cache_dir()?.join("artworks"))?;
            let (config, migrated) = ConfigStore::load(&app_dir, &storage)
                .map_err(|error| std::io::Error::other(error))?;
            let config = Arc::new(config);
            let configured = config.snapshot();
            let provider_settings = configured.lyrics.providers.clone();
            let selection = configured.app.player_selection;
            let locked = configured.overlay.locked;
            let overlay_settings = OverlaySettings {
                visible: configured.overlay.visible,
                locked,
                passthrough: locked,
            };
            let last_overlay_monitor = storage
                .get_preference("overlay.last_monitor")
                .unwrap_or(None);
            let geometry = overlay_geometry(&storage, last_overlay_monitor.as_deref());
            if migrated {
                let geometry_key = last_overlay_monitor
                    .as_ref()
                    .map(|id| format!("overlay.geometry.{id}"))
                    .unwrap_or_else(|| "overlay.geometry.default".into());
                let raw = serde_json::to_string(&geometry)?;
                storage.set_preference(&geometry_key, &raw)?;
                for key in [
                    "player.selection",
                    "lyrics.providers",
                    "overlay.visible",
                    "overlay.locked",
                    "overlay.passthrough",
                ] {
                    storage.remove_preference(key)?;
                }
                storage.remove_preferences_with_prefix("overlay.style.")?;
            }
            let mut overlay_style = configured.overlay.appearance.clone().into_style();
            overlay_style.horizontal_max_width = geometry.horizontal_max_width;
            overlay_style.vertical_max_height = geometry.vertical_max_height;
            app.manage(AppState {
                selection: Arc::new(RwLock::new(selection)),
                auto_player: Arc::new(RwLock::new(None)),
                overlay_settings: Arc::new(RwLock::new(overlay_settings.clone())),
                overlay_style: Arc::new(RwLock::new(overlay_style)),
                overlay_monitor: Arc::new(RwLock::new(last_overlay_monitor.clone())),
                overlay_placement: Arc::new(Mutex::new(OverlayPlacementState {
                    preferred_monitor: last_overlay_monitor,
                    ..OverlayPlacementState::default()
                })),
                last_snapshot: Arc::new(RwLock::new(player::PlaybackSnapshot::empty())),
                storage: Arc::new(storage),
                config,
                providers: Arc::new(lyrics::provider::ProviderRegistry::new(provider_settings)),
                artwork: Arc::new(artwork),
                http: reqwest::Client::builder()
                    .user_agent("Lyrics Plus/0.1 (macOS)")
                    .timeout(Duration::from_secs(8))
                    .build()
                    .map_err(|error| error.to_string())?,
            });

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_size(tauri::LogicalSize::new(980.0, 720.0));
                let _ = window.set_resizable(false);
                let _ = center_main_window_on_cursor(app.handle(), &window);
            }

            create_overlay(app.handle())?;
            create_unlock_handle(app)?;
            if let Some(window) = app.get_webview_window("lyrics-overlay") {
                let _ = window.set_resizable(false);
                let _ = window.set_ignore_cursor_events(overlay_settings.locked);
                let _ = window.set_focusable(!overlay_settings.locked);
                restore_overlay_position(app.handle(), &window);
                if overlay_settings.visible {
                    let _ = window.show();
                }
            }
            sync_unlock_handle(app.handle());
            start_unlock_handle_monitor(app.handle().clone());
            setup_tray(app)?;
            if configured.app.hide_dock_icon {
                apply_dock_icon_hidden(app.handle(), true).map_err(std::io::Error::other)?;
            }

            let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyL);
            app.global_shortcut()
                .on_shortcut(shortcut, |app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        let visible = app
                            .state::<AppState>()
                            .overlay_settings
                            .read()
                            .unwrap_or_else(|error| error.into_inner())
                            .visible;
                        let _ = commands::update_overlay_visible(app, !visible);
                    }
                })?;
            let unlock_shortcut =
                Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyU);
            app.global_shortcut()
                .on_shortcut(unlock_shortcut, |app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        let _ = commands::update_overlay_locked(app, false);
                    }
                })?;
            let reset_shortcut =
                Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Digit0);
            app.global_shortcut()
                .on_shortcut(reset_shortcut, |app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        let _ = commands::reset_overlay_bounds(app.clone());
                    }
                })?;

            start_player_monitor(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            if window.label() == "lyrics-overlay" {
                if let tauri::WindowEvent::Moved(position) = event {
                    if let Some(overlay) = window.app_handle().get_webview_window("lyrics-overlay")
                    {
                        if ignore_overlay_move(window.app_handle(), &overlay, *position) {
                            return;
                        }
                        let snapped = snapped_position(&overlay, *position);
                        if snapped != *position {
                            set_overlay_position(window.app_handle(), &overlay, snapped);
                            persist_overlay_state_at(window.app_handle(), &overlay, snapped);
                            return;
                        }
                        persist_overlay_state_at(window.app_handle(), &overlay, *position);
                    }
                }
                if matches!(event, tauri::WindowEvent::Resized(_)) {
                    if let Some(overlay) = window.app_handle().get_webview_window("lyrics-overlay")
                    {
                        if !suppress_overlay_persistence(window.app_handle(), &overlay) {
                            persist_overlay_state(window.app_handle(), &overlay);
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_playback_snapshot,
            commands::get_track_artwork,
            commands::get_player_selection,
            commands::set_player_selection,
            commands::player_action,
            commands::search_lyrics,
            commands::get_provider_settings,
            commands::set_provider_settings,
            commands::test_provider,
            commands::get_cached_lyrics,
            commands::save_lyrics,
            commands::import_lyrics,
            commands::set_lyrics_offset,
            commands::remove_lyrics_association,
            commands::get_library_overview,
            commands::set_lyrics_directory,
            commands::rescan_lyrics_library,
            commands::preview_library_entry,
            commands::set_overlay_visible,
            commands::get_overlay_visible,
            commands::get_overlay_settings,
            commands::set_overlay_locked,
            commands::set_overlay_passthrough,
            commands::get_overlay_style,
            commands::set_overlay_style,
            commands::nudge_overlay,
            commands::reset_overlay_bounds,
            commands::resize_overlay_edge,
            commands::fit_overlay_content,
            commands::show_main_window,
            commands::get_app_config,
            commands::set_ui_font_scale,
            commands::set_dock_icon_hidden,
            commands::export_app_config,
            commands::import_app_config,
            commands::reveal_config_directory,
            commands::get_config_editor_data,
            commands::validate_app_config_draft,
            commands::save_app_config_draft,
            commands::reset_settings_section,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Lyrics Plus");
    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if matches!(event, tauri::RunEvent::Reopen { .. }) {
            let _ = show_main_window_centered(app);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_initial_size_restores_the_saved_fixed_axis() {
        let horizontal = OverlayStyleSettings {
            horizontal_max_width: Some(540.0),
            ..OverlayStyleSettings::default()
        };
        assert_eq!(initial_overlay_dimensions(&horizontal), (540.0, 156.0));

        let vertical = OverlayStyleSettings {
            orientation: OverlayOrientation::Vertical,
            vertical_max_height: Some(480.0),
            ..OverlayStyleSettings::default()
        };
        assert_eq!(initial_overlay_dimensions(&vertical), (190.0, 480.0));
    }

    #[test]
    fn overlay_initial_size_uses_orientation_defaults_without_saved_geometry() {
        assert_eq!(
            initial_overlay_dimensions(&OverlayStyleSettings::default()),
            (760.0, 156.0)
        );
        let vertical = OverlayStyleSettings {
            orientation: OverlayOrientation::Vertical,
            ..OverlayStyleSettings::default()
        };
        assert_eq!(initial_overlay_dimensions(&vertical), (190.0, 620.0));
    }

    #[test]
    fn edge_snap_only_applies_inside_threshold() {
        assert_eq!(snap_coordinate(8, 0, 100), 0);
        assert_eq!(snap_coordinate(91, 0, 100), 100);
        assert_eq!(snap_coordinate(50, 0, 100), 50);
    }

    #[test]
    fn old_position_records_remain_compatible() {
        let bounds: StoredBounds =
            serde_json::from_str(r#"{"x":12,"y":34,"width":760,"height":156}"#).unwrap();
        assert_eq!((bounds.x, bounds.y), (12, 34));
        assert_eq!(bounds.relative_x, None);
        assert_eq!(bounds.work_width, None);
    }

    #[test]
    fn old_position_records_are_clamped_to_the_current_work_area() {
        let bounds: StoredBounds = serde_json::from_str(r#"{"x":900,"y":700}"#).unwrap();
        assert_eq!(
            restored_overlay_position(
                &bounds,
                tauri::PhysicalPosition::new(0, 0),
                tauri::PhysicalSize::new(800, 600),
                tauri::PhysicalSize::new(200, 100),
                2.0,
            ),
            tauri::PhysicalPosition::new(600, 500),
        );
    }

    #[test]
    fn relative_position_adapts_to_resolution_and_monitor_origin_changes() {
        let bounds: StoredBounds = serde_json::from_str(
            r#"{"x":400,"y":200,"workX":0,"workY":0,"workWidth":1000,"workHeight":800,"scaleFactor":2.0,"relativeX":0.5,"relativeY":0.25}"#,
        )
        .unwrap();
        assert_eq!(
            restored_overlay_position(
                &bounds,
                tauri::PhysicalPosition::new(1920, 0),
                tauri::PhysicalSize::new(2000, 1200),
                tauri::PhysicalSize::new(200, 100),
                1.0,
            ),
            tauri::PhysicalPosition::new(2820, 275),
        );
    }

    #[test]
    fn unchanged_work_area_preserves_exact_saved_position() {
        let bounds: StoredBounds = serde_json::from_str(
            r#"{"x":321,"y":234,"workX":0,"workY":24,"workWidth":1440,"workHeight":876,"scaleFactor":2.0,"relativeX":0.1,"relativeY":0.9}"#,
        )
        .unwrap();
        assert_eq!(
            restored_overlay_position(
                &bounds,
                tauri::PhysicalPosition::new(0, 24),
                tauri::PhysicalSize::new(1440, 876),
                tauri::PhysicalSize::new(760, 156),
                2.0,
            ),
            tauri::PhysicalPosition::new(321, 234),
        );
    }

    #[test]
    fn main_window_is_centered_inside_negative_origin_work_area() {
        assert_eq!(
            centered_position(
                tauri::PhysicalPosition::new(-1920, 24),
                tauri::PhysicalSize::new(1920, 1056),
                tauri::PhysicalSize::new(980, 720),
            ),
            tauri::PhysicalPosition::new(-1450, 192),
        );
    }

    #[test]
    fn topology_changes_preserve_preferred_monitor_and_clear_programmatic_move() {
        let topology = |width| {
            vec![MonitorTopologyEntry {
                id: "external".into(),
                x: 0,
                y: 0,
                width,
                height: 1080,
                work_x: 0,
                work_y: 24,
                work_width: width,
                work_height: 1056,
                scale_factor_bits: 1.0_f64.to_bits(),
            }]
        };
        let mut placement = OverlayPlacementState {
            preferred_monitor: Some("external".into()),
            expected_programmatic_position: Some(tauri::PhysicalPosition::new(10, 20)),
            ..OverlayPlacementState::default()
        };
        assert!(!placement.update_topology(topology(1920)));
        assert!(placement.consume_programmatic_move(tauri::PhysicalPosition::new(10, 20)));
        placement.expected_programmatic_position = Some(tauri::PhysicalPosition::new(30, 40));
        placement.programmatic_move_started_at = Some(Instant::now());
        assert!(placement.update_topology(topology(2560)));
        assert_eq!(placement.preferred_monitor.as_deref(), Some("external"));
        assert_eq!(placement.expected_programmatic_position, None);
        assert_eq!(placement.programmatic_move_started_at, None);
    }

    #[test]
    fn programmatic_move_suppression_expires() {
        let now = Instant::now();
        let mut placement = OverlayPlacementState {
            expected_programmatic_position: Some(tauri::PhysicalPosition::new(10, 20)),
            programmatic_move_started_at: Some(
                now - PROGRAMMATIC_MOVE_SUPPRESSION - Duration::from_millis(1),
            ),
            ..OverlayPlacementState::default()
        };
        assert!(!placement.suppress_persistence(now));
        assert_eq!(placement.expected_programmatic_position, None);
    }

    #[test]
    fn horizontal_unlock_handle_is_centered_at_the_top() {
        let overlay_position = tauri::PhysicalPosition::new(100, 200);
        let overlay_size = tauri::PhysicalSize::new(760, 156);
        let handle_size = tauri::PhysicalSize::new(28, 28);
        assert_eq!(
            unlock_handle_position(
                OverlayOrientation::Horizontal,
                overlay_position,
                overlay_size,
                handle_size,
                6,
            ),
            tauri::PhysicalPosition::new(466, 206),
        );
    }

    #[test]
    fn vertical_unlock_handle_is_centered_at_the_right() {
        let overlay_position = tauri::PhysicalPosition::new(100, 200);
        let overlay_size = tauri::PhysicalSize::new(190, 620);
        let handle_size = tauri::PhysicalSize::new(28, 28);
        assert_eq!(
            unlock_handle_position(
                OverlayOrientation::Vertical,
                overlay_position,
                overlay_size,
                handle_size,
                12,
            ),
            tauri::PhysicalPosition::new(250, 496),
        );
    }

    #[test]
    fn point_in_window_bounds_uses_exclusive_right_and_bottom_edges() {
        let position = tauri::PhysicalPosition::new(100, 200);
        let size = tauri::PhysicalSize::new(28, 28);
        assert!(point_in_window_bounds(
            tauri::PhysicalPosition::new(100.0, 200.0),
            position,
            size,
        ));
        assert!(point_in_window_bounds(
            tauri::PhysicalPosition::new(127.9, 227.9),
            position,
            size,
        ));
        assert!(!point_in_window_bounds(
            tauri::PhysicalPosition::new(128.0, 228.0),
            position,
            size,
        ));
    }
}
