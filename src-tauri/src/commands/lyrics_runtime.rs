// 兼容 façade：运行时实现归 `lyrics::runtime`，命令模块继续通过原有名称调用。
pub(crate) use crate::lyrics::runtime::{
    completed_lyrics_search, invalidate_lyrics_search_session, playback_track_key,
    reload_active_lyrics_runtime, republish_lyrics_runtime, search_lyrics_for_session,
    set_runtime_document_if_active, sync_desktop_style_from_config, sync_lyrics_runtime,
};
#[cfg(test)]
use crate::lyrics::runtime::candidate_capability_rank;
pub use crate::lyrics::runtime::{LyricsStyleMode, SettingsSection};
