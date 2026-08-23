use std::sync::Arc;

use crate::lyrics::provider::{LyricsSearchInput, LyricsSearchResult, ProviderStatus};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsMonitor {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLyricsInput {
    pub track_key: String,
    pub title: String,
    pub artist: String,
    pub source: String,
    pub lyrics: String,
    pub provider_id: Option<String>,
    pub provider_item_id: Option<String>,
    #[serde(default)]
    pub manual_selected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub auto_apply: bool,
    pub results: Vec<LyricsSearchResult>,
    pub provider_statuses: Vec<ProviderStatus>,
    pub error: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct LyricsSearchRequestKey {
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: Option<u64>,
}

impl LyricsSearchRequestKey {
    pub(crate) fn new(input: &LyricsSearchInput) -> Self {
        Self {
            title: input.title.trim().to_owned(),
            artist: input.artist.trim().to_owned(),
            album: input
                .album
                .as_deref()
                .map(str::trim)
                .filter(|album| !album.is_empty())
                .map(str::to_owned),
            duration_ms: input.duration_ms,
        }
    }
}

pub(crate) type LyricsSearchFlight = tokio::sync::OnceCell<Result<SearchResponse, String>>;
pub(crate) const LYRICS_SEARCH_INVALIDATED: &str = "当前歌词搜索已失效";

pub struct LyricsSearchSession {
    pub(crate) activation: u64,
    pub(crate) track_key: Option<String>,
    pub(crate) request_id: u64,
    pub(crate) request_key: Option<LyricsSearchRequestKey>,
    pub(crate) completed: Option<Result<SearchResponse, String>>,
    pub(crate) in_flight: Option<Arc<LyricsSearchFlight>>,
}

impl Default for LyricsSearchSession {
    fn default() -> Self {
        Self {
            activation: 0,
            track_key: None,
            request_id: 0,
            request_key: None,
            completed: None,
            in_flight: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LyricsRuntimeStatus {
    Idle,
    Loading,
    Ready,
    NotFound,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsRuntimeSnapshot {
    pub track_key: Option<String>,
    pub document: Option<LyricsDocument>,
    pub status: LyricsRuntimeStatus,
    pub error: Option<String>,
}

impl Default for LyricsRuntimeSnapshot {
    fn default() -> Self {
        Self {
            track_key: None,
            document: None,
            status: LyricsRuntimeStatus::Idle,
            error: None,
        }
    }
}
