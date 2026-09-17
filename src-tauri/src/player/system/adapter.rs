use std::io::{Cursor, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
#[cfg(test)]
use std::sync::RwLock;
use std::thread::JoinHandle;
#[cfg(test)]
use std::time::Instant;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use media_remote::NowPlayingInfo;
use serde_json::Value;
use sha2::{Digest, Sha256};
use wait_timeout::ChildExt;

use super::super::{PlaybackAction, PlaybackSnapshot};
use super::metadata::{milliseconds, snapshot_from_info, timed_info, TimedInfo};
use super::runtime::AdapterRuntime;

const ADAPTER_ARCHIVE: &[u8] =
    include_bytes!("../../../resources/mediaremote-adapter/mediaremote-adapter-0.3.8.tar.gz");
const ADAPTER_ARCHIVE_SHA256: &str =
    "87b19e480a213ee591b7794942c2111f3ad58e7f0a1f18ec62c581d8e80e0a94";

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

/// 控制前同步读取全局当前媒体，不使用可能已被其他应用抢占的流缓存。
pub(super) fn current_snapshot(client: &AdapterClient) -> Result<PlaybackSnapshot, String> {
    let _query = client
        .runtime
        .query
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let output = run_adapter(
        &client.runtime.script_path,
        &client.runtime.framework_path,
        ["get", "--no-artwork", "--now", "--micros"],
    )?;
    command_output(&output, "读取系统当前媒体失败")?;
    let payload: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("解析系统当前媒体失败：{error}"))?;
    let timed =
        payload_to_timed(&payload)?.ok_or_else(|| "系统当前没有可控制的媒体".to_string())?;
    Ok(snapshot_from_info(&timed))
}

/// 适配器只控制系统当前媒体；调用方必须在发送前核对选中来源。
pub(super) fn send_current_once(
    client: &AdapterClient,
    action: PlaybackAction,
) -> Result<(), String> {
    let command = match action {
        PlaybackAction::Play => "0",
        PlaybackAction::Pause => "1",
        PlaybackAction::Next => "4",
        PlaybackAction::Previous => "5",
        PlaybackAction::TogglePlayPause => return Err("请使用明确的播放或暂停命令".into()),
    };
    let result = run_adapter(
        &client.runtime.script_path,
        &client.runtime.framework_path,
        ["send", command],
    );
    client.runtime.request_refresh();
    let result = result?;
    command_output(&result, "系统当前媒体未接受控制命令")
}

pub(super) fn seek_current_once(client: &AdapterClient, position_ms: u64) -> Result<(), String> {
    let position_micros = position_ms.saturating_mul(1_000).to_string();
    let result = run_adapter(
        &client.runtime.script_path,
        &client.runtime.framework_path,
        ["seek", position_micros.as_str()],
    );
    client.runtime.request_refresh();
    let result = result?;
    command_output(&result, "系统当前媒体未接受跳转命令")
}

fn command_output(output: &std::process::Output, fallback: &str) -> Result<(), String> {
    if output.status.success() && output.stderr.is_empty() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if detail.is_empty() {
        fallback.into()
    } else {
        detail
    })
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
fn matches_optional_string(payload: &Value, key: &str, current: Option<&str>) -> bool {
    let Some(value) = payload.get(key).and_then(Value::as_str) else {
        return true;
    };
    if value.trim().is_empty() {
        return true;
    }
    current.is_some_and(|current| current.trim() == value.trim())
}

#[cfg(test)]
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
