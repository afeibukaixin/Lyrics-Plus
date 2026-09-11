use std::sync::{Mutex, OnceLock};

use super::super::{
    PlaybackAction, PlaybackArtwork, PlaybackErrorCode, PlaybackSnapshot, PlayerKind,
};
use super::{adapter, artwork, metadata};
use tokio::sync::Notify;

pub struct SystemMediaService {
    player: OnceLock<Result<adapter::AdapterClient, String>>,
    artwork_cache: Mutex<Option<PlaybackArtwork>>,
}

impl Default for SystemMediaService {
    fn default() -> Self {
        Self {
            player: OnceLock::new(),
            artwork_cache: Mutex::new(None),
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

    pub fn control(&self, action: PlaybackAction) -> Result<(), String> {
        let player = self.player()?;
        adapter::control(player, action)
    }

    pub fn seek(&self, position_ms: u64) -> Result<(), String> {
        let player = self.player()?;
        let result = adapter::seek(player, position_ms);
        player.runtime.request_refresh();
        result
    }

    pub fn artwork(&self, artwork_id: &str) -> Result<Option<PlaybackArtwork>, String> {
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
