use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime};

use image::DynamicImage;
use media_remote::{NowPlayingInfo, NowPlayingPerl, Subscription};
use serde_json::Value;

use super::{
    normalized_track_component, now_ms, run_with_timeout, PlaybackErrorCode, PlaybackSnapshot,
    PlayerKind,
};

const SYSTEM_MEDIA_SEEK_SCRIPT: &str = r#"
ObjC.import('Foundation');
function run(argv) {
  const target = Number(argv[0]);
  const framework = $.NSBundle.bundleWithPath('/System/Library/PrivateFrameworks/MediaRemote.framework/');
  if (!framework.load) return 'framework_unavailable';
  const Controller = $.NSClassFromString('MRNowPlayingController');
  const Request = $.NSClassFromString('MRNowPlayingRequest');
  if (!Controller || !Request) return 'controller_unavailable';
  const controller = Controller.localRouteController;
  const options = $.NSMutableDictionary.dictionary;
  options.setObjectForKey($(target), $('kMRMediaRemoteOptionPlaybackPosition'));
  controller.sendCommandOptionsCompletion(24, options, null);
  $.NSThread.sleepForTimeInterval(0.2);
  const item = Request.localNowPlayingItem;
  if (!item || !item.metadata) return 'media_unavailable';
  const actual = Number(item.metadata.calculatedPlaybackPosition);
  return Number.isFinite(actual) && Math.abs(actual - target) < 2 ? 'ok' : `position:${actual}`;
}
"#;

pub struct SystemMediaService {
    player: OnceLock<Result<AdapterClient, String>>,
}

struct AdapterClient {
    _player: NowPlayingPerl,
    latest: Arc<RwLock<Option<TimedInfo>>>,
    script_path: PathBuf,
    framework_path: PathBuf,
}

#[derive(Clone)]
struct TimedInfo {
    info: NowPlayingInfo,
    received_at: Instant,
}

impl Default for SystemMediaService {
    fn default() -> Self {
        Self {
            player: OnceLock::new(),
        }
    }
}

impl SystemMediaService {
    fn player(&self) -> Result<&AdapterClient, String> {
        self.player
            .get_or_init(|| {
                let existing = adapter_directories();
                let player = catch_unwind(AssertUnwindSafe(NowPlayingPerl::new))
                    .map_err(|_| "无法启动系统媒体适配器".to_string())?;
                let latest = Arc::new(RwLock::new(None));
                let latest_for_listener = latest.clone();
                player.subscribe(move |info| {
                    let next = info.as_ref().cloned().and_then(timed_info);
                    *latest_for_listener
                        .write()
                        .unwrap_or_else(|error| error.into_inner()) = next;
                });
                // ponytail: media-remote 0.3.7 未暴露 Perl 控制命令；固定版本并定位它刚创建的临时目录，上游开放 API 后删除此兼容层。
                let directory = adapter_directories()
                    .into_iter()
                    .find(|path| !existing.contains(path))
                    .ok_or_else(|| "无法定位系统媒体适配器资源".to_string())?;
                let script_path = directory.join("mediaremote-adapter.pl");
                let framework_path = directory.join("MediaRemoteAdapter.framework");
                if !script_path.is_file() || !framework_path.is_dir() {
                    return Err("系统媒体适配器资源不完整".into());
                }
                let output = run_adapter(
                    &script_path,
                    &framework_path,
                    ["get", "--no-artwork", "--now"],
                )?;
                if !output.status.success() {
                    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return Err(if detail.is_empty() {
                        "系统媒体适配器自检失败".into()
                    } else {
                        detail
                    });
                }
                sync_elapsed_from_adapter(&latest, &output.stdout);
                Ok(AdapterClient {
                    _player: player,
                    latest,
                    script_path,
                    framework_path,
                })
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        let player = match self.player() {
            Ok(player) => player,
            Err(error) => {
                return PlaybackSnapshot::unavailable_with_code(
                    Some(PlayerKind::System),
                    PlaybackErrorCode::Unavailable,
                    error,
                )
            }
        };
        let info = player
            .latest
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        info.as_ref().map(snapshot_from_info).unwrap_or_else(|| {
            PlaybackSnapshot::unavailable_with_code(
                Some(PlayerKind::System),
                PlaybackErrorCode::Waiting,
                "未检测到系统正在播放的媒体".into(),
            )
        })
    }

    pub fn artwork(&self, track_id: &str) -> Option<DynamicImage> {
        let player = self.player().ok()?;
        let info = player
            .latest
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()?;
        (system_track_id(&info.info).as_deref() == Some(track_id))
            .then(|| info.info.album_cover.clone())
            .flatten()
    }

    pub fn perform_action(&self, action: &str, position_ms: Option<u64>) -> Result<(), String> {
        let client = self.player()?;
        if client
            .latest
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .is_none()
        {
            return Err("当前没有可控制的系统媒体".into());
        }
        let seek_position_ms = position_ms.map(|position| {
            let duration_ms = client
                .latest
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .as_ref()
                .and_then(|timed| milliseconds(timed.info.duration));
            duration_ms
                .map(|duration| position.min(duration))
                .unwrap_or(position)
        });
        let arguments = match action {
            "play_pause" => vec!["send".to_string(), "2".to_string()],
            "next" => vec!["send".to_string(), "4".to_string()],
            "previous" => vec!["send".to_string(), "5".to_string()],
            "seek" => vec![
                "seek".to_string(),
                seek_position_ms
                    .ok_or_else(|| "缺少跳转位置".to_string())?
                    .saturating_mul(1000)
                    .to_string(),
            ],
            _ => return Err("未知播放器操作".into()),
        };
        let output = run_adapter(&client.script_path, &client.framework_path, arguments)?;
        if output.status.success() {
            if action == "seek" {
                update_elapsed_time(
                    &client.latest,
                    seek_position_ms.expect("seek position checked"),
                );
            }
            Ok(())
        } else if action == "seek" {
            let position = seek_position_ms.expect("seek position checked");
            seek_with_system_controller(position)?;
            update_elapsed_time(&client.latest, position);
            Ok(())
        } else {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if detail.is_empty() {
                "系统媒体命令未被当前应用接受".into()
            } else {
                detail
            })
        }
    }
}

fn sync_elapsed_from_adapter(latest: &RwLock<Option<TimedInfo>>, output: &[u8]) {
    let Ok(payload) = serde_json::from_slice::<Value>(output) else {
        return;
    };
    let Some(position_ms) = milliseconds(payload.get("elapsedTimeNow").and_then(Value::as_f64))
    else {
        return;
    };
    let mut latest = latest.write().unwrap_or_else(|error| error.into_inner());
    let Some(timed) = latest.as_mut() else {
        return;
    };
    let same_track = payload.get("title").and_then(Value::as_str) == timed.info.title.as_deref()
        && payload.get("bundleIdentifier").and_then(Value::as_str)
            == timed.info.bundle_id.as_deref();
    if same_track {
        timed.info.elapsed_time = Some(position_ms as f64 / 1000.0);
        timed.received_at = Instant::now();
    }
}

fn seek_with_system_controller(position_ms: u64) -> Result<(), String> {
    let mut command = Command::new("/usr/bin/osascript");
    command
        .args(["-l", "JavaScript", "-e", SYSTEM_MEDIA_SEEK_SCRIPT, "--"])
        .arg(format!("{:.3}", position_ms as f64 / 1000.0));
    let output = run_with_timeout(command, Duration::from_secs(3))?;
    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() && result == "ok" {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() { result } else { detail })
    }
}

fn update_elapsed_time(latest: &RwLock<Option<TimedInfo>>, position_ms: u64) {
    if let Some(timed) = latest
        .write()
        .unwrap_or_else(|error| error.into_inner())
        .as_mut()
    {
        timed.info.elapsed_time = Some(position_ms as f64 / 1000.0);
        timed.received_at = Instant::now();
    }
}

fn run_adapter(
    script_path: &std::path::Path,
    framework_path: &std::path::Path,
    arguments: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<std::process::Output, String> {
    let mut command = Command::new("/usr/bin/perl");
    command.arg(script_path).arg(framework_path).args(arguments);
    run_with_timeout(command, Duration::from_secs(3))
}

fn adapter_directories() -> Vec<PathBuf> {
    std::fs::read_dir(std::env::temp_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            name.to_string_lossy()
                .starts_with("mediaremote-adapter")
                .then(|| entry.path())
        })
        .collect()
}

fn milliseconds(seconds: Option<f64>) -> Option<u64> {
    seconds
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| (value * 1000.0).round() as u64)
}

fn valid_elapsed_time(info: &NowPlayingInfo) -> bool {
    match (info.elapsed_time, info.duration) {
        (Some(elapsed), Some(duration)) => {
            elapsed.is_finite()
                && duration.is_finite()
                && elapsed >= 0.0
                && duration >= 0.0
                && elapsed <= duration + 5.0
        }
        (Some(elapsed), None) => elapsed.is_finite() && elapsed >= 0.0,
        _ => true,
    }
}

fn timed_info(mut info: NowPlayingInfo) -> Option<TimedInfo> {
    if !valid_elapsed_time(&info) {
        return None;
    }
    if info.is_playing == Some(true) {
        if let (Some(elapsed), Some(updated_at)) = (info.elapsed_time, info.info_update_time) {
            if let Ok(age) = SystemTime::now().duration_since(updated_at) {
                info.elapsed_time = Some(elapsed + age.as_secs_f64());
            }
        }
    }
    Some(TimedInfo {
        info,
        received_at: Instant::now(),
    })
}

fn system_track_id(info: &NowPlayingInfo) -> Option<String> {
    let title = info.title.as_deref()?;
    let artist = info.artist.as_deref().unwrap_or_default();
    Some(format!(
        "system:{}|{}|{}|{}",
        normalized_track_component(info.bundle_id.as_deref().unwrap_or_default()),
        normalized_track_component(title),
        normalized_track_component(artist),
        milliseconds(info.duration).unwrap_or_default(),
    ))
}

fn snapshot_from_info(timed: &TimedInfo) -> PlaybackSnapshot {
    let info = &timed.info;
    let duration_ms = milliseconds(info.duration);
    let elapsed = info.elapsed_time.map(|elapsed| {
        if info.is_playing == Some(true) {
            elapsed + timed.received_at.elapsed().as_secs_f64()
        } else {
            elapsed
        }
    });
    let position_ms = milliseconds(elapsed).map(|position| {
        duration_ms
            .map(|duration| position.min(duration))
            .unwrap_or(position)
    });
    PlaybackSnapshot {
        player: Some(PlayerKind::System),
        is_running: true,
        is_playing: info.is_playing.unwrap_or(false),
        track_id: system_track_id(info),
        title: info.title.clone(),
        artist: info.artist.clone(),
        album: info.album.clone(),
        source_app_name: info.bundle_name.clone(),
        source_app_bundle_id: info.bundle_id.clone(),
        duration_ms,
        position_ms,
        can_seek: duration_ms.is_some_and(|duration| duration > 0) && position_ms.is_some(),
        observed_at_ms: now_ms(),
        error_code: None,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> NowPlayingInfo {
        NowPlayingInfo {
            is_playing: Some(true),
            title: Some(" Test Song ".into()),
            artist: Some("Some Artist".into()),
            album: Some("Album".into()),
            album_cover: None,
            elapsed_time: Some(12.345),
            duration: Some(123.456),
            info_update_time: Some(SystemTime::now()),
            bundle_id: Some("com.example.Player".into()),
            bundle_name: Some("Example Player".into()),
            bundle_icon: None,
        }
    }

    #[test]
    fn converts_system_media_info_to_snapshot() {
        let snapshot = snapshot_from_info(&TimedInfo {
            info: info(),
            received_at: Instant::now(),
        });
        assert_eq!(snapshot.player, Some(PlayerKind::System));
        assert_eq!(snapshot.position_ms, Some(12_345));
        assert_eq!(snapshot.duration_ms, Some(123_456));
        assert_eq!(snapshot.source_app_name.as_deref(), Some("Example Player"));
        assert!(snapshot.can_seek);
    }

    #[test]
    fn seek_updates_the_cached_timeline_origin() {
        let latest = RwLock::new(Some(TimedInfo {
            info: info(),
            received_at: Instant::now(),
        }));
        update_elapsed_time(&latest, 45_678);
        let timed = latest.read().unwrap();
        let snapshot = snapshot_from_info(timed.as_ref().unwrap());
        assert!(snapshot
            .position_ms
            .is_some_and(|position| (45_678..45_700).contains(&position)));
    }

    #[test]
    fn initial_adapter_snapshot_uses_calculated_current_position() {
        let latest = RwLock::new(Some(TimedInfo {
            info: info(),
            received_at: Instant::now(),
        }));
        sync_elapsed_from_adapter(
            &latest,
            br#"{"title":" Test Song ","bundleIdentifier":"com.example.Player","elapsedTimeNow":56.86}"#,
        );
        let timed = latest.read().unwrap();
        assert_eq!(timed.as_ref().unwrap().info.elapsed_time, Some(56.86));
    }

    #[test]
    fn anchors_existing_playback_to_the_media_timestamp() {
        let mut current = info();
        current.info_update_time = Some(SystemTime::now() - Duration::from_secs(30));
        let snapshot = snapshot_from_info(&timed_info(current).unwrap());
        assert!(snapshot
            .position_ms
            .is_some_and(|position| (42_345..43_345).contains(&position)));
    }

    #[test]
    fn system_track_id_includes_source_application() {
        let first = system_track_id(&info()).unwrap();
        let mut other = info();
        other.bundle_id = Some("com.example.Other".into());
        assert_ne!(first, system_track_id(&other).unwrap());
    }

    #[test]
    fn rejects_invalid_times() {
        assert_eq!(milliseconds(Some(f64::NAN)), None);
        assert_eq!(milliseconds(Some(-1.0)), None);
    }

    #[test]
    fn advances_playing_position_from_monotonic_receive_time() {
        let snapshot = snapshot_from_info(&TimedInfo {
            info: info(),
            received_at: Instant::now() - Duration::from_millis(500),
        });
        assert!(snapshot
            .position_ms
            .is_some_and(|value| (12_845..=12_855).contains(&value)));
    }

    #[test]
    fn rejects_dependency_timestamp_overflow() {
        let mut invalid = info();
        invalid.elapsed_time = Some(978_307_212.0);
        assert!(!valid_elapsed_time(&invalid));
    }
}
