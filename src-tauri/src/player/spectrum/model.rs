use serde::Serialize;

use super::super::{now_ms, PlaybackSnapshot, PlayerKind};

pub(super) const VISUAL_BAR_COUNT: usize = 6;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackSpectrumStatus {
    Idle,
    Waiting,
    Starting,
    Running,
    PermissionDenied,
    Unsupported,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSpectrumState {
    pub status: PlaybackSpectrumStatus,
    pub source_app_bundle_id: Option<String>,
    pub error: Option<String>,
}

impl Default for PlaybackSpectrumState {
    fn default() -> Self {
        Self {
            status: PlaybackSpectrumStatus::Idle,
            source_app_bundle_id: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSpectrumFrame {
    pub bands: [f32; VISUAL_BAR_COUNT],
    pub source_app_bundle_id: Option<String>,
    pub observed_at_ms: u64,
}

impl PlaybackSpectrumFrame {
    pub(super) fn silent(source_app_bundle_id: Option<String>) -> Self {
        Self {
            bands: [0.0; VISUAL_BAR_COUNT],
            source_app_bundle_id,
            observed_at_ms: now_ms(),
        }
    }
}

pub(super) fn spectrum_target_bundle_id(snapshot: &PlaybackSnapshot) -> Option<&str> {
    // 播放中允许元数据尚未到达；暂停时需有曲目，才能保持订阅并等待恢复。
    let has_title = snapshot
        .title
        .as_deref()
        .is_some_and(|title| !title.trim().is_empty());
    if !snapshot.is_running || (!snapshot.is_playing && !has_title) {
        return None;
    }
    match snapshot.player {
        Some(PlayerKind::AppleMusic) => Some("com.apple.Music"),
        Some(PlayerKind::Spotify) => Some("com.spotify.client"),
        Some(PlayerKind::System) => snapshot.source_app_bundle_id.as_deref(),
        None => None,
    }
}
