use std::collections::{hash_map::DefaultHasher, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, Manager};

use crate::lyrics::provider::{
    score_candidate, title_matches, LyricsSearchInput, LyricsSearchResult, KUGOU_DISPLAY_NAME,
    NETEASE_DISPLAY_NAME, QQMUSIC_DISPLAY_NAME,
};
use crate::lyrics::{parse_lrc_with_options, LyricsDocument};

pub mod library;

pub const LOCAL_PROVIDER_ID: &str = "local";
pub const LOCAL_FILE_SOURCE: &str = "本地文件";
const MAX_LOCAL_SEARCH_RESULTS: usize = 8;
const MIN_LOCAL_SEARCH_SCORE: f64 = 0.5;

include!("models.rs");
include!("database.rs");
include!("lyrics.rs");
include!("associations.rs");
include!("preferences.rs");
include!("helpers.rs");

#[cfg(test)]
include!("tests.rs");
