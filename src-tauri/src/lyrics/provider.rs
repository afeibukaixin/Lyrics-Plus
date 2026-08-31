use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::future::join_all;
use serde::{Deserialize, Serialize};
use strsim::normalized_levenshtein;
use zhhz::{Config, Converter};

use super::kugou::KugouProvider;
use super::kuwo::KuwoProvider;
use super::lrclib::LrcLibProvider;
use super::migu::MiguProvider;
use super::musixmatch::MusixmatchProvider;
use super::netease::NeteaseProvider;
use super::qqmusic::QqMusicProvider;
use super::{
    amll_ttml::AmllTtmlProvider,
    credentials::{MusixmatchTokenType, ProviderCredentialStore, ProviderCredentialView},
};

pub const LRCLIB_DISPLAY_NAME: &str = "LRCLIB";
pub const KUGOU_DISPLAY_NAME: &str = "Kugou";
pub const QQMUSIC_DISPLAY_NAME: &str = "QQMusic";
pub const NETEASE_DISPLAY_NAME: &str = "Netease";
pub const KUWO_DISPLAY_NAME: &str = "Kuwo";
pub const AMLL_DISPLAY_NAME: &str = "AMLL TTML";
pub const MIGU_DISPLAY_NAME: &str = "Migu";
pub const MUSIXMATCH_DISPLAY_NAME: &str = "Musixmatch";
pub const DEFAULT_AMLL_BASE_URL: &str = "https://api.amll.dev";
const MIN_LOCAL_TITLE_SIMILARITY: f64 = 0.6;
const LEGACY_AMLL_BASE_URLS: [&str; 3] = [
    "https://amlldb.bikonoo.com",
    "https://cdn.jsdelivr.net/gh/Steve-xmh/amll-ttml-db@main",
    "https://github.com/amll-dev/amll-ttml-db/raw/refs/heads/main",
];

include!("provider_types.rs");
include!("provider_settings.rs");
include!("provider_registry.rs");
include!("provider_matching.rs");

#[cfg(test)]
include!("provider_tests.rs");
