use std::io::{Cursor, Read};
use std::path::Path;
use std::process::{Command, Stdio};
#[cfg(test)]
use std::sync::RwLock;
use std::sync::{atomic::Ordering, Arc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use media_remote::NowPlayingInfo;
use serde_json::Value;
use sha2::{Digest, Sha256};
use wait_timeout::ChildExt;

use super::super::{run_with_timeout, PlaybackAction};
use super::metadata::{milliseconds, timed_info, TimedInfo};
use super::runtime::AdapterRuntime;

const SEEK_CONFIRM_TIMEOUT: Duration = Duration::from_secs(2);
const SEEK_CONFIRM_TOLERANCE_MS: u64 = 1_500;
const MEDIA_CHANGED_ERROR: &str = "跳转期间系统媒体已切换";
const ADAPTER_ARCHIVE: &[u8] =
    include_bytes!("../../../resources/mediaremote-adapter/mediaremote-adapter-0.3.8.tar.gz");
const ADAPTER_ARCHIVE_SHA256: &str =
    "87b19e480a213ee591b7794942c2111f3ad58e7f0a1f18ec62c581d8e80e0a94";

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

pub(super) struct AdapterClient {
    pub(super) runtime: Arc<AdapterRuntime>,
    workers: Vec<JoinHandle<()>>,
}

impl Drop for AdapterClient {
    fn drop(&mut self) {
        self.runtime.stop();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

pub(super) fn initialize() -> Result<AdapterClient, String> {
    if format!("{:x}", Sha256::digest(ADAPTER_ARCHIVE)) != ADAPTER_ARCHIVE_SHA256 {
        return Err("系统媒体适配器资源校验失败".into());
    }
    let resources = tempfile::Builder::new()
        .prefix("lyrics-plus-mediaremote-adapter-0.3.8-")
        .tempdir()
        .map_err(|error| format!("无法创建系统媒体资源目录：{error}"))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(Cursor::new(ADAPTER_ARCHIVE)));
    archive
        .unpack(resources.path())
        .map_err(|error| format!("无法解压系统媒体适配器：{error}"))?;
    let runtime = AdapterRuntime::new(resources);
    let workers = runtime.start()?;
    Ok(AdapterClient { runtime, workers })
}

pub(super) fn control(client: &AdapterClient, action: PlaybackAction) -> Result<(), String> {
    let context = client.runtime.prepare_control(action)?;
    let command = match context.action {
        PlaybackAction::Play => media_remote::Command::Play,
        PlaybackAction::Pause => media_remote::Command::Pause,
        PlaybackAction::TogglePlayPause => media_remote::Command::TogglePlayPause,
        PlaybackAction::Previous => media_remote::Command::PreviousTrack,
        PlaybackAction::Next => media_remote::Command::NextTrack,
    };
    let accepted = media_remote::send_command(command);
    client.runtime.request_refresh();
    if accepted {
        client.runtime.acknowledge_control(&context);
        Ok(())
    } else {
        Err("系统媒体播放器未接受控制命令".into())
    }
}

pub(super) fn seek(client: &AdapterClient, position_ms: u64) -> Result<(), String> {
    let expected_info = client
        .runtime
        .latest
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .map(|timed| timed.info.clone())
        .ok_or_else(|| "当前没有可控制的系统媒体".to_string())?;
    let position_micros = position_ms.saturating_mul(1_000);
    let position = position_micros.to_string();
    let adapter_result = run_adapter(
        &client.runtime.script_path,
        &client.runtime.framework_path,
        ["seek", position.as_str()],
    );
    match adapter_result {
        Ok(output) if output.status.success() => {
            let confirmation = confirm_seek_position(client, &expected_info, position_ms);
            if confirmation.is_ok() && current_info_matches(client, &expected_info) {
                acknowledge_elapsed_sync(client);
                return Ok(());
            }
            if confirmation
                .as_ref()
                .err()
                .is_some_and(|error| error.as_str() == MEDIA_CHANGED_ERROR)
            {
                return Err(MEDIA_CHANGED_ERROR.into());
            }
            seek_with_fallback(
                client,
                &expected_info,
                position_ms,
                "系统媒体适配器未确认跳转位置",
            )
        }
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            seek_with_fallback(
                client,
                &expected_info,
                position_ms,
                if detail.is_empty() {
                    "系统媒体播放器未接受跳转命令"
                } else {
                    detail.as_str()
                },
            )
        }
        Err(error) => seek_with_fallback(client, &expected_info, position_ms, &error),
    }
}

/// get 与 stream 都携带完整快照；空结果清空媒体，异常载荷保持已有状态。
pub(super) fn payload_to_timed(payload: &Value) -> Result<Option<TimedInfo>, String> {
    if payload.is_null() || payload.as_object().is_some_and(|value| value.is_empty()) {
        return Ok(None);
    }
    if !payload.is_object()
        || payload
            .get("title")
            .and_then(Value::as_str)
            .is_none_or(|title| title.trim().is_empty())
        || payload.get("playing").and_then(Value::as_bool).is_none()
    {
        return Err("系统媒体快照缺少有效歌曲或播放状态".into());
    }
    Ok(timed_info(NowPlayingInfo {
        title: payload
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned),
        artist: payload
            .get("artist")
            .and_then(Value::as_str)
            .map(str::to_owned),
        album: payload
            .get("album")
            .and_then(Value::as_str)
            .map(str::to_owned),
        bundle_id: payload
            .get("bundleIdentifier")
            .and_then(Value::as_str)
            .map(str::to_owned),
        is_playing: payload.get("playing").and_then(Value::as_bool),
        duration: adapter_duration_seconds(payload),
        elapsed_time: adapter_elapsed_seconds(payload),
        // adapter_elapsed_seconds 已按系统时间戳补偿，后续仅按单调时钟推进。
        info_update_time: None,
        album_cover: None,
        bundle_name: None,
        bundle_icon: None,
    }))
}

// 保留已有时间换算测试使用的窄接口；生产路径统一经过 runtime.commit。
#[cfg(test)]
pub(super) fn sync_elapsed_from_adapter(latest: &RwLock<Option<TimedInfo>>, output: &[u8]) -> bool {
    let Ok(payload) = serde_json::from_slice::<Value>(output) else {
        return false;
    };
    let Some(position_seconds) = adapter_elapsed_seconds(&payload) else {
        return false;
    };
    let mut latest = latest.write().unwrap_or_else(|error| error.into_inner());
    let Some(timed) = latest.as_mut() else {
        return false;
    };
    let same_track = payload_matches_info(&payload, &timed.info);
    if same_track {
        if let Some(is_playing) = payload.get("playing").and_then(Value::as_bool) {
            timed.info.is_playing = Some(is_playing);
        }
        if let Some(duration_seconds) = adapter_duration_seconds(&payload) {
            timed.info.duration = Some(duration_seconds);
        }
        timed.info.elapsed_time = Some(position_seconds);
        timed.received_at = Instant::now();
    }
    same_track
}

fn seek_with_fallback(
    client: &AdapterClient,
    expected_info: &media_remote::NowPlayingInfo,
    position_ms: u64,
    adapter_error: &str,
) -> Result<(), String> {
    if !current_info_matches(client, expected_info) {
        return Err(MEDIA_CHANGED_ERROR.into());
    }
    match seek_with_system_controller(position_ms) {
        Ok(()) => {
            if !current_info_matches(client, expected_info)
                || !update_elapsed_time(client, expected_info, position_ms)
            {
                return Err(MEDIA_CHANGED_ERROR.into());
            }
            Ok(())
        }
        Err(controller_error) => {
            client.runtime.request_refresh();
            Err(format!(
                "系统媒体跳转失败（适配器：{adapter_error}；系统控制器：{controller_error}）"
            ))
        }
    }
}

fn confirm_seek_position(
    client: &AdapterClient,
    expected_info: &NowPlayingInfo,
    target_ms: u64,
) -> Result<u64, String> {
    let started_at = Instant::now();
    let mut last_error = None;
    loop {
        {
            let _query = client
                .runtime
                .query
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let version = {
                let latest = client
                    .runtime
                    .latest
                    .read()
                    .unwrap_or_else(|e| e.into_inner());
                if !latest
                    .as_ref()
                    .is_some_and(|timed| same_media_info(&timed.info, expected_info))
                {
                    return Err(MEDIA_CHANGED_ERROR.into());
                }
                client.runtime.state_version.load(Ordering::SeqCst)
            };
            match run_adapter(
                &client.runtime.script_path,
                &client.runtime.framework_path,
                ["get", "--no-artwork", "--now", "--micros"],
            ) {
                Ok(output) if output.status.success() && output.stderr.is_empty() => {
                    if let Ok(payload) = serde_json::from_slice::<Value>(&output.stdout) {
                        let same_track = payload_matches_info(&payload, expected_info);
                        let position = adapter_position_ms(&payload);
                        // 由统一入口在写锁内比较版本；绝不先检查版本、稍后再写旧状态。
                        if client.runtime.commit(&payload, Some(version))? {
                            if !same_track {
                                return Err(MEDIA_CHANGED_ERROR.into());
                            }
                            if let Some(position) = position {
                                let expected = if payload.get("playing").and_then(Value::as_bool)
                                    == Some(true)
                                {
                                    target_ms
                                        .saturating_add(started_at.elapsed().as_millis() as u64)
                                } else {
                                    target_ms
                                };
                                let expected = milliseconds(expected_info.duration)
                                    .map(|duration| expected.min(duration))
                                    .unwrap_or(expected);
                                if position.abs_diff(expected) <= SEEK_CONFIRM_TOLERANCE_MS {
                                    return Ok(position);
                                }
                                last_error =
                                    Some(format!("实际位置 {position}ms，目标 {expected}ms"));
                            }
                        } else {
                            last_error = Some("跳转确认遇到更新的媒体事件".into());
                        }
                    }
                }
                Ok(output) => {
                    last_error = Some(format!(
                        "适配器未返回有效进度：{}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                }
                Err(error) => last_error = Some(error),
            }
        }
        if started_at.elapsed() >= SEEK_CONFIRM_TIMEOUT {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(last_error.unwrap_or_else(|| "系统媒体播放器未返回跳转后进度".into()))
}

fn update_elapsed_time(
    client: &AdapterClient,
    expected_info: &NowPlayingInfo,
    position_ms: u64,
) -> bool {
    let mut latest = client
        .runtime
        .latest
        .write()
        .unwrap_or_else(|e| e.into_inner());
    let Some(timed) = latest.as_mut() else {
        return false;
    };
    if !same_media_info(&timed.info, expected_info) {
        return false;
    }
    timed.info.elapsed_time = Some(position_ms as f64 / 1000.0);
    timed.received_at = Instant::now();
    client.runtime.state_version.fetch_add(1, Ordering::SeqCst);
    drop(latest);
    acknowledge_elapsed_sync(client);
    true
}

fn acknowledge_elapsed_sync(client: &AdapterClient) {
    client.runtime.request_refresh();
    client.runtime.playback_changed.notify_one();
}

fn current_info_matches(
    client: &AdapterClient,
    expected_info: &media_remote::NowPlayingInfo,
) -> bool {
    client
        .runtime
        .latest
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .is_some_and(|timed| same_media_info(&timed.info, expected_info))
}

fn adapter_position_ms(payload: &Value) -> Option<u64> {
    milliseconds(adapter_elapsed_seconds(payload))
}

fn adapter_elapsed_seconds(payload: &Value) -> Option<f64> {
    let value = payload
        .get("elapsedTimeMicros")
        .and_then(microseconds_value)
        .map(|micros| {
            let elapsed = micros as f64 / 1_000_000.0;
            if payload
                .get("playing")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_micros() as u64)
                    .ok();
                if let Some(age_micros) = now
                    .zip(
                        payload
                            .get("timestampEpochMicros")
                            .and_then(microseconds_value),
                    )
                    .and_then(|(now, timestamp)| now.checked_sub(timestamp))
                {
                    elapsed + age_micros as f64 / 1_000_000.0
                } else {
                    elapsed
                }
            } else {
                elapsed
            }
        })
        .or_else(|| {
            payload
                .get("elapsedTimeNowMicros")
                .and_then(microseconds_value)
                .map(|micros| micros as f64 / 1_000_000.0)
        })
        .or_else(|| number_value(payload.get("elapsedTimeNow")));
    value.filter(|value| value.is_finite() && *value >= 0.0)
}

fn adapter_duration_seconds(payload: &Value) -> Option<f64> {
    let value = payload
        .get("durationMicros")
        .and_then(microseconds_value)
        .map(|micros| micros as f64 / 1_000_000.0)
        .or_else(|| number_value(payload.get("duration")));
    value.filter(|value| value.is_finite() && *value >= 0.0)
}

fn microseconds_value(value: &Value) -> Option<u64> {
    match value {
        Value::Number(value) => value.as_u64().or_else(|| {
            value
                .as_f64()
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(|value| value.round() as u64)
        }),
        Value::String(value) => value.parse::<u64>().ok(),
        _ => None,
    }
}

fn number_value(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(value)) => value.as_f64(),
        Some(Value::String(value)) => value.parse::<f64>().ok(),
        _ => None,
    }
}

fn payload_matches_info(payload: &Value, info: &media_remote::NowPlayingInfo) -> bool {
    matches_required_string(payload, "title", info.title.as_deref())
        && matches_optional_string(payload, "bundleIdentifier", info.bundle_id.as_deref())
        && matches_optional_string(payload, "artist", info.artist.as_deref())
        && payload_duration_matches_info(payload, info)
}

pub(super) fn same_media_info(
    first: &media_remote::NowPlayingInfo,
    second: &media_remote::NowPlayingInfo,
) -> bool {
    first.title.as_deref().map(str::trim) == second.title.as_deref().map(str::trim)
        && first.bundle_id.as_deref().map(str::trim) == second.bundle_id.as_deref().map(str::trim)
        && first.artist.as_deref().map(str::trim) == second.artist.as_deref().map(str::trim)
        && first.album.as_deref().map(str::trim) == second.album.as_deref().map(str::trim)
        && duration_values_match(milliseconds(first.duration), milliseconds(second.duration))
}

fn payload_duration_matches_info(payload: &Value, info: &media_remote::NowPlayingInfo) -> bool {
    duration_values_match(
        adapter_duration_seconds(payload).and_then(|value| milliseconds(Some(value))),
        milliseconds(info.duration),
    )
}

fn duration_values_match(first: Option<u64>, second: Option<u64>) -> bool {
    match (first, second) {
        (Some(first), Some(second)) => first.abs_diff(second) <= 100,
        _ => true,
    }
}

fn matches_optional_string(payload: &Value, key: &str, current: Option<&str>) -> bool {
    let Some(value) = payload.get(key).and_then(Value::as_str) else {
        return true;
    };
    if value.trim().is_empty() {
        return true;
    }
    current.is_some_and(|current| current.trim() == value.trim())
}

fn matches_required_string(payload: &Value, key: &str, current: Option<&str>) -> bool {
    payload
        .get(key)
        .and_then(Value::as_str)
        .zip(current)
        .is_some_and(|(value, current)| value.trim() == current.trim())
}

/// 同时排空 stdout/stderr，避免带封面的 get 写满管道后被误判为超时。
pub(super) fn run_adapter(
    script_path: &Path,
    framework_path: &Path,
    arguments: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<std::process::Output, String> {
    let mut child = Command::new("/usr/bin/perl")
        .arg(script_path)
        .arg(framework_path)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let mut stdout = child.stdout.take().ok_or("无法读取适配器输出")?;
    let mut stderr = child.stderr.take().ok_or("无法读取适配器错误")?;
    std::thread::scope(|scope| {
        let stdout = scope.spawn(move || {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).map(|_| bytes)
        });
        let stderr = scope.spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).map(|_| bytes)
        });
        let status = match child.wait_timeout(Duration::from_secs(3)) {
            Ok(Some(status)) => Ok(status),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                Err(match result {
                    Err(error) => error.to_string(),
                    _ => "系统媒体适配器查询超时".into(),
                })
            }
        };
        let stdout = stdout
            .join()
            .map_err(|_| "适配器输出读取线程失败")?
            .map_err(|error| error.to_string())?;
        let stderr = stderr
            .join()
            .map_err(|_| "适配器错误读取线程失败")?
            .map_err(|error| error.to_string())?;
        Ok(std::process::Output {
            status: status?,
            stdout,
            stderr,
        })
    })
}

fn seek_with_system_controller(position_ms: u64) -> Result<(), String> {
    let mut command = Command::new("/usr/bin/osascript");
    command
        .args(["-l", "JavaScript", "-e", SYSTEM_MEDIA_SEEK_SCRIPT, "--"])
        .arg(format!("{:.3}", position_ms as f64 / 1_000.0));
    let output = run_with_timeout(command, Duration::from_secs(3))?;
    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if output.status.success() && result == "ok" {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() { result } else { detail })
    }
}
