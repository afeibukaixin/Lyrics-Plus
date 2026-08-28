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
pub(crate) use state::AppState;
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
const LIST_LYRICS_DEFAULT_WIDTH: f64 = 520.0;
const LIST_LYRICS_DEFAULT_HEIGHT: f64 = 720.0;
const NOTCH_VISIBILITY_TRANSITION_EVENT: &str = "notch://visibility-transition";
const NOTCH_EXIT_ANIMATION_DURATION: Duration = Duration::from_millis(400);
const SURFACE_RUNTIME_STATE_EVENT: &str = "surface://runtime-state";

#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum SurfaceRuntimeState {
    Active,
    Dormant,
}

#[derive(Default)]
pub(crate) struct NotchVisibilityState {
    target_visible: bool,
    generation: u64,
}

#[derive(Clone, serde::Serialize)]
struct NotchVisibilityTransitionPayload {
    visible: bool,
}

include!("app_runtime.rs");
include!("windows.rs");
include!("tray.rs");
include!("overlay_runtime.rs");
include!("bootstrap.rs");

#[cfg(test)]
include!("lib_tests.rs");
