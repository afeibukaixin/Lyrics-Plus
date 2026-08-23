use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_opener::OpenerExt;

use crate::config::{
    normalize_player_follower_application, normalize_system_media_applications,
    validate_config_draft, AppConfig, ConfigDraftValidation, ConfigEditorData,
    GlobalShortcutSettings, LanguagePreference, ListLyricsPreferences, LyricsBaseAppearance,
    LyricsModeStyleInheritance, NotchLyricsPreferences, OverlayAppearance, RegisteredApplication,
    StatusBarLyricsPreferences, SystemMediaFilterMode, ThemePreference,
};
use crate::language::UiLanguage;
use crate::lyrics::credentials::{MusixmatchTokenType, ProviderCredentialView};
use crate::lyrics::provider::{
    can_auto_apply, LyricsSearchInput, LyricsSearchResult, ProviderSettings,
    ProviderSettingsView, ProviderStatus,
};
use crate::lyrics::LyricsDocument;
use crate::player::{
    control_playback as control_player, run_with_timeout, seek_playback as seek_player,
    PlaybackAction, PlaybackArtwork, PlaybackSnapshot, PlaybackSpectrumState, PlayerKind,
    PlayerSelection,
};
use crate::storage::library::LibraryScanStatus;
use crate::storage::{SaveKind, SaveRequest, LOCAL_PROVIDER_ID};

pub use crate::overlay_model::{
    OverlayOrientation, OverlayStyleSettings, SecondaryDisplayMode,
};
#[cfg(test)]
use crate::overlay_model::{
    KaraokeStyle, OverlayAlignment, OverlayBackground, OverlayBackgroundMode, OverlayLayout,
};
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
