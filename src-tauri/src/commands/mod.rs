use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_opener::OpenerExt;

use crate::config::{
    normalize_player_follower_application, normalize_system_media_applications,
    validate_config_draft, AppConfig, ChineseConversion, ConfigDraftValidation, ConfigEditorData,
    GlobalShortcutSettings, LanguagePreference, ListLyricsPreferences, LyricsBaseAppearance,
    LyricsModeStyleInheritance, NotchLyricsPreferences, OverlayAppearance, RegisteredApplication,
    StatusBarLyricsPreferences, SystemMediaFilterMode, ThemePreference,
};
use crate::language::UiLanguage;
use crate::lyrics::credentials::{MusixmatchTokenType, ProviderCredentialView};
use crate::lyrics::provider::{
    LyricsSearchInput, ProviderSettings, ProviderSettingsView, ProviderStatus,
};
use crate::lyrics::LyricsDocument;
use crate::player::{
    control_playback as control_player, seek_playback as seek_player, PlaybackAction,
    PlaybackArtwork, PlaybackSnapshot, PlaybackSpectrumState, PlayerSelection,
};
use crate::storage::library::LibraryScanStatus;
use crate::storage::{SaveKind, SaveRequest, LOCAL_PROVIDER_ID};
use crate::ui_update::UiUpdateStateView;

mod application_discovery;
mod config_runtime;
mod overlay_geometry;
mod overlay_persistence;

#[cfg(all(test, target_os = "macos"))]
use application_discovery::application_icon_at_path;
#[cfg(test)]
use application_discovery::resolve_registered_application;
use application_discovery::{collect_application_icons, resolve_application_bundle_id};
use config_runtime::{
    apply_app_config, finish_display_config_update, update_dock_icon_hidden,
    update_global_shortcuts, update_menu_bar_icon_hidden,
};
#[cfg(test)]
use overlay_geometry::fit_overlay_bounds;
use overlay_geometry::{
    clear_manual_overlay_bounds, fit_overlay_content_bounds, fixed_axis_content_size,
    reset_overlay_dimensions, resize_overlay_edge_bounds, MIN_HORIZONTAL_WINDOW_WIDTH,
    MIN_VERTICAL_HOST_WIDTH,
};
use overlay_persistence::persist_overlay_style_for_current_monitor;

#[cfg(test)]
use crate::lyrics::provider::LyricsSearchResult;
#[cfg(test)]
use crate::overlay_model::SecondaryDisplayMode;
#[cfg(test)]
use crate::overlay_model::{
    KaraokeStyle, OverlayAlignment, OverlayBackground, OverlayBackgroundMode, OverlayLayout,
};
pub use crate::overlay_model::{OverlayOrientation, OverlayStyleSettings};
pub use crate::state::AppState;

include!("models.rs");
include!("lyrics_runtime.rs");
include!("settings_types.rs");
include!("playback.rs");
include!("lyrics.rs");
include!("overlay.rs");
include!("application.rs");
include!("settings.rs");

#[cfg(test)]
include!("tests.rs");
