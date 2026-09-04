mod commands;
mod config;
mod language;
mod lyrics;
#[cfg(target_os = "macos")]
mod macos_status_item;
mod overlay_effect;
mod overlay_model;
mod overlay_placement;
mod overlay_pointer;
mod player;
mod player_lifecycle;
mod runtime_model;
mod state;
mod storage;
mod ui_update;
mod windows;

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use config::ConfigStore;
use language::UiLanguage;
pub(crate) use overlay_effect::sync_overlay_vibrancy;
pub(crate) use overlay_model::{
    OverlayBackground, OverlayBackgroundMode, OverlayOrientation, OverlayStyleSettings,
};
pub use overlay_placement::ToolbarPlacement;
#[cfg(test)]
use overlay_placement::{
    centered_position, toolbar_placement_after_move, MonitorTopologyEntry,
    PROGRAMMATIC_MOVE_SUPPRESSION,
};
pub(crate) use overlay_placement::{
    mark_overlay_programmatic_position, move_overlay_to_primary, primary_mouse_button_pressed,
    reset_overlay_toolbar_placement, set_overlay_drag_active, settle_overlay_position_at,
    show_main_window_at, show_main_window_centered, update_overlay_toolbar_placement_during_drag,
    OverlayPlacementState, StoredOverlayGeometry,
};
use overlay_placement::{
    monitor_topology, overlay_drag_active, overlay_geometry, set_overlay_position,
    set_overlay_toolbar_placement, should_show_main_window, StoredBounds,
    UNLOCK_HANDLE_HOVER_EVENT,
};
use overlay_pointer::position_unlock_handle;
pub(crate) use overlay_pointer::{
    activate_runtime, sync_list_unlock_handle, sync_unlock_handle, wake_overlay_pointer_monitor,
    LIST_UNLOCK_HANDLE_HOVER_EVENT,
};
#[cfg(test)]
use overlay_pointer::{
    point_in_window_bounds, should_hover_overlay, stable_overlay_hover, unlock_handle_position,
};
use player::{query_selected_player, PlayerSelection, SystemMediaService};
use runtime_model::{NotchLayoutMetrics, OverlaySettings};
pub(crate) use state::{AppState, NotchVisibilityState, SurfaceReopenRequest};
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{Emitter, Manager, WebviewWindowBuilder};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};
pub(crate) use ui_update::webview_url;

struct TrayMenuState {
    icon: TrayIcon<tauri::Wry>,
    #[cfg(target_os = "macos")]
    lyrics_icon: TrayIcon<tauri::Wry>,
    toggle_overlay: CheckMenuItem<tauri::Wry>,
    toggle_status_bar_lyrics: CheckMenuItem<tauri::Wry>,
    toggle_list_lyrics: CheckMenuItem<tauri::Wry>,
    toggle_notch_lyrics: CheckMenuItem<tauri::Wry>,
    switch_lyrics: MenuItem<tauri::Wry>,
    settings: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

pub(crate) const LEGAL_NOTICE_VERSION: u16 = 1;
pub(crate) const LEGAL_NOTICE_PREFERENCE: &str = "legal.notice.acceptedVersion";
mod app_runtime;
pub(crate) use app_runtime::{
    apply_dock_icon_hidden, apply_global_shortcuts, apply_menu_bar_icon_hidden,
    apply_native_language, legal_notice_accepted, register_global_shortcuts,
    sync_app_menu_bar_icon_visibility, sync_tray_lyrics_display_checked, sync_tray_overlay_checked,
};
#[cfg(test)]
pub(crate) use windows::initial_overlay_dimensions;
pub(crate) use windows::{
    apply_joining_other_apps_fullscreen, apply_list_lyrics_window_lock,
    apply_lyrics_window_space_behavior, apply_lyrics_windows_space_behavior,
    cancel_surface_destroy, configure_web_content_process_handler, create_overlay,
    handle_surface_destroyed, hide_surface, is_managed_surface_label, notch_monitor_id,
    notch_window_position, position_auxiliary_lyrics_window_default, prepare_surface_show,
    reconcile_auxiliary_lyrics_windows, refresh_overlay_mouse_tracking,
    reset_list_lyrics_window_size, schedule_surface_destroy, set_surface_runtime_state,
    set_window_frame, show_quick_lyrics_window, surface_is_destroying, sync_lyrics_surfaces,
    toggle_quick_lyrics_window, SurfaceRuntimeState,
};
include!("tray.rs");
include!("overlay_runtime.rs");
include!("bootstrap.rs");

#[cfg(test)]
include!("lib_tests.rs");
