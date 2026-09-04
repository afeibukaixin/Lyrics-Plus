mod commands;
mod config;
mod language;
mod lyrics;
#[cfg(target_os = "macos")]
mod macos_status_item;
mod overlay_effect;
mod overlay_model;
mod player;
mod player_lifecycle;
mod runtime_model;
mod state;
mod storage;
mod windows;

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use config::{ConfigStore, GlobalShortcutSettings};
use language::UiLanguage;
pub(crate) use overlay_effect::sync_overlay_vibrancy;
use overlay_effect::{HORIZONTAL_OVERLAY_SURFACE_INSET, VERTICAL_OVERLAY_SURFACE_INSET};
pub(crate) use overlay_model::{
    OverlayBackground, OverlayBackgroundMode, OverlayOrientation, OverlayStyleSettings,
};
use player::{query_selected_player, PlayerSelection, SystemMediaService};
use runtime_model::{NotchLayoutMetrics, OverlaySettings};
pub(crate) use state::{AppState, NotchVisibilityState, SurfaceReopenRequest};
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

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
