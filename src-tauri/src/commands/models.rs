pub(crate) use crate::lyrics::{
    LyricsLoadResponse, LyricsLoadStatus, LyricsMonitor, LyricsRuntimeSnapshot,
    LyricsRuntimeStatus, LyricsSearchFlight, LyricsSearchIntent, LyricsSearchRequestKey,
    SaveLyricsInput, SearchResponse, LYRICS_SEARCH_INVALIDATED,
};
pub(crate) use crate::runtime_model::{NotchLayoutMetrics, OverlaySettings};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalNoticeStatus {
    pub current_version: u16,
    pub accepted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalShortcutStatus {
    pub toggle_overlay: bool,
    pub unlock_overlay: bool,
    pub reset_overlay: bool,
    pub toggle_status_bar_lyrics: bool,
    pub toggle_list_lyrics: bool,
    pub toggle_notch_lyrics: bool,
}
