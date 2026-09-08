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
pub use super::endpoints::amll_ttml::DEFAULT_BASE as DEFAULT_AMLL_BASE_URL;
use super::endpoints::amll_ttml::LEGACY_BASES as LEGACY_AMLL_BASE_URLS;
const MIN_LOCAL_TITLE_SIMILARITY: f64 = 0.6;

include!("provider_types.rs");
include!("provider_settings.rs");
#[path = "provider_registry/mod.rs"]
mod provider_registry;
use provider_registry::provider_definitions;
pub use provider_registry::ProviderRegistry;
include!("provider_matching.rs");

#[cfg(test)]
include!("provider_tests.rs");
