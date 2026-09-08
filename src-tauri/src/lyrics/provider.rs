use std::collections::HashSet;
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use strsim::normalized_levenshtein;
use zhhz::Config;

use crate::lyrics::conversion::{convert_text, is_japanese};

#[cfg(test)]
use super::credentials::ProviderCredentialStore;
#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::{Mutex, RwLock};

pub const LRCLIB_DISPLAY_NAME: &str = "LRCLIB";
pub const KUGOU_DISPLAY_NAME: &str = "Kugou";
pub const QQMUSIC_DISPLAY_NAME: &str = "QQMusic";
pub const NETEASE_DISPLAY_NAME: &str = "Netease";
pub const KUWO_DISPLAY_NAME: &str = "Kuwo";
pub const AMLL_DISPLAY_NAME: &str = "AMLL TTML";
pub const MIGU_DISPLAY_NAME: &str = "Migu";
pub const MUSIXMATCH_DISPLAY_NAME: &str = "Musixmatch";
pub const QISHUI_DISPLAY_NAME: &str = "Qishui";
pub const DEFAULT_AMLL_BASE_URL: &str = "https://api.amll.dev";
/// 后台自动搜索最多检查的正文候选数；5 条只作为首批可展示结果目标。
pub(crate) const MAX_AUTOMATIC_FETCH_CANDIDATES: usize = 24;
/// 自动搜索在决策稳定后，至少尽量保留的可展示正文数量。
pub(crate) const MIN_AUTOMATIC_FETCH_RESULTS: usize = 5;
/// 交互搜索允许用户比较更多版本，但最终列表仍有统一上限。
pub(crate) const MAX_INTERACTIVE_FETCH_CANDIDATES: usize = 24;
const MIN_LOCAL_TITLE_SIMILARITY: f64 = 0.6;
const LEGACY_AMLL_BASE_URLS: [&str; 3] = [
    "https://amlldb.bikonoo.com",
    "https://cdn.jsdelivr.net/gh/Steve-xmh/amll-ttml-db@main",
    "https://github.com/amll-dev/amll-ttml-db/raw/refs/heads/main",
];

include!("provider_types.rs");
include!("provider_settings.rs");
#[path = "provider_registry/mod.rs"]
mod provider_registry;
use provider_registry::provider_definitions;
pub use provider_registry::ProviderRegistry;
include!("provider_matching.rs");

#[cfg(test)]
include!("provider_tests.rs");
