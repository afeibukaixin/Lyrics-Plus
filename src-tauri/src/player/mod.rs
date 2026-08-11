use std::process::{Command, Output, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

use crate::config::SystemMediaApplication;

mod system_media;
pub use system_media::SystemMediaService;

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

const JXA_SCRIPT: &str = r#"
ObjC.import('Foundation');
function value(callable, fallback) {
  try { const v = callable(); return v === undefined || v === null ? fallback : v; }
  catch (_) { return fallback; }
}
function result(playerName) {
  const isMusic = playerName === 'apple_music';
  const appPath = $.NSProcessInfo.processInfo.environment.objectForKey('LYRICS_PLUS_APP_PATH').js;
  const app = Application(appPath);
  const running = value(() => app.running(), false);
  if (!running) return { player: playerName, isRunning: false, isPlaying: false, canSeek: false };
  const state = String(value(() => app.playerState(), 'stopped')).toLowerCase();
  if (state === 'stopped') return { player: playerName, isRunning: true, isPlaying: false, canSeek: false };
  const track = value(() => app.currentTrack(), null);
  if (!track) return { player: playerName, isRunning: true, isPlaying: state === 'playing', canSeek: false };
  const durationRaw = Number(value(() => track.duration(), 0));
  const durationMs = isMusic ? Math.round(durationRaw * 1000) : Math.round(durationRaw);
  const positionMs = Math.round(Number(value(() => app.playerPosition(), 0)) * 1000);
  const trackId = String(value(() => isMusic ? track.persistentID() : track.id(), '')) || null;
  return {
    player: playerName,
    isRunning: true,
    isPlaying: state === 'playing',
    trackId,
    title: value(() => track.name(), null),
    artist: value(() => track.artist(), null),
    album: value(() => track.album(), null),
    durationMs: durationMs > 0 ? durationMs : null,
    positionMs: positionMs >= 0 ? positionMs : null,
    canSeek: true
  };
}
JSON.stringify(result($.NSProcessInfo.processInfo.environment.objectForKey('LYRICS_PLUS_PLAYER').js));
"#;

fn query_player(kind: PlayerKind) -> PlaybackSnapshot {
    let (player_value, app_path) = match kind {
        PlayerKind::AppleMusic => (
            "apple_music",
            std::path::PathBuf::from("/System/Applications/Music.app"),
        ),
        PlayerKind::Spotify => {
            let system = std::path::PathBuf::from("/Applications/Spotify.app");
            let user = std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join("Applications/Spotify.app"));
            let path = if system.exists() {
                system
            } else {
                user.unwrap_or(system)
            };
            ("spotify", path)
        }
        PlayerKind::System => unreachable!("system playback uses SystemMediaService"),
    };
    if !app_path.exists() {
        let name = if kind == PlayerKind::AppleMusic {
            "Apple Music"
        } else {
            "Spotify"
        };
        return PlaybackSnapshot::unavailable_with_code(
            Some(kind),
            PlaybackErrorCode::NotInstalled,
            format!("未安装 {name}"),
        );
    }
    let mut command = Command::new("/usr/bin/osascript");
    command
        .args(["-l", "JavaScript", "-e", JXA_SCRIPT])
        .env("LYRICS_PLUS_PLAYER", player_value)
        .env("LYRICS_PLUS_APP_PATH", app_path);
    let output = run_with_timeout(command, Duration::from_secs(3));

    match output {
        Ok(output) if output.status.success() => {
            let mut snapshot = serde_json::from_slice::<PlaybackSnapshot>(&output.stdout)
                .unwrap_or_else(|error| {
                    PlaybackSnapshot::unavailable_with_code(
                        Some(kind),
                        PlaybackErrorCode::InvalidResponse,
                        format!("无法解析播放器响应：{error}"),
                    )
                });
            snapshot.observed_at_ms = now_ms();
            ensure_track_id(&mut snapshot);
            snapshot
        }
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if detail.contains("-1743")
                || detail.to_lowercase().contains("not authorized")
            {
                "没有播放器自动化权限。请到“系统设置 → 隐私与安全性 → 自动化”允许 Lyrics Plus 控制播放器。".into()
            } else {
                detail
            };
            let error_code = if detail.contains("自动化权限") {
                PlaybackErrorCode::AutomationDenied
            } else {
                PlaybackErrorCode::Unavailable
            };
            PlaybackSnapshot::unavailable_with_code(
                Some(kind),
                error_code,
                if detail.is_empty() {
                    "播放器未授权或暂不可用".into()
                } else {
                    detail
                },
            )
        }
        Err(error) => PlaybackSnapshot::unavailable_with_code(
            Some(kind),
            if error == PROCESS_TIMEOUT_ERROR {
                PlaybackErrorCode::ResponseTimeout
            } else {
                PlaybackErrorCode::Unavailable
            },
            error,
        ),
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
            source_app_bundle_id: Some("org.example.Player".into()),
            ..PlaybackSnapshot::default()
        };
        assert!(system_source_allowed(&snapshot, &[]));
        assert!(!system_source_allowed(
            &snapshot,
            &[SystemMediaApplication {
                name: "Other".into(),
                bundle_id: "org.example.Other".into(),
            }],
        ));
        snapshot.source_app_bundle_id = Some("com.apple.Music".into());
        assert!(system_source_allowed(
            &snapshot,
            &[SystemMediaApplication {
                name: "Other".into(),
                bundle_id: "org.example.Other".into(),
            }],
        ));
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

        let allowed = [SystemMediaApplication {
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
    system_media_applications: &[SystemMediaApplication],
) -> (PlaybackSnapshot, Option<PlayerKind>) {
    match selection {
        PlayerSelection::AppleMusic => (query_player(PlayerKind::AppleMusic), None),
        PlayerSelection::Spotify => (query_player(PlayerKind::Spotify), None),
        PlayerSelection::System => (system_media.snapshot(), None),
        PlayerSelection::Auto => query_auto_player(
            system_media.snapshot(),
            previous_auto_player,
            system_media_applications,
            query_player,
        ),
    }
}

fn query_auto_player(
    system: PlaybackSnapshot,
    previous_auto_player: Option<PlayerKind>,
    system_media_applications: &[SystemMediaApplication],
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
            _ if system_source_allowed(&system, system_media_applications) => {
                return (system, Some(PlayerKind::System));
            }
            _ => return (source_not_allowed(&system), Some(PlayerKind::System)),
        }
    }
    if system.source_app_bundle_id.is_some()
        && !system_source_allowed(&system, system_media_applications)
    {
        return (source_not_allowed(&system), Some(PlayerKind::System));
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
    applications: &[SystemMediaApplication],
) -> bool {
    let Some(bundle_id) = snapshot.source_app_bundle_id.as_deref() else {
        return applications.is_empty();
    };
    matches!(bundle_id, "com.apple.Music" | "com.spotify.client")
        || applications.is_empty()
        || applications
            .iter()
            .any(|application| application.bundle_id == bundle_id)
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
    action: &str,
    position_ms: Option<u64>,
) -> Result<(), String> {
    let app = match kind {
        PlayerKind::AppleMusic => "Music",
        PlayerKind::Spotify => "Spotify",
        PlayerKind::System => return Err("系统播放器操作必须通过系统媒体服务执行".into()),
    };
    let script = match action {
        "play_pause" => format!("tell application \"{app}\" to playpause"),
        "next" => format!("tell application \"{app}\" to next track"),
        "previous" => format!("tell application \"{app}\" to previous track"),
        "seek" => format!(
            "tell application \"{app}\" to set player position to {}",
            position_ms.ok_or_else(|| "缺少跳转位置".to_string())? as f64 / 1000.0
        ),
        _ => return Err("未知播放器操作".into()),
    };
    let mut command = Command::new("/usr/bin/osascript");
    command.args(["-e", &script]);
    let output = run_with_timeout(command, Duration::from_secs(3))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}
