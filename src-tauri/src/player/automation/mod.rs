use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::{
    ensure_track_id, now_ms, run_with_timeout, PlaybackErrorCode, PlaybackSnapshot, PlayerKind,
};

mod apple_music;
mod spotify;

pub(crate) use apple_music::{
    export_artwork as export_apple_music_artwork, ArtworkExport as AppleMusicArtworkExport,
};
pub(crate) use spotify::artwork_url as spotify_artwork_url;

const QUERY_SCRIPT: &str = r#"
ObjC.import('Foundation');
function value(callable, fallback) {
  try { const v = callable(); return v === undefined || v === null ? fallback : v; }
  catch (_) { return fallback; }
}
function result() {
  const environment = $.NSProcessInfo.processInfo.environment;
  const playerName = environment.objectForKey('LYRICS_PLUS_PLAYER').js;
  const appPath = environment.objectForKey('LYRICS_PLUS_APP_PATH').js;
  const durationScale = Number(environment.objectForKey('LYRICS_PLUS_DURATION_SCALE').js);
  const trackIdProperty = environment.objectForKey('LYRICS_PLUS_TRACK_ID_PROPERTY').js;
  const app = Application(appPath);
  const running = value(() => app.running(), false);
  if (!running) return { player: playerName, isRunning: false, isPlaying: false, canSeek: false };
  const state = String(value(() => app.playerState(), 'stopped')).toLowerCase();
  if (state === 'stopped') return { player: playerName, isRunning: true, isPlaying: false, canSeek: false };
  const track = value(() => app.currentTrack(), null);
  if (!track) return { player: playerName, isRunning: true, isPlaying: state === 'playing', canSeek: false };
  const durationMs = Math.round(Number(value(() => track.duration(), 0)) * durationScale);
  const positionMs = Math.round(Number(value(() => app.playerPosition(), 0)) * 1000);
  const trackId = String(value(() => track[trackIdProperty](), '')) || null;
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
JSON.stringify(result());
"#;

pub(crate) fn snapshot(kind: PlayerKind) -> PlaybackSnapshot {
    match kind {
        PlayerKind::AppleMusic => apple_music::snapshot(),
        PlayerKind::Spotify => spotify::snapshot(),
        PlayerKind::System => unreachable!("system playback uses SystemMediaService"),
    }
}

pub(crate) fn perform_action(
    kind: PlayerKind,
    action: &str,
    position_ms: Option<u64>,
) -> Result<(), String> {
    match kind {
        PlayerKind::AppleMusic => apple_music::perform_action(action, position_ms),
        PlayerKind::Spotify => spotify::perform_action(action, position_ms),
        PlayerKind::System => Err("系统播放器操作必须通过系统媒体服务执行".into()),
    }
}

fn query(
    kind: PlayerKind,
    player_value: &str,
    display_name: &str,
    app_path: &Path,
    duration_scale: u64,
    track_id_property: &str,
) -> PlaybackSnapshot {
    if !app_path.exists() {
        return PlaybackSnapshot::unavailable_with_code(
            Some(kind),
            PlaybackErrorCode::NotInstalled,
            format!("未安装 {display_name}"),
        );
    }
    let mut command = Command::new("/usr/bin/osascript");
    command
        .args(["-l", "JavaScript", "-e", QUERY_SCRIPT])
        .env("LYRICS_PLUS_PLAYER", player_value)
        .env("LYRICS_PLUS_APP_PATH", app_path)
        .env("LYRICS_PLUS_DURATION_SCALE", duration_scale.to_string())
        .env("LYRICS_PLUS_TRACK_ID_PROPERTY", track_id_property);
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
            if error == super::PROCESS_TIMEOUT_ERROR {
                PlaybackErrorCode::ResponseTimeout
            } else {
                PlaybackErrorCode::Unavailable
            },
            error,
        ),
    }
}

fn perform_action_for_app(app: &str, action: &str, position_ms: Option<u64>) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[cfg(target_os = "macos")]
    #[test]
    fn query_script_compiles() {
        let root = tempdir().unwrap();
        let output_path = root.path().join("player-query.scpt");
        let output = Command::new("/usr/bin/osacompile")
            .args(["-l", "JavaScript", "-o"])
            .arg(&output_path)
            .args(["-e", QUERY_SCRIPT])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "播放器查询脚本编译失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn osascript_preserves_unicode_arguments() {
        let output = Command::new("/usr/bin/osascript")
            .args([
                "-e",
                "on run argv\nreturn (item 1 of argv) & \"|\" & (item 2 of argv)\nend run",
                "--",
                "催眠",
                "王菲",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "AppleScript Unicode 参数读取失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "催眠|王菲");
    }
}
