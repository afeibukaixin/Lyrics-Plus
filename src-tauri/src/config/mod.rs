use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri_plugin_global_shortcut::Shortcut;

use crate::language::{detect_config_comment_language, ConfigComment, UiLanguage};
use crate::lyrics::provider::{normalize_settings, ProviderOrderMode, ProviderSettings};
use crate::overlay_model::{
    KaraokeStyle, LongTextMode, OverlayAlignment, OverlayBackground, OverlayBackgroundMode,
    OverlayLayout, OverlayOrientation, OverlayStyleSettings, SecondaryDisplayMode,
};
use crate::player::PlayerSelection;
use crate::storage::Storage;

pub const CONFIG_SCHEMA_VERSION: u16 = 50;
const APP_CONFIG_KEYS: &[&str] = &[
    "theme",
    "language",
    "playerSelection",
    "systemMediaFilterMode",
    "systemMediaApplications",
    "playerFollowerApplication",
    "hideDockIcon",
    "silentStartup",
    "autoCheckUpdates",
    "shortcuts",
];

include!("jsonc.rs");
include!("model.rs");
include!("migration.rs");
include!("validation.rs");
include!("store.rs");

#[cfg(test)]
include!("tests.rs");
