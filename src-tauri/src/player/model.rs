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
    /// 仅用于界面提示；发送系统媒体命令前还会重新核对当前来源。
    pub system_control_available: bool,
    /// 仅用于按钮和自动隐藏；计时与路由始终使用 is_playing。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_is_playing: Option<bool>,
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
            system_control_available: false,
            display_is_playing: None,
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
    pub fn is_playing_for_display(&self) -> bool {
        self.display_is_playing.unwrap_or(self.is_playing)
    }

    /// 系统当前媒体与选中来源必须同时有可核对的应用和曲目标识。
    pub(crate) fn same_system_media(&self, other: &Self) -> bool {
        self.player == Some(PlayerKind::System)
            && other.player == Some(PlayerKind::System)
            && self.is_running
            && other.is_running
            && self.error_code.is_none()
            && other.error_code.is_none()
            && self
                .source_app_bundle_id
                .as_deref()
                .filter(|value| !value.is_empty())
                .is_some_and(|bundle_id| other.source_app_bundle_id.as_deref() == Some(bundle_id))
            && self
                .track_id
                .as_deref()
                .filter(|value| !value.is_empty())
                .is_some_and(|track_id| other.track_id.as_deref() == Some(track_id))
    }

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
            system_control_available: false,
            display_is_playing: None,
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
