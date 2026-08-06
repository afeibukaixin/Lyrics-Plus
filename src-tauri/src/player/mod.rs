use std::process::{Command, Output, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerKind {
    AppleMusic,
    Spotify,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerSelection {
    Auto,
    AppleMusic,
    Spotify,
}

impl PlayerSelection {
    pub fn preferred_kind(self) -> Option<PlayerKind> {
        match self {
            Self::Auto => None,
            Self::AppleMusic => Some(PlayerKind::AppleMusic),
            Self::Spotify => Some(PlayerKind::Spotify),
        }
    }

    pub fn from_stored(value: Option<String>) -> Self {
        match value.as_deref() {
            Some("apple_music") => Self::AppleMusic,
            Some("spotify") => Self::Spotify,
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
    pub duration_ms: Option<u64>,
    pub position_ms: Option<u64>,
    pub can_seek: bool,
    pub observed_at_ms: u64,
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
            duration_ms: None,
            position_ms: None,
            can_seek: false,
            observed_at_ms: 0,
            error: None,
        }
    }
}

impl PlaybackSnapshot {
    pub fn empty() -> Self {
        Self::unavailable(None, "等待播放器".into())
    }

    pub fn unavailable(player: Option<PlayerKind>, error: String) -> Self {
        Self {
            player,
            is_running: false,
            is_playing: false,
            track_id: None,
            title: None,
            artist: None,
            album: None,
            duration_ms: None,
            position_ms: None,
            can_seek: false,
            observed_at_ms: now_ms(),
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
            Err("播放器响应超时".into())
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
    };
    if !app_path.exists() {
        let name = if kind == PlayerKind::AppleMusic {
            "Apple Music"
        } else {
            "Spotify"
        };
        return PlaybackSnapshot::unavailable(Some(kind), format!("未安装 {name}"));
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
                    PlaybackSnapshot::unavailable(
                        Some(kind),
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
            PlaybackSnapshot::unavailable(
                Some(kind),
                if detail.is_empty() {
                    "播放器未授权或暂不可用".into()
                } else {
                    detail
                },
            )
        }
        Err(error) => PlaybackSnapshot::unavailable(Some(kind), error),
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
}

pub fn query_selected_player(
    selection: PlayerSelection,
    previous_auto_player: Option<PlayerKind>,
) -> (PlaybackSnapshot, Option<PlayerKind>) {
    match selection {
        PlayerSelection::AppleMusic => (query_player(PlayerKind::AppleMusic), None),
        PlayerSelection::Spotify => (query_player(PlayerKind::Spotify), None),
        PlayerSelection::Auto => {
            let music = query_player(PlayerKind::AppleMusic);
            let spotify = query_player(PlayerKind::Spotify);
            if music.is_playing && spotify.is_playing {
                (
                    PlaybackSnapshot::unavailable(
                        None,
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
                    PlaybackSnapshot::unavailable(
                        None,
                        "未检测到唯一正在播放的 Apple Music 或 Spotify".into(),
                    ),
                    None,
                )
            }
        }
    }
}

pub fn perform_action(
    kind: PlayerKind,
    action: &str,
    position_ms: Option<u64>,
) -> Result<(), String> {
    let app = match kind {
        PlayerKind::AppleMusic => "Music",
        PlayerKind::Spotify => "Spotify",
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
