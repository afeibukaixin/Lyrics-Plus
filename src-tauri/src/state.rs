use std::sync::{Arc, Mutex, RwLock};

use crate::config::ConfigStore;
use crate::lyrics::provider::ProviderRegistry;
use crate::lyrics::{LyricsRuntimeSnapshot, LyricsSearchSession};
use crate::player::{
    PlaybackSnapshot, PlaybackSpectrumService, PlayerKind, PlayerSelection, SystemMediaService,
};
use crate::runtime_model::{NotchLayoutMetrics, OverlaySettings};
use crate::storage::Storage;
use crate::{NotchVisibilityState, OverlayPlacementState};

/// 应用级共享状态。命令模块只消费它，不再拥有状态定义。
pub struct AppState {
    pub runtime_started: Mutex<bool>,
    pub selection: Arc<RwLock<PlayerSelection>>,
    pub auto_player: Arc<RwLock<Option<PlayerKind>>>,
    pub overlay_settings: Arc<RwLock<OverlaySettings>>,
    pub overlay_style: Arc<RwLock<crate::overlay_model::OverlayStyleSettings>>,
    pub overlay_monitor: Arc<RwLock<Option<String>>>,
    pub overlay_placement: Arc<Mutex<OverlayPlacementState>>,
    pub last_snapshot: Arc<RwLock<PlaybackSnapshot>>,
    pub spectrum: Arc<PlaybackSpectrumService>,
    pub pointer_monitor_wake: Arc<tokio::sync::Notify>,
    pub status_bar_wake: Arc<tokio::sync::Notify>,
    pub lyrics_runtime: Arc<RwLock<LyricsRuntimeSnapshot>>,
    pub lyrics_generation: Arc<std::sync::atomic::AtomicU64>,
    pub lyrics_search_session: Arc<Mutex<LyricsSearchSession>>,
    pub notch_layout_metrics: Arc<RwLock<NotchLayoutMetrics>>,
    pub(crate) notch_visibility: Arc<Mutex<NotchVisibilityState>>,
    pub storage: Arc<Storage>,
    pub config: Arc<ConfigStore>,
    pub providers: Arc<ProviderRegistry>,
    pub system_media: Arc<SystemMediaService>,
    pub http: reqwest::Client,
}
