use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerKind {
    AppleMusic,
    Spotify,
    System,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerSelection {
    Auto,
    AppleMusic,
    Spotify,
    System,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackErrorCode {
    Waiting,
    NotInstalled,
    AutomationDenied,
    ResponseTimeout,
    InvalidResponse,
    MultiplePlaying,
    NoUniquePlayer,
    SourceNotAllowed,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackAction {
    Play,
    Pause,
    TogglePlayPause,
    Previous,
    Next,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSpectrumColors {
    pub left: PlaybackSpectrumColumnColors,
    pub center: PlaybackSpectrumColumnColors,
    pub right: PlaybackSpectrumColumnColors,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSpectrumColumnColors {
    pub top: String,
    pub middle: String,
    pub bottom: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackArtwork {
    pub id: String,
    pub mime_type: String,
    pub data_base64: String,
    pub accent_color: String,
    pub spectrum_colors: PlaybackSpectrumColors,
}

impl PlayerSelection {
    pub fn preferred_kind(self) -> Option<PlayerKind> {
        match self {
            Self::Auto => None,
            Self::AppleMusic => Some(PlayerKind::AppleMusic),
            Self::Spotify => Some(PlayerKind::Spotify),
            Self::System => Some(PlayerKind::System),
        }
    }

    pub fn from_stored(value: Option<String>) -> Self {
        match value.as_deref() {
            Some("apple_music") => Self::AppleMusic,
            Some("spotify") => Self::Spotify,
            Some("system") => Self::System,
            _ => Self::Auto,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    pub player: Option<PlayerKind>,
    pub is_running: bool,
    pub is_playing: bool,
    pub track_id: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub source_app_name: Option<String>,
    pub source_app_bundle_id: Option<String>,
    pub artwork_id: Option<String>,
    pub duration_ms: Option<u64>,
    pub position_ms: Option<u64>,
    pub observed_at_ms: u64,
    pub error_code: Option<PlaybackErrorCode>,
    pub error: Option<String>,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            player: None,
            is_running: false,
            is_playing: false,
            track_id: None,
            title: None,
            artist: None,
            album: None,
            source_app_name: None,
            source_app_bundle_id: None,
            artwork_id: None,
            duration_ms: None,
            position_ms: None,
            observed_at_ms: 0,
            error_code: None,
            error: None,
        }
    }
}

impl PlaybackSnapshot {
    pub fn empty() -> Self {
        Self::unavailable_with_code(None, PlaybackErrorCode::Waiting, "等待播放器".into())
    }

    pub fn unavailable(player: Option<PlayerKind>, error: String) -> Self {
        Self::unavailable_with_code(player, PlaybackErrorCode::Unavailable, error)
    }

    pub fn unavailable_with_code(
        player: Option<PlayerKind>,
        error_code: PlaybackErrorCode,
        error: String,
    ) -> Self {
        Self {
            player,
            is_running: false,
            is_playing: false,
            track_id: None,
            title: None,
            artist: None,
            album: None,
            source_app_name: None,
            source_app_bundle_id: None,
            artwork_id: None,
            duration_ms: None,
            position_ms: None,
            observed_at_ms: now_ms(),
            error_code: Some(error_code),
            error: Some(error),
        }
    }
}

pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(super) fn normalized_track_component(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(super) fn ensure_track_id(snapshot: &mut PlaybackSnapshot) {
    if snapshot
        .track_id
        .as_deref()
        .is_some_and(|id| !id.is_empty())
    {
        return;
    }
    let (Some(title), Some(artist)) = (&snapshot.title, &snapshot.artist) else {
        return;
    };
    snapshot.track_id = Some(format!(
        "fallback:{}|{}|{}",
        normalized_track_component(title),
        normalized_track_component(artist),
        snapshot.duration_ms.unwrap_or_default()
    ));
}
