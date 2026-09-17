use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::RegisteredApplication;

use super::super::{
    PlaybackAction, PlaybackArtwork, PlaybackErrorCode, PlaybackSnapshot, PlayerKind,
};
use super::{adapter, artwork, metadata, targeted};
use tokio::sync::Notify;

struct TargetedSource {
    timed: metadata::TimedInfo,
    last_artwork_request: Option<Instant>,
}

pub struct SystemMediaService {
    player: OnceLock<Result<adapter::AdapterClient, String>>,
    artwork_cache: Mutex<Option<PlaybackArtwork>>,
    targeted_sources: Mutex<HashMap<String, TargetedSource>>,
    targeted_artwork_cache: Mutex<Option<PlaybackArtwork>>,
    control_gate: Mutex<()>,
}

impl Default for SystemMediaService {
    fn default() -> Self {
        Self {
            player: OnceLock::new(),
            artwork_cache: Mutex::new(None),
            targeted_sources: Mutex::new(HashMap::new()),
            targeted_artwork_cache: Mutex::new(None),
            control_gate: Mutex::new(()),
        }
    }
}

impl SystemMediaService {
    fn player(&self) -> Result<&adapter::AdapterClient, String> {
        self.player
            .get_or_init(adapter::initialize)
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(crate) fn playback_change_notifier(&self) -> Option<std::sync::Arc<Notify>> {
        self.player()
            .ok()
            .map(|player| player.runtime.playback_changed.clone())
    }

    pub(crate) fn set_refresh_interval(&self, interval: std::time::Duration) {
        if let Ok(player) = self.player() {
            player.runtime.set_refresh_interval(interval);
        }
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
        let latest = player
            .runtime
            .latest
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let version = player
            .runtime
            .state_version
            .load(std::sync::atomic::Ordering::SeqCst);
        let display_is_playing = player
            .runtime
            .presentation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .playing;
        let info = latest.clone();
        drop(latest);
        let Some(info) = info.as_ref() else {
            let snapshot = PlaybackSnapshot::unavailable_with_code(
                Some(PlayerKind::System),
                PlaybackErrorCode::Waiting,
                "未检测到系统正在播放的媒体".into(),
            );
            artwork::invalidate_cache(&self.artwork_cache, &snapshot);
            return snapshot;
        };
        let mut snapshot = metadata::snapshot_from_info(info);
        snapshot.display_is_playing = Some(display_is_playing);
        log::debug!(
            "系统媒体快照读取 version={} observed_at_ms={} received_age_us={}",
            version,
            snapshot.observed_at_ms,
            info.received_at.elapsed().as_micros()
        );
        artwork::invalidate_cache(&self.artwork_cache, &snapshot);
        snapshot
    }

    /// 每次只保留本轮查询到的来源；退出或查询失败的应用不可继续显示旧快照。
    pub(crate) fn allowlisted_snapshots(
        &self,
        applications: &[RegisteredApplication],
    ) -> Result<(Vec<(PlaybackSnapshot, Option<f64>)>, bool), String> {
        let result = targeted::query(applications).map_err(|error| {
            self.targeted_sources
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clear();
            self.targeted_artwork_cache
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .take();
            error
        })?;
        let mut sources = self
            .targeted_sources
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut previous = std::mem::take(&mut *sources);
        let mut snapshots = Vec::with_capacity(result.candidates.len());
        for candidate in result.candidates {
            let Some(bundle_id) = candidate.timed.info.bundle_id.clone() else {
                continue;
            };
            let mut source = TargetedSource {
                timed: candidate.timed,
                last_artwork_request: None,
            };
            if let Some(old) = previous.remove(&bundle_id) {
                if adapter::same_media_info(&old.timed.info, &source.timed.info) {
                    source.timed.info.album_cover = old.timed.info.album_cover;
                    source.last_artwork_request = old.last_artwork_request;
                }
            }
            snapshots.push((
                metadata::snapshot_from_info(&source.timed),
                candidate.activity_date,
            ));
            sources.insert(bundle_id, source);
        }
        if sources.is_empty() {
            self.targeted_artwork_cache
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take();
        }
        Ok((snapshots, result.had_error))
    }

    /// 只对最终选中的来源补全封面，避免每次轮询传输所有应用的大图。
    pub(crate) fn targeted_snapshot(&self, bundle_id: &str) -> Option<PlaybackSnapshot> {
        let expected = {
            let mut sources = self
                .targeted_sources
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let source = sources.get_mut(bundle_id)?;
            let should_request = source.timed.info.album_cover.is_none()
                && source
                    .last_artwork_request
                    .is_none_or(|at| at.elapsed() >= Duration::from_secs(5));
            if should_request {
                source.last_artwork_request = Some(Instant::now());
                Some(source.timed.info.clone())
            } else {
                None
            }
        };
        if let Some(expected) = expected {
            match targeted::artwork(bundle_id, &expected) {
                Ok(Some(image)) => {
                    let mut sources = self
                        .targeted_sources
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    if let Some(source) = sources.get_mut(bundle_id) {
                        if adapter::same_media_info(&source.timed.info, &expected) {
                            source.timed.info.album_cover = Some(image);
                        }
                    }
                }
                Ok(None) => {}
                Err(error) => log::debug!("指定播放器封面暂不可用：{error}"),
            }
        }
        let snapshot = self
            .targeted_sources
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(bundle_id)
            .map(|source| metadata::snapshot_from_info(&source.timed))?;
        artwork::invalidate_cache(&self.targeted_artwork_cache, &snapshot);
        Some(snapshot)
    }

    pub fn control_current(
        &self,
        expected: &PlaybackSnapshot,
        action: PlaybackAction,
    ) -> Result<(), String> {
        let _gate = self
            .control_gate
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let player = self.player()?;
        let current = adapter::current_snapshot(player)?;
        if !expected.same_system_media(&current) {
            return Err("所选歌曲已不是系统当前媒体".into());
        }
        let action = match action {
            PlaybackAction::TogglePlayPause => {
                if expected.is_playing != current.is_playing {
                    return Err("所选歌曲的播放状态已变化，请稍后重试".into());
                }
                if current.is_playing {
                    PlaybackAction::Pause
                } else {
                    PlaybackAction::Play
                }
            }
            action => action,
        };
        adapter::send_current_once(player, action)?;
        self.verify_current(player, |after| match action {
            PlaybackAction::Play => expected.same_system_media(after) && after.is_playing,
            PlaybackAction::Pause => expected.same_system_media(after) && !after.is_playing,
            PlaybackAction::Next => {
                after.is_running
                    && after.error_code.is_none()
                    && after.source_app_bundle_id == expected.source_app_bundle_id
                    && after.track_id.is_some()
                    && after.track_id != expected.track_id
            }
            PlaybackAction::Previous => {
                after.is_running
                    && after.error_code.is_none()
                    && after.source_app_bundle_id == expected.source_app_bundle_id
                    && (after.track_id.is_some() && after.track_id != expected.track_id
                        || expected.same_system_media(after)
                            && current.position_ms.is_some_and(|before| {
                                before > 3_000
                                    && after.position_ms.is_some_and(|position| {
                                        position.saturating_add(1_500) < before
                                    })
                            }))
            }
            PlaybackAction::TogglePlayPause => false,
        })
    }

    pub fn seek_current(
        &self,
        expected: &PlaybackSnapshot,
        position_ms: u64,
    ) -> Result<(), String> {
        let _gate = self
            .control_gate
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let player = self.player()?;
        let current = adapter::current_snapshot(player)?;
        if !expected.same_system_media(&current) {
            return Err("所选歌曲已不是系统当前媒体".into());
        }
        adapter::seek_current_once(player, position_ms)?;
        let sent_at = Instant::now();
        self.verify_current(player, |after| {
            expected.same_system_media(after)
                && after.position_ms.is_some_and(|actual| {
                    let allowance = 1_500
                        + if after.is_playing {
                            sent_at.elapsed().as_millis() as u64
                        } else {
                            0
                        };
                    actual >= position_ms.saturating_sub(1_500)
                        && actual <= position_ms.saturating_add(allowance)
                })
        })
    }

    fn verify_current(
        &self,
        player: &adapter::AdapterClient,
        accepted: impl Fn(&PlaybackSnapshot) -> bool,
    ) -> Result<(), String> {
        let mut last_error = None;
        for attempt in 0..5 {
            if attempt > 0 {
                thread::sleep(Duration::from_millis(150));
            }
            match adapter::current_snapshot(player) {
                Ok(snapshot) if accepted(&snapshot) => return Ok(()),
                Ok(_) => last_error = None,
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| "无法确认系统当前媒体已执行操作".into()))
    }

    pub fn artwork(&self, artwork_id: &str) -> Result<Option<PlaybackArtwork>, String> {
        if let Some(cached) = artwork::cached(&self.targeted_artwork_cache, artwork_id) {
            return Ok(Some(cached));
        }
        let targeted_image = self
            .targeted_sources
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .find_map(|source| {
                (metadata::snapshot_from_info(&source.timed)
                    .artwork_id
                    .as_deref()
                    == Some(artwork_id))
                .then(|| source.timed.info.album_cover.clone())
                .flatten()
            });
        if let Some(image) = targeted_image {
            let artwork = artwork::encode(artwork_id, &image)?;
            artwork::store(&self.targeted_artwork_cache, artwork.clone());
            return Ok(Some(artwork));
        }
        let current = self.snapshot();
        if current.artwork_id.as_deref() != Some(artwork_id) {
            return Ok(None);
        }

        if let Some(cached) = artwork::cached(&self.artwork_cache, artwork_id) {
            return Ok(Some(cached));
        }

        let player = self.player()?;
        let latest = player
            .runtime
            .latest
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let Some(timed) = latest.as_ref() else {
            return Ok(None);
        };
        if metadata::snapshot_from_info(timed).artwork_id.as_deref() != Some(artwork_id) {
            return Ok(None);
        }
        let image = timed.info.album_cover.clone();
        let Some(image) = image else {
            return Ok(None);
        };

        let artwork = artwork::encode(artwork_id, &image)?;
        artwork::store(&self.artwork_cache, artwork.clone());
        Ok(Some(artwork))
    }
}
