use std::process::{Command, Output, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

use crate::config::{is_dedicated_player_bundle_id, RegisteredApplication};

pub(crate) mod automation;
mod system;
pub use system::SystemMediaService;

const PROCESS_TIMEOUT_ERROR: &str = "Process timed out";

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
    pub duration_ms: Option<u64>,
    pub position_ms: Option<u64>,
    pub can_seek: bool,
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
            duration_ms: None,
            position_ms: None,
            can_seek: false,
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
            duration_ms: None,
            position_ms: None,
            can_seek: false,
            observed_at_ms: now_ms(),
            error_code: Some(error_code),
            error: Some(error),
        }
    }

    pub fn matches_track(&self, player: PlayerKind, track_id: &str) -> bool {
        self.player == Some(player) && self.track_id.as_deref() == Some(track_id)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn normalized_track_component(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn ensure_track_id(snapshot: &mut PlaybackSnapshot) {
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

pub(crate) fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<Output, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    match child
        .wait_timeout(timeout)
        .map_err(|error| error.to_string())?
    {
        Some(_) => child.wait_with_output().map_err(|error| error.to_string()),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            Err(PROCESS_TIMEOUT_ERROR.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_stable_fallback_track_id() {
        let mut snapshot = PlaybackSnapshot {
            title: Some("  Test   Song ".into()),
            artist: Some("Some ARTIST".into()),
            duration_ms: Some(123_000),
            ..PlaybackSnapshot::default()
        };
        ensure_track_id(&mut snapshot);
        assert_eq!(
            snapshot.track_id.as_deref(),
            Some("fallback:test song|some artist|123000")
        );
    }

    #[test]
    fn preserves_player_track_id() {
        let mut snapshot = PlaybackSnapshot {
            track_id: Some("native-id".into()),
            title: Some("Test".into()),
            artist: Some("Artist".into()),
            ..PlaybackSnapshot::default()
        };
        ensure_track_id(&mut snapshot);
        assert_eq!(snapshot.track_id.as_deref(), Some("native-id"));
    }

    #[test]
    fn matches_only_the_current_player_and_track() {
        let snapshot = PlaybackSnapshot {
            player: Some(PlayerKind::Spotify),
            track_id: Some("current-track".into()),
            ..PlaybackSnapshot::default()
        };
        assert!(snapshot.matches_track(PlayerKind::Spotify, "current-track"));
        assert!(!snapshot.matches_track(PlayerKind::Spotify, "other-track"));
        assert!(!snapshot.matches_track(PlayerKind::AppleMusic, "current-track"));
    }

    #[test]
    fn empty_snapshot_exposes_a_stable_waiting_error_code() {
        let snapshot = PlaybackSnapshot::empty();
        assert_eq!(snapshot.error_code, Some(PlaybackErrorCode::Waiting));
        assert!(snapshot.error.is_some());
    }

    #[test]
    fn restores_system_player_selection() {
        assert_eq!(
            PlayerSelection::from_stored(Some("system".into())),
            PlayerSelection::System
        );
        assert_eq!(
            PlayerSelection::System.preferred_kind(),
            Some(PlayerKind::System)
        );
    }

    #[test]
    fn system_source_allowlist_only_filters_third_party_apps() {
        let mut snapshot = PlaybackSnapshot {
            is_running: true,
            source_app_bundle_id: Some("org.example.Player".into()),
            ..PlaybackSnapshot::default()
        };
        assert!(system_source_allowed(&snapshot, &[]));
        assert!(!system_source_allowed(
            &snapshot,
            &[RegisteredApplication {
                name: "Other".into(),
                bundle_id: "org.example.Other".into(),
            }],
        ));
        snapshot.source_app_bundle_id = Some("com.apple.Music".into());
        assert!(system_source_allowed(
            &snapshot,
            &[RegisteredApplication {
                name: "Other".into(),
                bundle_id: "org.example.Other".into(),
            }],
        ));
    }

    #[test]
    fn manual_system_source_uses_the_same_allowlist() {
        let snapshot = PlaybackSnapshot {
            player: Some(PlayerKind::System),
            is_running: true,
            source_app_bundle_id: Some("org.example.Player".into()),
            ..PlaybackSnapshot::default()
        };
        let allowed = [RegisteredApplication {
            name: "Player".into(),
            bundle_id: "org.example.Player".into(),
        }];
        assert_eq!(filter_system_source(snapshot.clone(), &[]).error_code, None);
        assert_eq!(
            filter_system_source(snapshot.clone(), &allowed).error_code,
            None
        );
        assert_eq!(
            filter_system_source(
                snapshot,
                &[RegisteredApplication {
                    name: "Other".into(),
                    bundle_id: "org.example.Other".into(),
                }],
            )
            .error_code,
            Some(PlaybackErrorCode::SourceNotAllowed)
        );

        for bundle_id in ["com.apple.Music", "com.spotify.client"] {
            let builtin = PlaybackSnapshot {
                player: Some(PlayerKind::System),
                is_running: true,
                source_app_bundle_id: Some(bundle_id.into()),
                ..PlaybackSnapshot::default()
            };
            assert_eq!(filter_system_source(builtin, &allowed).error_code, None);
        }
    }

    #[test]
    fn automatic_routing_prefers_system_source_then_native_fallbacks() {
        let playing_system = |bundle_id: &str| PlaybackSnapshot {
            player: Some(PlayerKind::System),
            is_running: true,
            is_playing: true,
            title: Some("Track".into()),
            source_app_bundle_id: Some(bundle_id.into()),
            ..PlaybackSnapshot::default()
        };
        let native_music = PlaybackSnapshot {
            player: Some(PlayerKind::AppleMusic),
            is_running: true,
            is_playing: true,
            title: Some("Track".into()),
            ..PlaybackSnapshot::default()
        };
        let (snapshot, selected) =
            query_auto_player(playing_system("com.apple.Music"), None, &[], |kind| {
                if kind == PlayerKind::AppleMusic {
                    native_music.clone()
                } else {
                    PlaybackSnapshot::default()
                }
            });
        assert_eq!(snapshot.player, Some(PlayerKind::AppleMusic));
        assert_eq!(selected, Some(PlayerKind::AppleMusic));

        let (snapshot, selected) =
            query_auto_player(playing_system("com.spotify.client"), None, &[], |_| {
                PlaybackSnapshot::default()
            });
        assert_eq!(snapshot.player, Some(PlayerKind::System));
        assert_eq!(selected, Some(PlayerKind::System));

        let allowed = [RegisteredApplication {
            name: "Browser".into(),
            bundle_id: "org.example.Browser".into(),
        }];
        let (snapshot, _) = query_auto_player(
            playing_system("org.example.Browser"),
            None,
            &allowed,
            |_| PlaybackSnapshot::default(),
        );
        assert_eq!(snapshot.error_code, None);
        let (snapshot, _) = query_auto_player(
            playing_system("org.example.Blocked"),
            None,
            &allowed,
            |_| PlaybackSnapshot::default(),
        );
        assert_eq!(
            snapshot.error_code,
            Some(PlaybackErrorCode::SourceNotAllowed)
        );
    }

    #[test]
    fn automatic_routing_keeps_paused_system_source_and_uses_legacy_detection_without_one() {
        let paused = PlaybackSnapshot {
            player: Some(PlayerKind::System),
            is_running: true,
            title: Some("Paused Track".into()),
            source_app_bundle_id: Some("org.example.Player".into()),
            ..PlaybackSnapshot::default()
        };
        let (snapshot, selected) = query_auto_player(paused, Some(PlayerKind::System), &[], |_| {
            PlaybackSnapshot::default()
        });
        assert_eq!(snapshot.title.as_deref(), Some("Paused Track"));
        assert_eq!(selected, Some(PlayerKind::System));

        let (snapshot, selected) =
            query_auto_player(PlaybackSnapshot::default(), None, &[], |kind| {
                PlaybackSnapshot {
                    player: Some(kind),
                    is_running: true,
                    is_playing: kind == PlayerKind::Spotify,
                    ..PlaybackSnapshot::default()
                }
            });
        assert_eq!(snapshot.player, Some(PlayerKind::Spotify));
        assert_eq!(selected, Some(PlayerKind::Spotify));
    }
}

pub fn query_selected_player(
    selection: PlayerSelection,
    previous_auto_player: Option<PlayerKind>,
    system_media: &SystemMediaService,
    system_media_applications: &[RegisteredApplication],
) -> (PlaybackSnapshot, Option<PlayerKind>) {
    match selection {
        PlayerSelection::AppleMusic => (automation::snapshot(PlayerKind::AppleMusic), None),
        PlayerSelection::Spotify => (automation::snapshot(PlayerKind::Spotify), None),
        PlayerSelection::System => (
            filter_system_source(system_media.snapshot(), system_media_applications),
            None,
        ),
        PlayerSelection::Auto => query_auto_player(
            system_media.snapshot(),
            previous_auto_player,
            system_media_applications,
            automation::snapshot,
        ),
    }
}

fn query_auto_player(
    system: PlaybackSnapshot,
    previous_auto_player: Option<PlayerKind>,
    system_media_applications: &[RegisteredApplication],
    query: impl Fn(PlayerKind) -> PlaybackSnapshot,
) -> (PlaybackSnapshot, Option<PlayerKind>) {
    if system.is_playing {
        match system.source_app_bundle_id.as_deref() {
            Some("com.apple.Music") => {
                let music = query(PlayerKind::AppleMusic);
                return if music.is_playing {
                    (music, Some(PlayerKind::AppleMusic))
                } else {
                    (system, Some(PlayerKind::System))
                };
            }
            Some("com.spotify.client") => {
                let spotify = query(PlayerKind::Spotify);
                return if spotify.is_playing {
                    (spotify, Some(PlayerKind::Spotify))
                } else {
                    (system, Some(PlayerKind::System))
                };
            }
            _ => {
                return (
                    filter_system_source(system, system_media_applications),
                    Some(PlayerKind::System),
                )
            }
        }
    }
    let system = filter_system_source(system, system_media_applications);
    if system.error_code == Some(PlaybackErrorCode::SourceNotAllowed) {
        return (system, Some(PlayerKind::System));
    }
    if previous_auto_player == Some(PlayerKind::System)
        && system.is_running
        && system.title.is_some()
        && system_source_allowed(&system, system_media_applications)
    {
        return (system, previous_auto_player);
    }
    let music = query(PlayerKind::AppleMusic);
    let spotify = query(PlayerKind::Spotify);
    if music.is_playing && spotify.is_playing {
        (
            PlaybackSnapshot::unavailable_with_code(
                None,
                PlaybackErrorCode::MultiplePlaying,
                "Apple Music 与 Spotify 同时在播放，请手动选择播放器".into(),
            ),
            None,
        )
    } else if music.is_playing {
        (music, Some(PlayerKind::AppleMusic))
    } else if spotify.is_playing {
        (spotify, Some(PlayerKind::Spotify))
    } else if previous_auto_player == Some(PlayerKind::AppleMusic)
        && music.is_running
        && music.title.is_some()
    {
        (music, previous_auto_player)
    } else if previous_auto_player == Some(PlayerKind::Spotify)
        && spotify.is_running
        && spotify.title.is_some()
    {
        (spotify, previous_auto_player)
    } else {
        (
            PlaybackSnapshot::unavailable_with_code(
                None,
                PlaybackErrorCode::NoUniquePlayer,
                "未检测到唯一正在播放的 Apple Music 或 Spotify".into(),
            ),
            None,
        )
    }
}

fn system_source_allowed(
    snapshot: &PlaybackSnapshot,
    applications: &[RegisteredApplication],
) -> bool {
    let Some(bundle_id) = snapshot.source_app_bundle_id.as_deref() else {
        return applications.is_empty();
    };
    is_dedicated_player_bundle_id(bundle_id)
        || applications.is_empty()
        || applications
            .iter()
            .any(|application| application.bundle_id == bundle_id)
}

fn filter_system_source(
    snapshot: PlaybackSnapshot,
    applications: &[RegisteredApplication],
) -> PlaybackSnapshot {
    if !snapshot.is_running || system_source_allowed(&snapshot, applications) {
        snapshot
    } else {
        source_not_allowed(&snapshot)
    }
}

fn source_not_allowed(snapshot: &PlaybackSnapshot) -> PlaybackSnapshot {
    let mut unavailable = PlaybackSnapshot::unavailable_with_code(
        Some(PlayerKind::System),
        PlaybackErrorCode::SourceNotAllowed,
        "当前系统播放应用不在允许列表中".into(),
    );
    unavailable.source_app_name = snapshot.source_app_name.clone();
    unavailable.source_app_bundle_id = snapshot.source_app_bundle_id.clone();
    unavailable
}

pub fn perform_action(
    kind: PlayerKind,
    system_media: &SystemMediaService,
    action: &str,
    position_ms: Option<u64>,
) -> Result<(), String> {
    match kind {
        PlayerKind::System => system_media.perform_action(action, position_ms),
        PlayerKind::AppleMusic | PlayerKind::Spotify => {
            automation::perform_action(kind, action, position_ms)
        }
    }
}
