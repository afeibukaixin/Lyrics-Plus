mod controller;
mod model;
mod publication;
mod ranking;
mod search;
mod style;

pub(crate) use model::LyricsSearchSession;
pub use model::{
    LyricsLoadResponse, LyricsLoadStatus, LyricsMonitor, LyricsRuntimeSnapshot, LyricsSearchIntent,
    SaveLyricsInput, SearchResponse,
};
pub use style::{LyricsStyleMode, SettingsSection};

pub(crate) use controller::{
    playback_track_key, reload_active_lyrics_runtime, set_runtime_document_if_active,
    sync_lyrics_runtime,
};
pub(crate) use publication::republish_lyrics_runtime;
#[cfg(test)]
pub(crate) use ranking::candidate_capability_rank;
pub(crate) use search::{
    completed_lyrics_search, invalidate_lyrics_search_session, search_lyrics_for_session,
};
pub(crate) use style::sync_desktop_style_from_config;
