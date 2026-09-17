//! 通过 Apple 签名的 osascript 读取指定 MediaRemote 播放器。
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::DynamicImage;
use media_remote::NowPlayingInfo;
use serde::Deserialize;
use wait_timeout::ChildExt;

use crate::config::RegisteredApplication;

use super::adapter::same_media_info;
use super::metadata::{timed_info, TimedInfo};

const SCRIPT: &str = include_str!("../../../resources/system-media-targeted.js");
const QUERY_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_METADATA_BYTES: u64 = 256 * 1024;
const MAX_ARTWORK_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireSource {
    bundle_identifier: String,
    status: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<f64>,
    elapsed_time: Option<f64>,
    timestamp: Option<f64>,
    playing: Option<bool>,
    activity_date: Option<f64>,
    artwork_data: Option<String>,
}

pub(super) struct Candidate {
    pub(super) timed: TimedInfo,
    pub(super) activity_date: Option<f64>,
}

pub(super) struct QueryResult {
    pub(super) candidates: Vec<Candidate>,
    pub(super) had_error: bool,
}

fn invoke(args: &[&str], max_bytes: u64) -> Result<Vec<u8>, String> {
    let mut child = Command::new("/usr/bin/osascript")
        .args(["-l", "JavaScript", "-e", SCRIPT, "--"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("无法启动指定播放器查询：{error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取指定播放器查询结果".to_string())?;
    // 封面可能超过管道容量；读取必须和子进程执行并行。
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take(max_bytes + 1).read_to_end(&mut bytes);
        (result, bytes)
    });
    let status = match child.wait_timeout(QUERY_TIMEOUT) {
        Ok(Some(status)) => status,
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err("指定播放器响应超时".into());
        }
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(format!("等待指定播放器查询失败：{error}"));
        }
    };
    let (read_result, bytes) = reader
        .join()
        .map_err(|_| "读取指定播放器查询结果失败".to_string())?;
    read_result.map_err(|error| format!("读取指定播放器查询结果失败：{error}"))?;
    if bytes.len() as u64 > max_bytes {
        return Err("指定播放器查询结果过大".into());
    }
    if !status.success() {
        return Err("指定播放器 MediaRemote 接口不可用".into());
    }
    Ok(bytes)
}

fn seconds_to_time(seconds: Option<f64>) -> Option<SystemTime> {
    let seconds =
        seconds.filter(|value| value.is_finite() && (0.0..4_102_444_800.0).contains(value))?;
    UNIX_EPOCH.checked_add(Duration::from_secs_f64(seconds))
}

fn to_info(source: WireSource, name: Option<&str>) -> Option<NowPlayingInfo> {
    if source.status != "ok" {
        return None;
    }
    Some(NowPlayingInfo {
        title: source.title,
        artist: source.artist,
        album: source.album,
        bundle_id: Some(source.bundle_identifier),
        is_playing: source.playing,
        duration: source.duration,
        elapsed_time: source.elapsed_time,
        info_update_time: seconds_to_time(source.timestamp),
        album_cover: None,
        bundle_name: name.map(str::to_owned),
        bundle_icon: None,
    })
}

pub(super) fn query(applications: &[RegisteredApplication]) -> Result<QueryResult, String> {
    if applications.is_empty() {
        return Ok(QueryResult {
            candidates: Vec::new(),
            had_error: false,
        });
    }
    let mut args = Vec::with_capacity(applications.len() + 1);
    args.push("query");
    args.extend(
        applications
            .iter()
            .map(|application| application.bundle_id.as_str()),
    );
    let bytes = invoke(&args, MAX_METADATA_BYTES)?;
    let responses: Vec<WireSource> = serde_json::from_slice(&bytes)
        .map_err(|error| format!("解析指定播放器状态失败：{error}"))?;
    let mut candidates = Vec::new();
    let mut had_error = responses.len() != applications.len();
    for (application, response) in applications.iter().zip(responses) {
        if response.bundle_identifier != application.bundle_id {
            had_error = true;
            continue;
        }
        if response.status == "error" {
            had_error = true;
            continue;
        }
        let activity_date = response.activity_date;
        if let Some(info) = to_info(response, Some(&application.name)) {
            if let Some(timed) = timed_info(info) {
                candidates.push(Candidate {
                    timed,
                    activity_date,
                });
            }
        }
    }
    Ok(QueryResult {
        candidates,
        had_error,
    })
}

pub(super) fn artwork(
    bundle_id: &str,
    expected: &NowPlayingInfo,
) -> Result<Option<DynamicImage>, String> {
    let bytes = invoke(&["artwork", bundle_id], MAX_ARTWORK_BYTES)?;
    let responses: Vec<WireSource> = serde_json::from_slice(&bytes)
        .map_err(|error| format!("解析指定播放器封面失败：{error}"))?;
    let Some(response) = responses.into_iter().next() else {
        return Ok(None);
    };
    if response.status != "ok" || response.bundle_identifier != bundle_id {
        return Ok(None);
    }
    let artwork_data = response.artwork_data.clone();
    let Some(info) = to_info(response, None) else {
        return Ok(None);
    };
    if !same_media_info(&info, expected) {
        return Ok(None);
    }
    let Some(artwork_data) = artwork_data else {
        return Ok(None);
    };
    let bytes = BASE64
        .decode(artwork_data)
        .map_err(|error| format!("解码指定播放器封面失败：{error}"))?;
    image::load_from_memory(&bytes)
        .map(Some)
        .map_err(|error| format!("读取指定播放器封面失败：{error}"))
}
