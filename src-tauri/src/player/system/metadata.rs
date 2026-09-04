use std::time::{Instant, SystemTime};

use media_remote::NowPlayingInfo;

use super::super::{normalized_track_component, now_ms, PlaybackSnapshot, PlayerKind};
use super::artwork::artwork_fingerprint;
use super::compat;

#[derive(Clone)]
pub(super) struct TimedInfo {
    pub(super) info: NowPlayingInfo,
    pub(super) received_at: Instant,
}

pub(super) fn milliseconds(seconds: Option<f64>) -> Option<u64> {
    seconds
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| (value * 1000.0).round() as u64)
}

pub(super) fn valid_elapsed_time(info: &NowPlayingInfo) -> bool {
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

pub(super) fn timed_info(mut info: NowPlayingInfo) -> Option<TimedInfo> {
    // 播放器退出时，系统适配器会发送 null；media-remote 会把它映射为全字段为空的结构体。
    // 这类事件表示媒体已清空，不能作为仍在运行的系统播放器缓存下来。
    if !has_media_identity(&info) {
        return None;
    }
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

fn has_media_identity(info: &NowPlayingInfo) -> bool {
    info.title
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || info
            .artist
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || info
            .album
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || info.album_cover.is_some()
        || info
            .bundle_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || info
            .bundle_name
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || info.bundle_icon.is_some()
}

fn normalized_system_metadata(info: &NowPlayingInfo) -> compat::TrackMetadata {
    compat::normalize(
        info.bundle_id.as_deref(),
        compat::TrackMetadata::new(info.title.clone(), info.artist.clone()),
    )
}

#[cfg(test)]
pub(super) fn system_track_id(info: &NowPlayingInfo) -> Option<String> {
    let metadata = normalized_system_metadata(info);
    system_track_id_from_metadata(info, &metadata)
}

fn system_track_id_from_metadata(
    info: &NowPlayingInfo,
    metadata: &compat::TrackMetadata,
) -> Option<String> {
    let title = metadata.title.as_deref()?;
    let artist = metadata.artist.as_deref().unwrap_or_default();
    Some(format!(
        "system:{}|{}|{}|{}",
        normalized_track_component(info.bundle_id.as_deref().unwrap_or_default()),
        normalized_track_component(title),
        normalized_track_component(artist),
        milliseconds(info.duration).unwrap_or_default(),
    ))
}

pub(super) fn snapshot_from_info(timed: &TimedInfo) -> PlaybackSnapshot {
    let info = &timed.info;
    let metadata = normalized_system_metadata(info);
    let track_id = system_track_id_from_metadata(info, &metadata);
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
    let artwork_id = info.album_cover.as_ref().and_then(|cover| {
        track_id
            .as_ref()
            .map(|track_id| format!("{track_id}|artwork:{:016x}", artwork_fingerprint(cover)))
    });
    PlaybackSnapshot {
        player: Some(PlayerKind::System),
        is_running: true,
        is_playing: info.is_playing.unwrap_or(false),
        track_id,
        title: metadata.title,
        artist: metadata.artist,
        album: info.album.clone(),
        source_app_name: info.bundle_name.clone(),
        source_app_bundle_id: info.bundle_id.clone(),
        artwork_id,
        duration_ms,
        position_ms,
        observed_at_ms: now_ms(),
        error_code: None,
        error: None,
    }
}
