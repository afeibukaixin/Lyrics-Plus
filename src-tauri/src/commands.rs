use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_opener::OpenerExt;

use crate::config::{
    normalize_player_follower_application, normalize_system_media_applications,
    validate_config_draft, AppConfig, ConfigDraftValidation, ConfigEditorData, ConfigStore,
    GlobalShortcutSettings, LanguagePreference, ListLyricsPreferences, LyricsBaseAppearance,
    LyricsModeStyleInheritance, NotchLyricsPreferences, OverlayAppearance, RegisteredApplication,
    StatusBarLyricsPreferences, SystemMediaFilterMode, ThemePreference,
};
use crate::language::UiLanguage;
use crate::lyrics::credentials::{MusixmatchTokenType, ProviderCredentialView};
use crate::lyrics::provider::{
    can_auto_apply, LyricsSearchInput, LyricsSearchResult, ProviderRegistry, ProviderSettings,
    ProviderSettingsView, ProviderStatus,
};
use crate::lyrics::LyricsDocument;
use crate::player::{
    run_with_timeout, PlaybackSnapshot, PlayerKind, PlayerSelection, SystemMediaService,
};
use crate::storage::library::LibraryScanStatus;
use crate::storage::{SaveKind, SaveRequest, Storage};

pub struct AppState {
    pub runtime_started: Mutex<bool>,
    pub selection: Arc<RwLock<PlayerSelection>>,
    pub auto_player: Arc<RwLock<Option<PlayerKind>>>,
    pub overlay_settings: Arc<RwLock<OverlaySettings>>,
    pub overlay_style: Arc<RwLock<OverlayStyleSettings>>,
    pub overlay_monitor: Arc<RwLock<Option<String>>>,
    pub overlay_placement: Arc<Mutex<crate::OverlayPlacementState>>,
    pub last_snapshot: Arc<RwLock<PlaybackSnapshot>>,
    pub lyrics_runtime: Arc<RwLock<LyricsRuntimeSnapshot>>,
    pub lyrics_generation: Arc<AtomicU64>,
    pub lyrics_search_session: Arc<Mutex<LyricsSearchSession>>,
    pub notch_layout_metrics: Arc<RwLock<NotchLayoutMetrics>>,
    pub(crate) notch_visibility: Arc<Mutex<crate::NotchVisibilityState>>,
    pub storage: Arc<Storage>,
    pub config: Arc<ConfigStore>,
    pub providers: Arc<ProviderRegistry>,
    pub system_media: Arc<SystemMediaService>,
    pub http: reqwest::Client,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalNoticeStatus {
    pub current_version: u16,
    pub accepted: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotchLayoutMetrics {
    pub has_notch: bool,
    pub top_inset: f64,
    pub center_gap_width: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsMonitor {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OverlaySettings {
    pub visible: bool,
    pub locked: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalShortcutStatus {
    pub toggle_overlay: bool,
    pub unlock_overlay: bool,
    pub reset_overlay: bool,
    pub toggle_status_bar_lyrics: bool,
    pub toggle_list_lyrics: bool,
    pub toggle_notch_lyrics: bool,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            visible: true,
            locked: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverlayBackground {
    #[default]
    Glass,
    Transparent,
    Solid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverlayBackgroundMode {
    #[default]
    Solid,
    Transparent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverlayLayout {
    #[default]
    Single,
    Double,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverlayOrientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverlayAlignment {
    #[default]
    Center,
    #[serde(alias = "left", alias = "right")]
    Distributed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LongTextMode {
    #[default]
    Shrink,
    Wrap,
    Marquee,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum KaraokeStyle {
    #[default]
    Sweep,
    // 兼容已持久化的旧选项；归一化后会保存为 Sweep。
    Fill,
    Bounce,
    Highlight,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SecondaryDisplayMode {
    #[default]
    Legacy,
    Next,
    Translation,
    Romanization,
    TranslationRomanization,
}

fn legacy_secondary_display() -> SecondaryDisplayMode {
    SecondaryDisplayMode::Legacy
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OverlayStyleSettings {
    pub font_family: String,
    pub font_size: u16,
    pub font_weight: u16,
    pub secondary_font_weight: u16,
    pub line_height: f64,
    pub active_color: String,
    pub inactive_color: String,
    pub opacity: f64,
    pub background_opacity: f64,
    pub background_blur: f64,
    pub background_radius: f64,
    pub background_padding_x: f64,
    pub background_padding_y: f64,
    pub background_mode: OverlayBackgroundMode,
    pub background: OverlayBackground,
    pub solid_color: String,
    pub layout: OverlayLayout,
    pub orientation: OverlayOrientation,
    pub alignment: OverlayAlignment,
    pub long_text: LongTextMode,
    #[serde(default = "legacy_secondary_display")]
    pub secondary_display: SecondaryDisplayMode,
    pub auto_center_with_translation_or_romanization: bool,
    #[serde(skip_serializing)]
    pub translation_enabled: bool,
    #[serde(skip_serializing)]
    pub romanization_enabled: bool,
    pub karaoke_style: KaraokeStyle,
    pub secondary_font_scale: f64,
    pub translation_font_scale: f64,
    pub romanization_font_scale: f64,
    pub translation_color: String,
    pub romanization_color: String,
    pub text_shadow_offset_x: f64,
    pub text_shadow_offset_y: f64,
    pub text_shadow_blur: f64,
    pub text_shadow_color: String,
    pub horizontal_max_width: Option<f64>,
    pub vertical_max_height: Option<f64>,
}

impl Default for OverlayStyleSettings {
    fn default() -> Self {
        Self {
            font_family: "Inter, \"SF Pro Text\", \"SF Pro Display\", -apple-system, BlinkMacSystemFont, \"Segoe UI\", \"PingFang SC\", \"Hiragino Sans GB\", \"Microsoft YaHei\", \"Noto Sans CJK SC\", \"Noto Sans SC\", Arial, sans-serif".into(),
            font_size: 36,
            font_weight: 800,
            secondary_font_weight: 500,
            line_height: 1.2,
            active_color: "#a3e635".into(),
            inactive_color: "#ecfccb".into(),
            opacity: 1.0,
            background_opacity: 0.6,
            background_blur: 18.0,
            background_radius: 18.0,
            background_padding_x: 26.0,
            background_padding_y: 22.0,
            background_mode: OverlayBackgroundMode::Solid,
            background: OverlayBackground::Glass,
            solid_color: "#171821".into(),
            layout: OverlayLayout::Single,
            orientation: OverlayOrientation::Horizontal,
            alignment: OverlayAlignment::Center,
            long_text: LongTextMode::Marquee,
            secondary_display: SecondaryDisplayMode::TranslationRomanization,
            auto_center_with_translation_or_romanization: false,
            translation_enabled: true,
            romanization_enabled: true,
            karaoke_style: KaraokeStyle::Sweep,
            secondary_font_scale: 1.0,
            translation_font_scale: 0.8,
            romanization_font_scale: 0.8,
            translation_color: "#d9f99d".into(),
            romanization_color: "#bef264".into(),
            text_shadow_offset_x: 0.0,
            text_shadow_offset_y: 1.0,
            text_shadow_blur: 4.0,
            text_shadow_color: "rgba(0, 0, 0, 0.55)".into(),
            horizontal_max_width: None,
            vertical_max_height: None,
        }
    }
}

impl OverlayStyleSettings {
    pub(crate) fn normalized(mut self) -> Self {
        if self.secondary_display == SecondaryDisplayMode::Legacy {
            self.secondary_display = if self.translation_enabled {
                SecondaryDisplayMode::Translation
            } else if self.romanization_enabled {
                SecondaryDisplayMode::Romanization
            } else {
                SecondaryDisplayMode::Next
            };
        }
        self.font_size = self.font_size.clamp(16, 72);
        self.font_weight = nearest_overlay_font_weight(self.font_weight);
        self.secondary_font_weight = nearest_overlay_font_weight(self.secondary_font_weight);
        self.line_height = self.line_height.clamp(0.8, 2.0);
        self.opacity = self.opacity.clamp(0.2, 1.0);
        self.background_opacity = self.background_opacity.clamp(0.0, 1.0);
        self.background_blur = self.background_blur.clamp(0.0, 40.0);
        self.background_radius = self.background_radius.clamp(0.0, 64.0);
        self.background_padding_x = self.background_padding_x.clamp(0.0, 64.0);
        self.background_padding_y = self.background_padding_y.clamp(0.0, 64.0);
        self.text_shadow_offset_x = self.text_shadow_offset_x.clamp(-20.0, 20.0);
        self.text_shadow_offset_y = self.text_shadow_offset_y.clamp(-20.0, 20.0);
        self.text_shadow_blur = self.text_shadow_blur.clamp(0.0, 40.0);
        if self.background == OverlayBackground::Transparent {
            self.background = OverlayBackground::Solid;
            self.background_mode = OverlayBackgroundMode::Transparent;
        }
        self.secondary_font_scale = self.secondary_font_scale.clamp(0.35, 1.0);
        self.translation_font_scale = self.translation_font_scale.clamp(0.35, 1.0);
        self.romanization_font_scale = self.romanization_font_scale.clamp(0.35, 1.0);
        self.horizontal_max_width = self
            .horizontal_max_width
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(320.0, 10_000.0));
        self.vertical_max_height = self
            .vertical_max_height
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(280.0, 10_000.0));
        if self.karaoke_style == KaraokeStyle::Fill {
            self.karaoke_style = KaraokeStyle::Sweep;
        }
        if self.active_color.trim().is_empty() {
            self.active_color = "#a3e635".into();
        }
        if self.font_family.trim().is_empty() {
            self.font_family = Self::default().font_family;
        } else {
            self.font_family = self.font_family.trim().to_string();
        }
        if self.text_shadow_color.trim().is_empty() {
            self.text_shadow_color = "rgba(0, 0, 0, 0.55)".into();
        }
        if self.inactive_color.trim().is_empty() {
            self.inactive_color = "#ecfccb".into();
        }
        if self.solid_color.trim().is_empty() {
            self.solid_color = "#171821".into();
        }
        if self.translation_color.trim().is_empty() {
            self.translation_color = "#d9f99d".into();
        }
        if self.romanization_color.trim().is_empty() {
            self.romanization_color = "#bef264".into();
        }
        self
    }
}

fn nearest_overlay_font_weight(value: u16) -> u16 {
    [400_u16, 500, 600, 700, 800]
        .into_iter()
        .min_by_key(|weight| weight.abs_diff(value))
        .unwrap_or(800)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLyricsInput {
    pub track_key: String,
    pub title: String,
    pub artist: String,
    pub source: String,
    pub lyrics: String,
    pub provider_id: Option<String>,
    pub provider_item_id: Option<String>,
    #[serde(default)]
    pub manual_selected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub auto_apply: bool,
    pub results: Vec<LyricsSearchResult>,
    pub provider_statuses: Vec<ProviderStatus>,
    pub error: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
struct LyricsSearchRequestKey {
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: Option<u64>,
}

impl LyricsSearchRequestKey {
    fn new(input: &LyricsSearchInput) -> Self {
        Self {
            title: input.title.trim().to_owned(),
            artist: input.artist.trim().to_owned(),
            album: input
                .album
                .as_deref()
                .map(str::trim)
                .filter(|album| !album.is_empty())
                .map(str::to_owned),
            duration_ms: input.duration_ms,
        }
    }
}

type LyricsSearchFlight = tokio::sync::OnceCell<Result<SearchResponse, String>>;
const LYRICS_SEARCH_INVALIDATED: &str = "当前歌词搜索已失效";

pub struct LyricsSearchSession {
    activation: u64,
    track_key: Option<String>,
    request_id: u64,
    request_key: Option<LyricsSearchRequestKey>,
    completed: Option<Result<SearchResponse, String>>,
    in_flight: Option<Arc<LyricsSearchFlight>>,
}

impl Default for LyricsSearchSession {
    fn default() -> Self {
        Self {
            activation: 0,
            track_key: None,
            request_id: 0,
            request_key: None,
            completed: None,
            in_flight: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LyricsRuntimeStatus {
    Idle,
    Loading,
    Ready,
    NotFound,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsRuntimeSnapshot {
    pub track_key: Option<String>,
    pub document: Option<LyricsDocument>,
    pub status: LyricsRuntimeStatus,
    pub error: Option<String>,
}

impl Default for LyricsRuntimeSnapshot {
    fn default() -> Self {
        Self {
            track_key: None,
            document: None,
            status: LyricsRuntimeStatus::Idle,
            error: None,
        }
    }
}

fn player_key(player: PlayerKind) -> &'static str {
    match player {
        PlayerKind::AppleMusic => "apple_music",
        PlayerKind::Spotify => "spotify",
        PlayerKind::System => "system",
    }
}

pub(crate) fn playback_track_key(snapshot: &PlaybackSnapshot) -> Option<String> {
    let player = snapshot.player?;
    let title = snapshot.title.as_deref()?.trim();
    let artist = snapshot.artist.as_deref()?.trim();
    if title.is_empty() || artist.is_empty() {
        return None;
    }
    if let Some(track_id) = snapshot
        .track_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        return Some(format!("{}:{}", player_key(player), track_id));
    }
    let fallback = format!("{title}|{artist}|{}", snapshot.duration_ms.unwrap_or(0))
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    Some(format!("{}:fallback:{fallback}", player_key(player)))
}

fn publish_lyrics_runtime(app: &tauri::AppHandle, snapshot: LyricsRuntimeSnapshot) {
    if let Some(state) = app.try_state::<AppState>() {
        *state
            .lyrics_runtime
            .write()
            .unwrap_or_else(|error| error.into_inner()) = snapshot.clone();
    }
    let _ = app.emit("lyrics://runtime-changed", &snapshot);
    crate::sync_lyrics_surfaces(app);
}

fn reset_lyrics_search_session(state: &AppState, track_key: Option<String>) {
    let mut session = state
        .lyrics_search_session
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    session.activation = session.activation.wrapping_add(1);
    session.track_key = track_key;
    session.request_id = 0;
    session.request_key = None;
    session.completed = None;
    session.in_flight = None;
}

fn invalidate_lyrics_search_session(state: &AppState) {
    let track_key = state
        .lyrics_search_session
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .track_key
        .clone();
    reset_lyrics_search_session(state, track_key);
}

async fn perform_lyrics_search(
    state: &AppState,
    input: &LyricsSearchInput,
) -> Result<SearchResponse, String> {
    let mut outcome = state.providers.search(&state.http, input).await?;
    let secondary_display = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .secondary_display;
    if outcome.prefer_capabilities {
        prefer_candidate_capabilities(&mut outcome.results, secondary_display);
    }
    Ok(SearchResponse {
        auto_apply: can_auto_apply(&outcome.results, outcome.auto_apply_threshold),
        results: outcome.results,
        provider_statuses: outcome.statuses,
        error: outcome.error,
    })
}

async fn search_lyrics_for_session(
    state: &AppState,
    track_key: &str,
    input: LyricsSearchInput,
    force: bool,
) -> Result<SearchResponse, String> {
    if input.title.trim().is_empty() || input.artist.trim().is_empty() {
        return Err("搜索歌词需要歌曲名和歌手".into());
    }

    let request_key = LyricsSearchRequestKey::new(&input);
    let (activation, request_id, flight) = {
        let mut session = state
            .lyrics_search_session
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if session.track_key.as_deref() != Some(track_key) {
            return Err("当前歌曲已发生变化".into());
        }
        if !force {
            if let Some(completed) = &session.completed {
                return completed.clone();
            }
            if let Some(flight) = &session.in_flight {
                (session.activation, session.request_id, flight.clone())
            } else {
                session.request_id = session.request_id.wrapping_add(1);
                session.request_key = Some(request_key);
                let flight = Arc::new(LyricsSearchFlight::new());
                session.in_flight = Some(flight.clone());
                (session.activation, session.request_id, flight)
            }
        } else if session.request_key.as_ref() == Some(&request_key) {
            if let Some(flight) = &session.in_flight {
                (session.activation, session.request_id, flight.clone())
            } else {
                session.request_id = session.request_id.wrapping_add(1);
                session.completed = None;
                let flight = Arc::new(LyricsSearchFlight::new());
                session.in_flight = Some(flight.clone());
                (session.activation, session.request_id, flight)
            }
        } else {
            session.request_id = session.request_id.wrapping_add(1);
            session.request_key = Some(request_key);
            session.completed = None;
            let flight = Arc::new(LyricsSearchFlight::new());
            session.in_flight = Some(flight.clone());
            (session.activation, session.request_id, flight)
        }
    };

    let result = flight
        .get_or_init(|| perform_lyrics_search(state, &input))
        .await
        .clone();
    let mut session = state
        .lyrics_search_session
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if session.activation != activation || session.request_id != request_id {
        return Err(LYRICS_SEARCH_INVALIDATED.into());
    }
    session.completed = Some(result.clone());
    session.in_flight = None;
    result
}

pub(crate) fn set_runtime_document_if_active(
    app: &tauri::AppHandle,
    track_key: &str,
    document: Option<LyricsDocument>,
) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let active = state
        .lyrics_runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .track_key
        .as_deref()
        == Some(track_key);
    if !active {
        return;
    }
    state.lyrics_generation.fetch_add(1, Ordering::SeqCst);
    publish_lyrics_runtime(
        app,
        LyricsRuntimeSnapshot {
            track_key: Some(track_key.to_owned()),
            status: if document.is_some() {
                LyricsRuntimeStatus::Ready
            } else {
                LyricsRuntimeStatus::NotFound
            },
            document,
            error: None,
        },
    );
}

fn reload_active_lyrics_runtime(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let runtime = state
        .lyrics_runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let track_key = runtime.track_key;
    let Some(track_key) = track_key else {
        return;
    };
    match state.storage.load(&track_key) {
        Ok(Some(document)) => set_runtime_document_if_active(app, &track_key, Some(document)),
        Ok(None) if runtime.status != LyricsRuntimeStatus::Loading => {
            set_runtime_document_if_active(app, &track_key, None);
        }
        Ok(None) => {}
        Err(error) => log::warn!("Failed to refresh the active lyrics runtime: {error}"),
    }
}

pub(crate) fn sync_lyrics_runtime(app: &tauri::AppHandle, playback: &PlaybackSnapshot) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let next_key = playback_track_key(playback);
    let current_key = state
        .lyrics_runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .track_key
        .clone();
    if current_key == next_key {
        crate::sync_lyrics_surfaces(app);
        return;
    }

    let generation = state.lyrics_generation.fetch_add(1, Ordering::SeqCst) + 1;
    reset_lyrics_search_session(&state, next_key.clone());
    let Some(track_key) = next_key else {
        publish_lyrics_runtime(app, LyricsRuntimeSnapshot::default());
        return;
    };
    publish_lyrics_runtime(
        app,
        LyricsRuntimeSnapshot {
            track_key: Some(track_key.clone()),
            document: None,
            status: LyricsRuntimeStatus::Loading,
            error: None,
        },
    );

    let playback = playback.clone();
    let worker_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = worker_app.state::<AppState>();
        let current = || state.lyrics_generation.load(Ordering::SeqCst) == generation;
        match state.storage.load(&track_key) {
            Ok(Some(document)) => {
                if current() {
                    publish_lyrics_runtime(
                        &worker_app,
                        LyricsRuntimeSnapshot {
                            track_key: Some(track_key),
                            document: Some(document),
                            status: LyricsRuntimeStatus::Ready,
                            error: None,
                        },
                    );
                }
                return;
            }
            Err(error) => {
                if current() {
                    publish_lyrics_runtime(
                        &worker_app,
                        LyricsRuntimeSnapshot {
                            track_key: Some(track_key),
                            document: None,
                            status: LyricsRuntimeStatus::Error,
                            error: Some(error),
                        },
                    );
                }
                return;
            }
            Ok(None) => {}
        }

        let (Some(title), Some(artist)) = (playback.title.clone(), playback.artist.clone()) else {
            if current() {
                publish_lyrics_runtime(
                    &worker_app,
                    LyricsRuntimeSnapshot {
                        track_key: Some(track_key),
                        document: None,
                        status: LyricsRuntimeStatus::NotFound,
                        error: None,
                    },
                );
            }
            return;
        };

        let input = LyricsSearchInput {
            title: title.clone(),
            artist: artist.clone(),
            album: playback.album.clone(),
            duration_ms: playback.duration_ms,
            scoring: Arc::default(),
        };
        match search_lyrics_for_session(&state, &track_key, input, false).await {
            Ok(response) => {
                if !current() {
                    return;
                }
                if let Some(error) = response.error {
                    publish_lyrics_runtime(
                        &worker_app,
                        LyricsRuntimeSnapshot {
                            track_key: Some(track_key),
                            document: None,
                            status: LyricsRuntimeStatus::Error,
                            error: Some(error),
                        },
                    );
                    return;
                }
                let document = if response.auto_apply {
                    response.results.first().and_then(|result| {
                        state
                            .storage
                            .save(SaveRequest {
                                track_key: &track_key,
                                title: &title,
                                artist: &artist,
                                source: &result.source,
                                raw: &result.lyrics,
                                provider_id: Some(&result.provider_id),
                                provider_item_id: Some(&result.id),
                                kind: SaveKind::Automatic,
                            })
                            .ok()
                    })
                } else {
                    None
                };
                if document.is_some() {
                    let _ = worker_app.emit("lyrics://changed", &track_key);
                }
                if current() {
                    publish_lyrics_runtime(
                        &worker_app,
                        LyricsRuntimeSnapshot {
                            track_key: Some(track_key),
                            status: if document.is_some() {
                                LyricsRuntimeStatus::Ready
                            } else {
                                LyricsRuntimeStatus::NotFound
                            },
                            document,
                            error: None,
                        },
                    );
                }
            }
            Err(error) if current() && error == LYRICS_SEARCH_INVALIDATED => publish_lyrics_runtime(
                &worker_app,
                LyricsRuntimeSnapshot {
                    track_key: Some(track_key),
                    document: None,
                    status: LyricsRuntimeStatus::NotFound,
                    error: None,
                },
            ),
            Err(error) if current() => publish_lyrics_runtime(
                &worker_app,
                LyricsRuntimeSnapshot {
                    track_key: Some(track_key),
                    document: None,
                    status: LyricsRuntimeStatus::Error,
                    error: Some(error),
                },
            ),
            Err(_) => {}
        }
    });
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsSection {
    Style,
    Display,
    Lyrics,
    Player,
    Application,
    About,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LyricsStyleMode {
    Desktop,
    StatusBar,
    ListWindow,
    Notch,
}

fn sync_desktop_style_from_config(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
    config: &AppConfig,
) -> Result<OverlayStyleSettings, String> {
    let geometry = {
        let current = state
            .overlay_style
            .read()
            .unwrap_or_else(|error| error.into_inner());
        (current.horizontal_max_width, current.vertical_max_height)
    };
    let mut style = config.overlay.appearance.clone().into_style();
    style.horizontal_max_width = geometry.0;
    style.vertical_max_height = geometry.1;
    *state
        .overlay_style
        .write()
        .unwrap_or_else(|error| error.into_inner()) = style.clone();
    if let Some(window) = app.get_webview_window("lyrics-overlay") {
        crate::sync_overlay_vibrancy(&window, &style);
    }
    app.emit("overlay://style", &style)
        .map_err(|error| error.to_string())?;
    Ok(style)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsResetResponse {
    pub overlay_settings: OverlaySettings,
    pub overlay_style: OverlayStyleSettings,
    pub provider_view: ProviderSettingsView,
    pub player_selection: PlayerSelection,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigExport {
    pub file_name: String,
    pub raw: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialUpdate {
    pub credentials: ProviderCredentialView,
    pub provider_view: ProviderSettingsView,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverlayResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayResizeBounds {
    pub width: f64,
    pub height: f64,
}

#[tauri::command]
pub fn get_playback_snapshot(state: State<'_, AppState>) -> PlaybackSnapshot {
    state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

#[tauri::command]
pub fn get_player_selection(state: State<'_, AppState>) -> PlayerSelection {
    *state
        .selection
        .read()
        .unwrap_or_else(|error| error.into_inner())
}

pub fn update_player_selection(
    app: &tauri::AppHandle,
    selection: PlayerSelection,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let saved = state
        .config
        .update(|config| config.app.player_selection = selection)?;
    *state
        .selection
        .write()
        .unwrap_or_else(|error| error.into_inner()) = selection;
    *state
        .auto_player
        .write()
        .unwrap_or_else(|error| error.into_inner()) = None;
    app.emit("player://selection", selection)
        .map_err(|error| error.to_string())?;
    app.emit("config://changed", &saved)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_player_selection(
    app: tauri::AppHandle,
    selection: PlayerSelection,
) -> Result<(), String> {
    update_player_selection(&app, selection)
}

#[tauri::command]
pub async fn search_lyrics(
    track_key: String,
    input: LyricsSearchInput,
    force: bool,
    state: State<'_, AppState>,
) -> Result<SearchResponse, String> {
    search_lyrics_for_session(&state, &track_key, input, force).await
}

fn prefer_candidate_capabilities(
    results: &mut [LyricsSearchResult],
    secondary_display: SecondaryDisplayMode,
) {
    if results.len() < 2 {
        return;
    }
    let capability_rank = |result: &LyricsSearchResult| {
        let secondary_rank = match secondary_display {
            SecondaryDisplayMode::Translation => u8::from(!result.has_translation),
            SecondaryDisplayMode::Romanization => u8::from(!result.has_romanization),
            SecondaryDisplayMode::TranslationRomanization => {
                if result.has_translation && result.has_romanization {
                    0
                } else if result.has_translation {
                    1
                } else if result.has_romanization {
                    2
                } else {
                    3
                }
            }
            SecondaryDisplayMode::Legacy | SecondaryDisplayMode::Next => 0,
        };
        (u8::from(!result.has_word_timing), secondary_rank)
    };

    let mut ranked = results.iter().cloned().enumerate().collect::<Vec<_>>();

    let mut band_start = 0;
    while band_start < ranked.len() {
        let band_score = ranked[band_start].1.score;
        let band_len = ranked[band_start..]
            .iter()
            .take_while(|(_, result)| (band_score - result.score).abs() <= 0.04 + f64::EPSILON)
            .count();
        let band_end = band_start + band_len;
        ranked[band_start..band_end].sort_by(|(left_index, left), (right_index, right)| {
            capability_rank(left)
                .cmp(&capability_rank(right))
                .then_with(|| left_index.cmp(right_index))
        });
        band_start = band_end;
    }

    for (target, (_, result)) in results.iter_mut().zip(ranked) {
        *target = result;
    }
}

#[tauri::command]
pub fn get_provider_settings(state: State<'_, AppState>) -> ProviderSettingsView {
    state.providers.settings_view()
}

#[tauri::command]
pub fn get_provider_credentials(state: State<'_, AppState>) -> ProviderCredentialView {
    state.providers.credential_view()
}

#[tauri::command]
pub fn set_musixmatch_token(
    token_type: MusixmatchTokenType,
    token: String,
    state: State<'_, AppState>,
) -> Result<ProviderCredentialUpdate, String> {
    let (credentials, provider_view) = state.providers.set_musixmatch_token(token_type, token)?;
    state
        .config
        .update(|config| config.lyrics.providers = provider_view.settings.clone())?;
    invalidate_lyrics_search_session(&state);
    Ok(ProviderCredentialUpdate {
        credentials,
        provider_view,
    })
}

#[tauri::command]
pub fn clear_musixmatch_token(
    state: State<'_, AppState>,
) -> Result<ProviderCredentialUpdate, String> {
    let (credentials, provider_view) = state.providers.clear_musixmatch_token()?;
    state
        .config
        .update(|config| config.lyrics.providers = provider_view.settings.clone())?;
    invalidate_lyrics_search_session(&state);
    Ok(ProviderCredentialUpdate {
        credentials,
        provider_view,
    })
}

#[tauri::command]
pub fn set_provider_settings(
    settings: ProviderSettings,
    state: State<'_, AppState>,
) -> Result<ProviderSettingsView, String> {
    let view = state.providers.set_settings(settings)?;
    state
        .config
        .update(|config| config.lyrics.providers = view.settings.clone())?;
    invalidate_lyrics_search_session(&state);
    Ok(view)
}

#[tauri::command]
pub async fn test_provider(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<ProviderStatus, String> {
    state
        .providers
        .test_provider(&state.http, &provider_id)
        .await
}

#[tauri::command]
pub fn get_cached_lyrics(
    track_key: String,
    state: State<'_, AppState>,
) -> Result<Option<LyricsDocument>, String> {
    state.storage.load(&track_key)
}

#[tauri::command]
pub fn get_lyrics_runtime_snapshot(state: State<'_, AppState>) -> LyricsRuntimeSnapshot {
    state
        .lyrics_runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

#[tauri::command]
pub fn get_notch_layout_metrics(state: State<'_, AppState>) -> NotchLayoutMetrics {
    state
        .notch_layout_metrics
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

#[tauri::command]
pub fn get_lyrics_monitors(app: tauri::AppHandle) -> Result<Vec<LyricsMonitor>, String> {
    let primary_id = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| crate::notch_monitor_id(&monitor));
    app.available_monitors()
        .map_err(|error| error.to_string())
        .map(|monitors| {
            monitors
                .into_iter()
                .map(|monitor| {
                    let id = crate::notch_monitor_id(&monitor);
                    let size = monitor.size();
                    LyricsMonitor {
                        is_primary: primary_id.as_deref() == Some(id.as_str()),
                        id,
                        name: monitor.name().cloned().unwrap_or_default(),
                        width: size.width,
                        height: size.height,
                    }
                })
                .collect()
        })
}

fn save_and_emit(
    app: &tauri::AppHandle,
    state: &AppState,
    input: SaveLyricsInput,
    kind: SaveKind,
) -> Result<LyricsDocument, String> {
    let document = state.storage.save(SaveRequest {
        track_key: &input.track_key,
        title: &input.title,
        artist: &input.artist,
        source: &input.source,
        raw: &input.lyrics,
        provider_id: input.provider_id.as_deref(),
        provider_item_id: input.provider_item_id.as_deref(),
        kind,
    })?;
    app.emit("lyrics://changed", &input.track_key)
        .map_err(|error| error.to_string())?;
    set_runtime_document_if_active(app, &input.track_key, Some(document.clone()));
    Ok(document)
}

#[tauri::command]
pub fn save_lyrics(
    app: tauri::AppHandle,
    input: SaveLyricsInput,
    state: State<'_, AppState>,
) -> Result<LyricsDocument, String> {
    let kind = if input.manual_selected {
        SaveKind::ManualSelection
    } else {
        SaveKind::Automatic
    };
    save_and_emit(&app, &state, input, kind)
}

#[tauri::command]
pub fn import_lyrics(
    app: tauri::AppHandle,
    input: SaveLyricsInput,
    state: State<'_, AppState>,
) -> Result<LyricsDocument, String> {
    save_and_emit(&app, &state, input, SaveKind::Import)
}

#[tauri::command]
pub fn set_lyrics_offset(
    app: tauri::AppHandle,
    track_key: String,
    offset_ms: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.storage.set_offset(&track_key, offset_ms)?;
    app.emit("lyrics://changed", &track_key)
        .map_err(|error| error.to_string())?;
    let document = state.storage.load(&track_key)?;
    set_runtime_document_if_active(&app, &track_key, document);
    Ok(())
}

#[tauri::command]
pub fn remove_lyrics_association(
    app: tauri::AppHandle,
    track_key: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.storage.remove(&track_key)?;
    app.emit("lyrics://changed", &track_key)
        .map_err(|error| error.to_string())?;
    set_runtime_document_if_active(&app, &track_key, None);
    Ok(())
}

pub(crate) fn start_library_scan(app: &tauri::AppHandle) -> LibraryScanStatus {
    let storage = app.state::<AppState>().storage.clone();
    let status = storage.begin_library_scan();
    let scan_id = status.scan_id;
    let worker_app = app.clone();
    let _ = app.emit("lyrics://library-scan-progress", &status);
    tauri::async_runtime::spawn_blocking(move || {
        let result = storage.run_library_scan(scan_id, |status| {
            let _ = worker_app.emit("lyrics://library-scan-progress", status);
        });
        match result {
            Ok(true) => {
                reload_active_lyrics_runtime(&worker_app);
                let _ = worker_app.emit("lyrics://library-changed", ());
            }
            Ok(false) => {}
            Err(error) => {
                log::warn!("Failed to scan the lyrics library: {error}");
                if let Some(status) = storage.fail_library_scan(scan_id, error) {
                    let _ = worker_app.emit("lyrics://library-scan-progress", status);
                }
            }
        }
    });
    status
}

#[tauri::command]
pub fn get_library_scan_status(state: State<'_, AppState>) -> LibraryScanStatus {
    state.storage.library_scan_status()
}

#[tauri::command]
pub fn rescan_lyrics_library(app: tauri::AppHandle) -> LibraryScanStatus {
    start_library_scan(&app)
}

#[tauri::command]
pub fn set_lyrics_directory(
    app: tauri::AppHandle,
    path: String,
    state: State<'_, AppState>,
) -> Result<LibraryScanStatus, String> {
    state.storage.set_library_directory(&path)?;
    Ok(start_library_scan(&app))
}

#[tauri::command]
pub fn open_lyrics_directory(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    app.opener()
        .open_path(
            state.storage.library_directory().to_string_lossy(),
            None::<&str>,
        )
        .map_err(|error| format!("打开歌词目录失败：{error}"))
}

pub fn update_overlay_visible(app: &tauri::AppHandle, visible: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let config = state
        .config
        .update(|config| config.overlay.visible = visible)?;
    state
        .overlay_settings
        .write()
        .unwrap_or_else(|error| error.into_inner())
        .visible = visible;
    crate::reconcile_overlay_visibility(app)?;
    crate::sync_tray_overlay_checked(app, visible);
    app.emit("overlay://settings", get_overlay_settings_inner(&state))
        .map_err(|error| error.to_string())?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_overlay_visible(app: tauri::AppHandle, visible: bool) -> Result<(), String> {
    update_overlay_visible(&app, visible)
}

fn get_overlay_settings_inner(state: &AppState) -> OverlaySettings {
    state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

#[tauri::command]
pub fn get_overlay_settings(state: State<'_, AppState>) -> OverlaySettings {
    get_overlay_settings_inner(&state)
}

pub fn update_overlay_locked(app: &tauri::AppHandle, locked: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("lyrics-overlay")
        .ok_or_else(|| "歌词浮窗不存在".to_string())?;
    let state = app.state::<AppState>();
    let previous_settings = {
        let mut settings = state
            .overlay_settings
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let previous = settings.clone();
        settings.locked = locked;
        previous
    };
    let update_result = (|| {
        if locked {
            let current_size = window.outer_size().map_err(|error| error.to_string())?;
            let scale = window.scale_factor().map_err(|error| error.to_string())?;
            let scale = if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            };
            let mut style = state
                .overlay_style
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            match style.orientation {
                OverlayOrientation::Horizontal => {
                    style.horizontal_max_width = Some(current_size.width as f64 / scale);
                }
                OverlayOrientation::Vertical => {
                    style.vertical_max_height = Some(current_size.height as f64 / scale);
                }
            }
            let style = style.normalized();
            *state
                .overlay_style
                .write()
                .unwrap_or_else(|error| error.into_inner()) = style.clone();
            persist_overlay_style_for_current_monitor(app, &state, &style)?;
        }
        window
            .set_ignore_cursor_events(locked)
            .map_err(|error| error.to_string())?;
        let _ = window.set_focusable(!locked);
        if !locked {
            crate::refresh_overlay_mouse_tracking(&window);
        }
        let _ = window.set_resizable(false);
        state
            .config
            .update(|config| config.overlay.locked = locked)?;
        crate::sync_unlock_handle(app);
        app.emit("overlay://settings", get_overlay_settings_inner(&state))
            .map_err(|error| error.to_string())
    })();
    if let Err(error) = update_result {
        *state
            .overlay_settings
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = previous_settings;
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub fn set_overlay_locked(app: tauri::AppHandle, locked: bool) -> Result<(), String> {
    update_overlay_locked(&app, locked)
}

#[tauri::command]
pub fn get_overlay_style(state: State<'_, AppState>) -> OverlayStyleSettings {
    state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

#[tauri::command]
pub fn get_overlay_toolbar_placement(state: State<'_, AppState>) -> crate::ToolbarPlacement {
    state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .toolbar_placement
}

fn persist_overlay_style_for_current_monitor(
    app: &tauri::AppHandle,
    state: &AppState,
    style: &OverlayStyleSettings,
) -> Result<(), String> {
    let monitor_id = app
        .get_webview_window("lyrics-overlay")
        .and_then(|window| window.current_monitor().ok().flatten())
        .map(|monitor| crate::monitor_id(&monitor));
    *state
        .overlay_monitor
        .write()
        .unwrap_or_else(|error| error.into_inner()) = monitor_id.clone();
    let key = monitor_id
        .map(|id| format!("overlay.geometry.{id}"))
        .unwrap_or_else(|| "overlay.geometry.default".into());
    let geometry = crate::StoredOverlayGeometry {
        horizontal_max_width: style.horizontal_max_width,
        vertical_max_height: style.vertical_max_height,
    };
    let raw =
        serde_json::to_string(&geometry).map_err(|error| format!("无法序列化浮窗尺寸：{error}"))?;
    state.storage.set_preference(&key, &raw)?;
    state
        .config
        .update(|config| config.overlay.appearance = OverlayAppearance::from(style))?;
    if let Some(window) = app.get_webview_window("lyrics-overlay") {
        crate::sync_overlay_vibrancy(&window, style);
    }
    app.emit("overlay://style", style)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_overlay_style(
    app: tauri::AppHandle,
    style: OverlayStyleSettings,
    state: State<'_, AppState>,
) -> Result<OverlayStyleSettings, String> {
    let style = style.normalized();
    let previous_orientation = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .orientation;
    *state
        .overlay_style
        .write()
        .unwrap_or_else(|error| error.into_inner()) = style.clone();
    if previous_orientation != style.orientation {
        crate::reset_overlay_toolbar_placement(&app, style.orientation);
    }
    persist_overlay_style_for_current_monitor(&app, &state, &style)?;
    crate::sync_unlock_handle(&app);
    Ok(style)
}

#[tauri::command]
pub fn nudge_overlay(app: tauri::AppHandle, dx: i32, dy: i32) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .locked
    {
        return Err("请先解锁歌词浮窗".into());
    }
    let window = app
        .get_webview_window("lyrics-overlay")
        .ok_or_else(|| "歌词浮窗不存在".to_string())?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    window
        .set_position(tauri::PhysicalPosition::new(
            position.x.saturating_add(dx.clamp(-20, 20)),
            position.y.saturating_add(dy.clamp(-20, 20)),
        ))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reset_overlay_bounds(app: tauri::AppHandle) -> Result<OverlayStyleSettings, String> {
    let state = app.state::<AppState>();
    let locked = state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .locked;
    let window = match app.get_webview_window("lyrics-overlay") {
        Some(window) => window,
        None => {
            crate::create_overlay(&app).map_err(|error| error.to_string())?;
            app.get_webview_window("lyrics-overlay")
                .ok_or_else(|| "无法创建歌词浮窗".to_string())?
        }
    };
    let (current_width, current_height) = window
        .outer_size()
        .ok()
        .and_then(|size| {
            let scale = window.scale_factor().ok()?;
            let scale = if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            };
            Some((size.width as f64 / scale, size.height as f64 / scale))
        })
        .unwrap_or((190.0, 156.0));
    let style = {
        let mut current = state
            .overlay_style
            .write()
            .unwrap_or_else(|error| error.into_inner());
        clear_manual_overlay_bounds(&mut current);
        current.clone()
    };
    state
        .storage
        .remove_preferences_with_prefix("overlay.position.")?;
    state
        .storage
        .remove_preferences_with_prefix("overlay.geometry.")?;
    state.storage.remove_preference("overlay.last_monitor")?;
    *state
        .overlay_monitor
        .write()
        .unwrap_or_else(|error| error.into_inner()) = None;
    state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .preferred_monitor = None;
    let (reset_width, reset_height) =
        reset_overlay_dimensions(style.orientation, current_width, current_height);
    window
        .set_size(tauri::LogicalSize::new(reset_width, reset_height))
        .map_err(|error| error.to_string())?;
    window
        .set_ignore_cursor_events(locked)
        .map_err(|error| error.to_string())?;
    let _ = window.set_focusable(!locked);
    if !locked {
        crate::refresh_overlay_mouse_tracking(&window);
    }
    let _ = window.set_resizable(false);
    crate::move_overlay_to_primary(&app, &window);
    persist_overlay_style_for_current_monitor(&app, &state, &style)?;
    state
        .overlay_settings
        .write()
        .unwrap_or_else(|error| error.into_inner())
        .visible = true;
    state
        .config
        .update(|config| config.overlay.visible = true)?;
    crate::sync_tray_overlay_checked(&app, true);
    crate::reconcile_overlay_visibility(&app)?;
    app.emit("overlay://settings", get_overlay_settings_inner(&state))
        .map_err(|error| error.to_string())?;
    Ok(style)
}

fn clear_manual_overlay_bounds(style: &mut OverlayStyleSettings) {
    style.horizontal_max_width = None;
    style.vertical_max_height = None;
}

fn reset_overlay_dimensions(
    orientation: OverlayOrientation,
    current_width: f64,
    current_height: f64,
) -> (f64, f64) {
    match orientation {
        OverlayOrientation::Horizontal => (760.0, current_height.max(76.0)),
        OverlayOrientation::Vertical => (current_width.max(190.0), 620.0),
    }
}

fn resize_overlay_edge_bounds(
    position: tauri::PhysicalPosition<i32>,
    current_size: tauri::PhysicalSize<u32>,
    edge: OverlayResizeEdge,
    requested_main_size: f64,
    minimum_main_size: f64,
    scale: f64,
    monitor_position: tauri::PhysicalPosition<i32>,
    monitor_size: tauri::PhysicalSize<u32>,
) -> (tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let margin = 0_i64;
    let minimum_main_size = if minimum_main_size.is_finite() {
        minimum_main_size.max(0.0)
    } else {
        0.0
    };
    let work_left = monitor_position.x as i64 + margin;
    let work_top = monitor_position.y as i64 + margin;
    let work_right = monitor_position.x as i64 + monitor_size.width as i64 - margin;
    let work_bottom = monitor_position.y as i64 + monitor_size.height as i64 - margin;
    let available_width = (work_right - work_left).max(1) as u32;
    let available_height = (work_bottom - work_top).max(1) as u32;
    let minimum_width =
        ((minimum_main_size.max(320.0) * scale).round() as u32).min(available_width);
    let minimum_height =
        ((minimum_main_size.max(280.0) * scale).round() as u32).min(available_height);
    let fallback_size = match edge {
        OverlayResizeEdge::Left | OverlayResizeEdge::Right => current_size.width,
        OverlayResizeEdge::Top | OverlayResizeEdge::Bottom => current_size.height,
    };
    let requested = if requested_main_size.is_finite() {
        (requested_main_size.max(0.0) * scale).round() as u32
    } else {
        fallback_size
    };

    match edge {
        OverlayResizeEdge::Left => {
            let fixed_right = (position.x as i64 + current_size.width as i64)
                .clamp(work_left + minimum_width as i64, work_right);
            let maximum_width = (fixed_right - work_left) as u32;
            let width = requested.clamp(minimum_width, maximum_width.max(minimum_width));
            (
                tauri::PhysicalPosition::new((fixed_right - width as i64) as i32, position.y),
                tauri::PhysicalSize::new(width, current_size.height),
            )
        }
        OverlayResizeEdge::Right => {
            let fixed_left =
                (position.x as i64).clamp(work_left, work_right - minimum_width as i64);
            let maximum_width = (work_right - fixed_left) as u32;
            let width = requested.clamp(minimum_width, maximum_width.max(minimum_width));
            (
                tauri::PhysicalPosition::new(fixed_left as i32, position.y),
                tauri::PhysicalSize::new(width, current_size.height),
            )
        }
        OverlayResizeEdge::Top => {
            let fixed_bottom = (position.y as i64 + current_size.height as i64)
                .clamp(work_top + minimum_height as i64, work_bottom);
            let maximum_height = (fixed_bottom - work_top) as u32;
            let height = requested.clamp(minimum_height, maximum_height.max(minimum_height));
            (
                tauri::PhysicalPosition::new(position.x, (fixed_bottom - height as i64) as i32),
                tauri::PhysicalSize::new(current_size.width, height),
            )
        }
        OverlayResizeEdge::Bottom => {
            let fixed_top =
                (position.y as i64).clamp(work_top, work_bottom - minimum_height as i64);
            let maximum_height = (work_bottom - fixed_top) as u32;
            let height = requested.clamp(minimum_height, maximum_height.max(minimum_height));
            (
                tauri::PhysicalPosition::new(position.x, fixed_top as i32),
                tauri::PhysicalSize::new(current_size.width, height),
            )
        }
    }
}

#[tauri::command]
pub fn resize_overlay_edge(
    app: tauri::AppHandle,
    edge: OverlayResizeEdge,
    main_size: f64,
    minimum_main_size: f64,
    state: State<'_, AppState>,
) -> Result<OverlayResizeBounds, String> {
    if state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .locked
    {
        return Err("请先解锁歌词浮窗".into());
    }
    let window = app
        .get_webview_window("lyrics-overlay")
        .ok_or_else(|| "歌词浮窗不存在".to_string())?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let current_size = window.outer_size().map_err(|error| error.to_string())?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or_else(|| "无法读取显示器信息".to_string())?;
    let work_area = monitor.work_area();
    let (next_position, next_size) = resize_overlay_edge_bounds(
        position,
        current_size,
        edge,
        main_size,
        minimum_main_size,
        scale,
        work_area.position,
        work_area.size,
    );
    if current_size != next_size {
        window
            .set_size(next_size)
            .map_err(|error| error.to_string())?;
    }
    if position != next_position {
        window
            .set_position(next_position)
            .map_err(|error| error.to_string())?;
    }
    let applied = window.outer_size().unwrap_or(next_size);
    crate::sync_unlock_handle(&app);
    Ok(OverlayResizeBounds {
        width: applied.width as f64 / scale,
        height: applied.height as f64 / scale,
    })
}

fn fixed_axis_content_size(
    style: &OverlayStyleSettings,
    requested_width: f64,
    requested_height: f64,
    current_width: f64,
    current_height: f64,
    locked: bool,
) -> (f64, f64) {
    match style.orientation {
        OverlayOrientation::Horizontal => (
            if locked {
                current_width
            } else {
                style.horizontal_max_width.unwrap_or(760.0)
            },
            requested_height,
        ),
        OverlayOrientation::Vertical => (
            requested_width,
            if locked {
                current_height
            } else {
                style.vertical_max_height.unwrap_or(620.0)
            },
        ),
    }
}

fn fit_overlay_bounds(
    position: tauri::PhysicalPosition<i32>,
    requested_width: f64,
    requested_height: f64,
    scale: f64,
    monitor_position: tauri::PhysicalPosition<i32>,
    monitor_size: tauri::PhysicalSize<u32>,
) -> (tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let margin = 0_u32;
    let minimum_width = (190.0 * scale).round() as u32;
    let minimum_height = (76.0 * scale).round() as u32;
    let maximum_width = monitor_size
        .width
        .saturating_sub(margin.saturating_mul(2))
        .max(minimum_width);
    let maximum_height = monitor_size
        .height
        .saturating_sub(margin.saturating_mul(2))
        .max(minimum_height);
    let requested_width = if requested_width.is_finite() {
        (requested_width.max(0.0) * scale).round() as u32
    } else {
        minimum_width
    };
    let requested_height = if requested_height.is_finite() {
        (requested_height.max(0.0) * scale).round() as u32
    } else {
        minimum_height
    };
    let size = tauri::PhysicalSize::new(
        requested_width.clamp(minimum_width, maximum_width),
        requested_height.clamp(minimum_height, maximum_height),
    );

    let minimum_x = monitor_position.x as i64 + margin as i64;
    let minimum_y = monitor_position.y as i64 + margin as i64;
    let maximum_x =
        monitor_position.x as i64 + monitor_size.width as i64 - margin as i64 - size.width as i64;
    let maximum_y =
        monitor_position.y as i64 + monitor_size.height as i64 - margin as i64 - size.height as i64;
    let x = (position.x as i64).clamp(minimum_x, maximum_x.max(minimum_x));
    let y = (position.y as i64).clamp(minimum_y, maximum_y.max(minimum_y));

    (tauri::PhysicalPosition::new(x as i32, y as i32), size)
}

#[tauri::command]
pub fn fit_overlay_content(app: tauri::AppHandle, width: f64, height: f64) -> Result<bool, String> {
    let window = app
        .get_webview_window("lyrics-overlay")
        .ok_or_else(|| "歌词浮窗不存在".to_string())?;
    if crate::primary_mouse_button_pressed() {
        return Ok(false);
    }
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let current_size = window.outer_size().map_err(|error| error.to_string())?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or_else(|| "无法读取显示器信息".to_string())?;
    let work_area = monitor.work_area();
    let state = app.state::<AppState>();
    let style = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let locked = state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .locked;
    let current_width = current_size.width as f64 / scale;
    let current_height = current_size.height as f64 / scale;
    let (width, height) =
        fixed_axis_content_size(&style, width, height, current_width, current_height, locked);
    let (next_position, next_size) = fit_overlay_bounds(
        position,
        width,
        height,
        scale,
        work_area.position,
        work_area.size,
    );
    let size_changed = current_size.width.abs_diff(next_size.width) > 2
        || current_size.height.abs_diff(next_size.height) > 2;
    if size_changed {
        window
            .set_size(next_size)
            .map_err(|error| error.to_string())?;
    }
    if size_changed || position != next_position {
        window
            .set_position(next_position)
            .map_err(|error| error.to_string())?;
    }
    crate::sync_unlock_handle(&app);
    Ok(true)
}

#[tauri::command]
pub fn fit_notch_lyrics_content(
    app: tauri::AppHandle,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let window = app
        .get_webview_window("lyrics-notch")
        .ok_or_else(|| "灵动岛歌词窗口不存在".to_string())?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or_else(|| "无法读取灵动岛歌词所在的显示器".to_string())?;
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let requested_width = if width.is_finite() {
        width.max(120.0)
    } else {
        120.0
    };
    let requested_height = if height.is_finite() {
        height.max(44.0)
    } else {
        44.0
    };
    let next_size = tauri::PhysicalSize::new(
        ((requested_width * scale).round() as u32).min(monitor_size.width),
        ((requested_height * scale).round() as u32).min(monitor_size.height),
    );
    let next_position = tauri::PhysicalPosition::new(
        monitor_position.x + monitor_size.width.saturating_sub(next_size.width) as i32 / 2,
        monitor_position.y,
    );
    let current_size = window.outer_size().map_err(|error| error.to_string())?;
    let current_position = window.outer_position().map_err(|error| error.to_string())?;
    let size_changed = current_size.width.abs_diff(next_size.width) > 1
        || current_size.height.abs_diff(next_size.height) > 1;
    if size_changed {
        window
            .set_size(next_size)
            .map_err(|error| error.to_string())?;
    }
    if current_position != next_position {
        window
            .set_position(next_position)
            .map_err(|error| error.to_string())?;
    }
    if size_changed {
        crate::refresh_overlay_mouse_tracking(&window);
    }
    Ok(())
}

#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::show_main_window_centered(&app)
}

#[tauri::command]
pub fn show_lyrics_style_settings(
    app: tauri::AppHandle,
    mode: LyricsStyleMode,
) -> Result<(), String> {
    let mode = match mode {
        LyricsStyleMode::Desktop => "desktop",
        LyricsStyleMode::StatusBar => "statusBar",
        LyricsStyleMode::ListWindow => "listWindow",
        LyricsStyleMode::Notch => "notch",
    };
    let route = format!("#/settings/style?mode={mode}");
    crate::show_main_window_at(&app, Some(&route))
}

#[tauri::command]
pub fn show_quick_lyrics_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::show_quick_lyrics_window(&app)
}

#[tauri::command]
pub fn get_app_config(state: State<'_, AppState>) -> AppConfig {
    state.config.snapshot()
}

#[tauri::command]
pub fn get_legal_notice_status(state: State<'_, AppState>) -> Result<LegalNoticeStatus, String> {
    Ok(LegalNoticeStatus {
        current_version: crate::LEGAL_NOTICE_VERSION,
        accepted: crate::legal_notice_accepted(&state.storage)?,
    })
}

#[tauri::command]
pub fn accept_legal_notice(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.storage.set_preference(
        crate::LEGAL_NOTICE_PREFERENCE,
        &crate::LEGAL_NOTICE_VERSION.to_string(),
    )?;
    crate::activate_runtime(&app)
}

#[tauri::command]
pub fn quit_application(app: tauri::AppHandle) {
    log::info!("Application exit requested: reason=frontend_quit_command");
    app.exit(0);
}

#[tauri::command]
pub fn set_theme(
    app: tauri::AppHandle,
    theme: ThemePreference,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state.config.update(|config| config.app.theme = theme)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

fn plist_string(path: &Path, key: &str) -> Option<String> {
    let mut command = Command::new("/usr/bin/plutil");
    command.args(["-extract", key, "raw", "-o", "-"]).arg(path);
    let output = run_with_timeout(command, Duration::from_secs(3)).ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(target_os = "macos")]
fn localized_application_name(path: &Path) -> Option<String> {
    use objc2_foundation::{NSBundle, NSString};

    let bundle_path = NSString::from_str(path.to_string_lossy().as_ref());
    let bundle = NSBundle::bundleWithPath(&bundle_path)?;
    ["CFBundleDisplayName", "CFBundleName"]
        .into_iter()
        .find_map(|key| {
            let value = bundle.objectForInfoDictionaryKey(&NSString::from_str(key))?;
            let value = value.downcast_ref::<NSString>()?.to_string();
            (!value.trim().is_empty()).then_some(value)
        })
}

#[cfg(not(target_os = "macos"))]
fn localized_application_name(_path: &Path) -> Option<String> {
    None
}

fn application_display_name(name: String) -> String {
    name.strip_suffix(".app").unwrap_or(&name).to_owned()
}

fn resolve_registered_application(path: &Path) -> Result<RegisteredApplication, String> {
    if !path.is_dir() || path.extension().and_then(|value| value.to_str()) != Some("app") {
        return Err(format!("不是有效的 .app：{}", path.display()));
    }
    let plist = ["Contents/Info.plist", "WrappedBundle/Info.plist"]
        .into_iter()
        .map(|relative_path| path.join(relative_path))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("应用缺少 Info.plist：{}", path.display()))?;
    let bundle_id = plist_string(&plist, "CFBundleIdentifier")
        .ok_or_else(|| format!("应用缺少 Bundle ID：{}", path.display()))?;
    let name = localized_application_name(path)
        .or_else(|| plist_string(&plist, "CFBundleDisplayName"))
        .or_else(|| plist_string(&plist, "CFBundleName"))
        .or_else(|| {
            path.file_stem()
                .map(|value| value.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| bundle_id.clone());
    Ok(RegisteredApplication {
        name: application_display_name(name),
        bundle_id,
    })
}

#[tauri::command]
pub fn resolve_system_media_applications(
    paths: Vec<PathBuf>,
) -> Result<Vec<RegisteredApplication>, String> {
    normalize_system_media_applications(
        paths
            .iter()
            .map(|path| resolve_registered_application(path))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

#[tauri::command]
pub fn set_system_media_applications(
    app: tauri::AppHandle,
    applications: Vec<RegisteredApplication>,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let applications = normalize_system_media_applications(applications)?;
    let config = state
        .config
        .update(|config| config.app.system_media_applications = applications)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_system_media_filter_mode(
    app: tauri::AppHandle,
    mode: SystemMediaFilterMode,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.app.system_media_filter_mode = mode)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn resolve_player_follower_application(path: PathBuf) -> Result<RegisteredApplication, String> {
    normalize_player_follower_application(Some(resolve_registered_application(&path)?))?
        .ok_or_else(|| "未选择播放器".into())
}

#[tauri::command]
pub fn set_player_follower_application(
    app: tauri::AppHandle,
    application: Option<RegisteredApplication>,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let application = normalize_player_follower_application(application)?;
    let config = state
        .config
        .update(|config| config.app.player_follower_application = application)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    crate::player_lifecycle::sync_service(&app, &config.app)?;
    Ok(config)
}

#[tauri::command]
pub fn get_player_follower_service_status() -> crate::player_lifecycle::PlayerFollowerServiceState {
    crate::player_lifecycle::service_state()
}

#[tauri::command]
pub fn open_player_follower_system_settings() -> Result<(), String> {
    crate::player_lifecycle::open_system_settings()
}

#[tauri::command]
pub fn open_automation_system_settings(app: tauri::AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation",
            None::<&str>,
        )
        .map_err(|error| format!("打开自动化系统设置失败：{error}"))
}

#[tauri::command]
pub async fn get_application_icons(
    bundle_ids: Vec<String>,
) -> Result<HashMap<String, String>, String> {
    tauri::async_runtime::spawn_blocking(move || collect_application_icons(bundle_ids))
        .await
        .map_err(|error| format!("读取应用图标失败：{error}"))
}

fn collect_application_icons(bundle_ids: Vec<String>) -> HashMap<String, String> {
    bundle_ids
        .into_iter()
        .filter_map(|bundle_id| application_icon(&bundle_id).map(|icon| (bundle_id, icon)))
        .collect()
}

#[cfg(target_os = "macos")]
fn application_icon(bundle_id: &str) -> Option<String> {
    use objc2::rc::autoreleasepool;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let url =
            workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))?;
        let path = url.path()?;
        application_icon_at_path(&path.to_string())
    })
}

#[cfg(target_os = "macos")]
fn application_icon_at_path(path: &str) -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use objc2::{rc::autoreleasepool, AnyThread};
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let icon = workspace.iconForFile(&NSString::from_str(path));
        let mut bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(64.0, 64.0));
        let image = unsafe { icon.CGImageForProposedRect_context_hints(&mut bounds, None, None)? };
        let bitmap = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &image);
        let properties = NSDictionary::new();
        let png = unsafe {
            bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)?
        };
        Some(format!(
            "data:image/png;base64,{}",
            STANDARD.encode(png.to_vec())
        ))
    })
}

#[cfg(not(target_os = "macos"))]
fn application_icon(_bundle_id: &str) -> Option<String> {
    None
}

#[tauri::command]
pub async fn resolve_application_by_bundle_id(
    bundle_id: String,
) -> Result<RegisteredApplication, String> {
    tauri::async_runtime::spawn_blocking(move || resolve_application_bundle_id(&bundle_id))
        .await
        .map_err(|error| format!("读取应用信息失败：{error}"))?
}

#[cfg(target_os = "macos")]
fn resolve_application_bundle_id(bundle_id: &str) -> Result<RegisteredApplication, String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    let workspace = NSWorkspace::sharedWorkspace();
    let url = workspace
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))
        .ok_or_else(|| format!("找不到应用：{bundle_id}"))?;
    let path = url
        .path()
        .ok_or_else(|| format!("无法读取应用路径：{bundle_id}"))?;
    resolve_registered_application(Path::new(&path.to_string()))
}

#[cfg(not(target_os = "macos"))]
fn resolve_application_bundle_id(_bundle_id: &str) -> Result<RegisteredApplication, String> {
    Err("应用解析仅支持 macOS".into())
}

#[tauri::command]
pub fn set_language(
    app: tauri::AppHandle,
    language: LanguagePreference,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    if !language.is_valid() {
        return Err("language must be system or a valid BCP 47 language tag".into());
    }
    let config = state
        .config
        .update(|config| config.app.language = language)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_native_language(
    app: tauri::AppHandle,
    language: UiLanguage,
    state: State<'_, AppState>,
) -> Result<(), String> {
    crate::apply_native_language(&app, language)?;
    if state.config.set_comment_language(language)? {
        app.emit("config://changed", state.config.snapshot())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_global_shortcut_status(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<GlobalShortcutStatus, String> {
    let ([toggle, unlock, reset], [toggle_status_bar, toggle_list, toggle_notch]) =
        state.config.snapshot().app.shortcuts.parsed()?;
    let shortcuts = app.global_shortcut();
    Ok(GlobalShortcutStatus {
        toggle_overlay: shortcuts.is_registered(toggle),
        unlock_overlay: shortcuts.is_registered(unlock),
        reset_overlay: shortcuts.is_registered(reset),
        toggle_status_bar_lyrics: toggle_status_bar
            .is_some_and(|shortcut| shortcuts.is_registered(shortcut)),
        toggle_list_lyrics: toggle_list.is_some_and(|shortcut| shortcuts.is_registered(shortcut)),
        toggle_notch_lyrics: toggle_notch.is_some_and(|shortcut| shortcuts.is_registered(shortcut)),
    })
}

pub fn update_global_shortcuts(
    app: &tauri::AppHandle,
    shortcuts: GlobalShortcutSettings,
) -> Result<AppConfig, String> {
    let state = app.state::<AppState>();
    let previous = state.config.snapshot().app.shortcuts;
    crate::apply_global_shortcuts(app, &previous, &shortcuts)?;
    let registered = shortcuts.clone();
    let config = match state
        .config
        .update(|config| config.app.shortcuts = shortcuts)
    {
        Ok(config) => config,
        Err(error) => {
            let _ = crate::apply_global_shortcuts(app, &registered, &previous);
            return Err(error);
        }
    };
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_global_shortcuts(
    app: tauri::AppHandle,
    shortcuts: GlobalShortcutSettings,
) -> Result<AppConfig, String> {
    update_global_shortcuts(&app, shortcuts)
}

pub fn update_dock_icon_hidden(app: &tauri::AppHandle, hidden: bool) -> Result<AppConfig, String> {
    let state = app.state::<AppState>();
    let previous = state.config.snapshot().app.hide_dock_icon;
    crate::apply_dock_icon_hidden(app, hidden)?;
    let config = match state
        .config
        .update(|config| config.app.hide_dock_icon = hidden)
    {
        Ok(config) => config,
        Err(error) => {
            let _ = crate::apply_dock_icon_hidden(app, previous);
            return Err(error);
        }
    };
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_dock_icon_hidden(app: tauri::AppHandle, hidden: bool) -> Result<AppConfig, String> {
    update_dock_icon_hidden(&app, hidden)
}

#[tauri::command]
pub fn set_silent_startup(
    app: tauri::AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.app.silent_startup = enabled)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_auto_check_updates(
    app: tauri::AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.app.auto_check_updates = enabled)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_overlay_hide_when_not_playing(
    app: tauri::AppHandle,
    hidden: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.overlay.hide_when_not_playing = hidden)?;
    crate::reconcile_overlay_visibility(&app)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

fn finish_display_config_update(
    app: &tauri::AppHandle,
    config: AppConfig,
) -> Result<AppConfig, String> {
    crate::sync_lyrics_surfaces(app);
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_lyrics_base_appearance(
    app: tauri::AppHandle,
    appearance: LyricsBaseAppearance,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.lyrics.base_appearance = appearance.clone())?;
    sync_desktop_style_from_config(&app, &state, &config)?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_lyrics_style_inheritance(
    app: tauri::AppHandle,
    mode: LyricsStyleMode,
    inheritance: LyricsModeStyleInheritance,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state.config.update(|config| match mode {
        LyricsStyleMode::Desktop => config.lyrics.style_inheritance.desktop = inheritance,
        LyricsStyleMode::StatusBar => config.lyrics.style_inheritance.status_bar = inheritance,
        LyricsStyleMode::ListWindow => config.lyrics.style_inheritance.list_window = inheritance,
        LyricsStyleMode::Notch => config.lyrics.style_inheritance.notch = inheritance,
    })?;
    if matches!(mode, LyricsStyleMode::Desktop) {
        sync_desktop_style_from_config(&app, &state, &config)?;
    }
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn reset_lyrics_base_appearance(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state.config.update(|config| {
        config.lyrics.base_appearance = LyricsBaseAppearance::default();
    })?;
    sync_desktop_style_from_config(&app, &state, &config)?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_status_bar_lyrics_enabled(
    app: tauri::AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.lyrics.displays.status_bar.enabled = enabled)?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_list_lyrics_visible(
    app: tauri::AppHandle,
    visible: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.lyrics.displays.list_window.enabled = visible)?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_list_lyrics_options(
    app: tauri::AppHandle,
    show_translation: bool,
    show_romanization: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state.config.update(|config| {
        config.lyrics.displays.list_window.show_translation = show_translation;
        config.lyrics.displays.list_window.show_romanization = show_romanization;
    })?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_notch_lyrics_visible(
    app: tauri::AppHandle,
    visible: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.lyrics.displays.notch.enabled = visible)?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_lyrics_display_preferences(
    app: tauri::AppHandle,
    mode: LyricsStyleMode,
    preferences: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = match mode {
        LyricsStyleMode::Desktop => return Err("桌面歌词样式请使用桌面样式接口".into()),
        LyricsStyleMode::StatusBar => {
            let value = serde_json::from_value::<StatusBarLyricsPreferences>(preferences)
                .map_err(|error| format!("状态栏歌词配置无效：{error}"))?;
            state
                .config
                .update(|config| config.lyrics.displays.status_bar = value.clone())?
        }
        LyricsStyleMode::ListWindow => {
            let value = serde_json::from_value::<ListLyricsPreferences>(preferences)
                .map_err(|error| format!("歌词列表配置无效：{error}"))?;
            state
                .config
                .update(|config| config.lyrics.displays.list_window = value.clone())?
        }
        LyricsStyleMode::Notch => {
            let value = serde_json::from_value::<NotchLyricsPreferences>(preferences)
                .map_err(|error| format!("灵动岛歌词配置无效：{error}"))?;
            state
                .config
                .update(|config| config.lyrics.displays.notch = value.clone())?
        }
    };
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn reset_lyrics_style_mode(
    app: tauri::AppHandle,
    mode: LyricsStyleMode,
    state: State<'_, AppState>,
) -> Result<SettingsResetResponse, String> {
    if matches!(mode, LyricsStyleMode::Desktop) {
        return reset_settings_section(app, SettingsSection::Style, state);
    }
    state.config.update(|config| match mode {
        LyricsStyleMode::StatusBar => {
            config.lyrics.displays.status_bar = Default::default();
            config.lyrics.style_inheritance.status_bar = Default::default();
        }
        LyricsStyleMode::ListWindow => {
            config.lyrics.displays.list_window = Default::default();
            config.lyrics.style_inheritance.list_window = Default::default();
        }
        LyricsStyleMode::Notch => {
            config.lyrics.displays.notch = Default::default();
            config.lyrics.style_inheritance.notch = Default::default();
        }
        LyricsStyleMode::Desktop => {}
    })?;
    let configured = state.config.snapshot();
    crate::sync_lyrics_surfaces(&app);
    app.emit("config://changed", &configured)
        .map_err(|error| error.to_string())?;
    Ok(SettingsResetResponse {
        overlay_settings: get_overlay_settings_inner(&state),
        overlay_style: state
            .overlay_style
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone(),
        provider_view: state.providers.settings_view(),
        player_selection: *state
            .selection
            .read()
            .unwrap_or_else(|error| error.into_inner()),
    })
}

#[tauri::command]
pub fn reset_lyrics_display_position(
    app: tauri::AppHandle,
    mode: LyricsStyleMode,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let label = match mode {
        LyricsStyleMode::StatusBar => "lyrics-status-bar",
        LyricsStyleMode::ListWindow => "lyrics-list",
        LyricsStyleMode::Notch => "lyrics-notch",
        LyricsStyleMode::Desktop => return Err("桌面歌词请使用桌面位置复位命令".into()),
    };
    if label == "lyrics-status-bar" {
        state
            .storage
            .remove_preference("lyrics-status-bar.position")?;
        state
            .storage
            .remove_preference("lyrics-status-bar.last-monitor")?;
        state
            .storage
            .remove_preferences_with_prefix("lyrics-status-bar.position.")?;
    } else {
        state
            .storage
            .remove_preferences_with_prefix(&format!("{label}.position."))?;
    }
    if let Some(window) = app.get_webview_window(label) {
        crate::position_auxiliary_lyrics_window_default(&app, &window, label)?;
    }
    Ok(())
}

#[tauri::command]
pub fn reset_list_lyrics_window_size(app: tauri::AppHandle) -> Result<(), String> {
    crate::reset_list_lyrics_window_size(&app)
}

#[tauri::command]
pub fn export_app_config(state: State<'_, AppState>) -> Result<ConfigExport, String> {
    Ok(ConfigExport {
        file_name: "lyrics-plus-config.jsonc".into(),
        raw: state.config.export_json()?,
    })
}

#[tauri::command]
pub fn reveal_config_directory(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let directory = state
        .config
        .path()
        .parent()
        .ok_or_else(|| "配置目录无效".to_string())?;
    app.opener()
        .open_path(directory.to_string_lossy(), None::<&str>)
        .map_err(|error| format!("打开配置目录失败：{error}"))
}

#[tauri::command]
pub fn get_config_editor_data(state: State<'_, AppState>) -> ConfigEditorData {
    state.config.editor_data()
}

#[tauri::command]
pub fn validate_app_config_draft(raw: String) -> ConfigDraftValidation {
    validate_config_draft(&raw)
}

#[tauri::command]
pub fn save_app_config_draft(
    app: tauri::AppHandle,
    raw: String,
    expected_revision: u64,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let validation = validate_config_draft(&raw);
    if let Some(error) = validation.error {
        return Err(format!(
            "第 {} 行第 {} 列：{}",
            error.line, error.column, error.message
        ));
    }
    apply_app_config(&app, &state, validation.effective_config, expected_revision)
}

fn apply_app_config(
    app: &tauri::AppHandle,
    state: &AppState,
    next: AppConfig,
    expected_revision: u64,
) -> Result<AppConfig, String> {
    let previous_config = state.config.snapshot();
    let previous_dock_icon_hidden = previous_config.app.hide_dock_icon;
    let previous_shortcuts = previous_config.app.shortcuts.clone();
    let dock_visibility_changed = previous_dock_icon_hidden != next.app.hide_dock_icon;
    if dock_visibility_changed {
        crate::apply_dock_icon_hidden(app, next.app.hide_dock_icon)?;
    }
    let shortcuts_changed = previous_shortcuts != next.app.shortcuts;
    if shortcuts_changed {
        if let Err(error) =
            crate::apply_global_shortcuts(app, &previous_shortcuts, &next.app.shortcuts)
        {
            if dock_visibility_changed {
                let _ = crate::apply_dock_icon_hidden(app, previous_dock_icon_hidden);
            }
            return Err(error);
        }
    }
    let save_result = state
        .config
        .replace_at_revision(next.clone(), expected_revision);
    let saved = match save_result {
        Ok(saved) => saved,
        Err(error) => {
            if dock_visibility_changed {
                let _ = crate::apply_dock_icon_hidden(app, previous_dock_icon_hidden);
            }
            if shortcuts_changed {
                let _ =
                    crate::apply_global_shortcuts(app, &next.app.shortcuts, &previous_shortcuts);
            }
            return Err(error);
        }
    };

    let geometry = {
        let style = state
            .overlay_style
            .read()
            .unwrap_or_else(|error| error.into_inner());
        (style.horizontal_max_width, style.vertical_max_height)
    };
    let mut style = saved.overlay.appearance.clone().into_style();
    style.horizontal_max_width = geometry.0;
    style.vertical_max_height = geometry.1;
    *state
        .overlay_style
        .write()
        .unwrap_or_else(|error| error.into_inner()) = style.clone();
    if let Some(window) = app.get_webview_window("lyrics-overlay") {
        crate::sync_overlay_vibrancy(&window, &style);
    }

    state
        .providers
        .set_settings(saved.lyrics.providers.clone())?;
    invalidate_lyrics_search_session(&state);
    *state
        .selection
        .write()
        .unwrap_or_else(|error| error.into_inner()) = saved.app.player_selection;
    *state
        .auto_player
        .write()
        .unwrap_or_else(|error| error.into_inner()) = None;
    *state
        .overlay_settings
        .write()
        .unwrap_or_else(|error| error.into_inner()) = OverlaySettings {
        visible: saved.overlay.visible,
        locked: saved.overlay.locked,
    };
    if let Some(window) = app.get_webview_window("lyrics-overlay") {
        let _ = window.set_ignore_cursor_events(saved.overlay.locked);
        let _ = window.set_focusable(!saved.overlay.locked);
        if !saved.overlay.locked {
            crate::refresh_overlay_mouse_tracking(&window);
        }
    }
    crate::reconcile_overlay_visibility(app)?;
    crate::sync_tray_overlay_checked(app, saved.overlay.visible);
    crate::sync_lyrics_surfaces(app);
    let _ = app.emit("player://selection", saved.app.player_selection);
    let _ = app.emit("overlay://settings", get_overlay_settings_inner(&state));
    let _ = app.emit("overlay://style", &style);
    app.emit("config://changed", &saved)
        .map_err(|error| error.to_string())?;
    crate::player_lifecycle::sync_service(app, &saved.app)?;
    Ok(saved)
}

#[tauri::command]
pub fn reset_settings_section(
    app: tauri::AppHandle,
    section: SettingsSection,
    state: State<'_, AppState>,
) -> Result<SettingsResetResponse, String> {
    let mut player_follower_error = None;
    match section {
        SettingsSection::Style => {
            state
                .storage
                .remove_preferences_with_prefix("overlay.style.")?;

            let geometry = {
                let current = state
                    .overlay_style
                    .read()
                    .unwrap_or_else(|error| error.into_inner());
                (current.horizontal_max_width, current.vertical_max_height)
            };
            *state
                .overlay_settings
                .write()
                .unwrap_or_else(|error| error.into_inner()) = OverlaySettings::default();
            let configured = state.config.update(|config| {
                config.overlay.appearance = OverlayAppearance::default();
                config.overlay.visible = true;
                config.overlay.locked = false;
                config.overlay.hide_when_not_playing = false;
                config.lyrics.style_inheritance.desktop = Default::default();
            })?;
            let mut style = configured.overlay.appearance.into_style();
            style.horizontal_max_width = geometry.0;
            style.vertical_max_height = geometry.1;
            *state
                .overlay_style
                .write()
                .unwrap_or_else(|error| error.into_inner()) = style.clone();

            if let Some(window) = app.get_webview_window("lyrics-overlay") {
                crate::sync_overlay_vibrancy(&window, &style);
                crate::reset_overlay_toolbar_placement(&app, style.orientation);
            }
            app.emit("overlay://style", &style)
                .map_err(|error| error.to_string())?;
            app.emit("overlay://settings", get_overlay_settings_inner(&state))
                .map_err(|error| error.to_string())?;
            crate::reconcile_overlay_visibility(&app)?;
            crate::sync_tray_overlay_checked(&app, true);
        }
        SettingsSection::Display => {
            state
                .storage
                .remove_preferences_with_prefix("overlay.position.")?;
            state
                .storage
                .remove_preferences_with_prefix("overlay.geometry.")?;
            state.storage.remove_preference("overlay.last_monitor")?;
            state.storage.remove_preference("overlay.visible")?;
            state.storage.remove_preference("overlay.locked")?;
            state.storage.remove_preference("overlay.passthrough")?;

            let style = {
                let mut current = state
                    .overlay_style
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone();
                current.horizontal_max_width = None;
                current.vertical_max_height = None;
                current
            };
            *state
                .overlay_style
                .write()
                .unwrap_or_else(|error| error.into_inner()) = style.clone();
            *state
                .overlay_monitor
                .write()
                .unwrap_or_else(|error| error.into_inner()) = None;
            state
                .overlay_placement
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .preferred_monitor = None;
            *state
                .overlay_settings
                .write()
                .unwrap_or_else(|error| error.into_inner()) = OverlaySettings::default();
            state.config.update(|config| {
                config.overlay.visible = true;
                config.overlay.locked = false;
                config.overlay.hide_when_not_playing = false;
                config.lyrics.displays = Default::default();
            })?;

            let window = app
                .get_webview_window("lyrics-overlay")
                .ok_or_else(|| "歌词浮窗不存在".to_string())?;
            crate::sync_overlay_vibrancy(&window, &style);
            window
                .set_ignore_cursor_events(false)
                .map_err(|error| error.to_string())?;
            let _ = window.set_focusable(true);
            crate::refresh_overlay_mouse_tracking(&window);
            let _ = window.set_resizable(false);
            crate::restore_overlay_position(&app, &window);
            crate::reconcile_overlay_visibility(&app)?;
            crate::sync_tray_overlay_checked(&app, true);
            app.emit("overlay://settings", get_overlay_settings_inner(&state))
                .map_err(|error| error.to_string())?;
            app.emit("overlay://style", &style)
                .map_err(|error| error.to_string())?;
        }
        SettingsSection::Lyrics => {
            let view = state.providers.set_settings(ProviderSettings::default())?;
            state
                .config
                .update(|config| config.lyrics.providers = view.settings)?;
            invalidate_lyrics_search_session(&state);
        }
        SettingsSection::Player => {
            update_player_selection(&app, PlayerSelection::Auto)?;
            let config = state.config.update(|config| {
                config.app.system_media_filter_mode = SystemMediaFilterMode::Allowlist;
                config.app.system_media_applications.clear();
                config.app.player_follower_application = None;
            })?;
            player_follower_error = crate::player_lifecycle::sync_service(&app, &config.app).err();
        }
        SettingsSection::Application => {
            update_dock_icon_hidden(&app, false)?;
            update_global_shortcuts(&app, GlobalShortcutSettings::default())?;
            state.config.update(|config| {
                config.app.theme = ThemePreference::Dark;
                config.app.language = LanguagePreference::default();
                config.app.silent_startup = false;
            })?;
        }
        SettingsSection::About => {
            state
                .config
                .update(|config| config.app.auto_check_updates = true)?;
        }
    }

    let configured = state.config.snapshot();
    crate::sync_lyrics_surfaces(&app);
    let _ = app.emit("config://changed", &configured);
    if let Some(error) = player_follower_error {
        return Err(error);
    }
    Ok(SettingsResetResponse {
        overlay_settings: get_overlay_settings_inner(&state),
        overlay_style: state
            .overlay_style
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone(),
        provider_view: state.providers.settings_view(),
        player_selection: *state
            .selection
            .read()
            .unwrap_or_else(|error| error.into_inner()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_style_deserializes_old_saved_shape_with_defaults() {
        let style: OverlayStyleSettings = serde_json::from_str(
            r##"{"fontSize":42,"activeColor":"#ff0000","inactiveColor":"#888888","opacity":0.7}"##,
        )
        .unwrap();
        let style = style.normalized();
        assert_eq!(style.font_size, 42);
        assert_eq!(style.background, OverlayBackground::Glass);
        assert_eq!(style.background_blur, 18.0);
        assert_eq!(style.layout, OverlayLayout::Single);
        assert_eq!(style.orientation, OverlayOrientation::Horizontal);
        assert_eq!(style.secondary_display, SecondaryDisplayMode::Translation);
        assert!(!style.auto_center_with_translation_or_romanization);
    }

    #[test]
    fn legacy_transparent_background_normalizes_to_transparent_mode() {
        let style: OverlayStyleSettings =
            serde_json::from_str(r##"{"background":"transparent","backgroundOpacity":0.75}"##)
                .unwrap();
        let style = style.normalized();
        assert_eq!(style.background, OverlayBackground::Solid);
        assert_eq!(style.background_mode, OverlayBackgroundMode::Transparent);
        assert_eq!(style.background_opacity, 0.75);
    }

    #[test]
    fn new_overlay_style_defaults_to_current_secondary_display() {
        assert_eq!(OverlayStyleSettings::default().background_opacity, 0.6);
        assert_eq!(
            OverlayStyleSettings::default().background_mode,
            OverlayBackgroundMode::Solid
        );
        assert_eq!(
            OverlayStyleSettings::default().secondary_display,
            SecondaryDisplayMode::TranslationRomanization
        );
        assert_eq!(OverlayStyleSettings::default().secondary_font_scale, 1.0);
        assert!(!OverlayStyleSettings::default().auto_center_with_translation_or_romanization);
    }

    #[test]
    fn legacy_fill_migrates_and_manual_bounds_are_restored() {
        let style: OverlayStyleSettings = serde_json::from_str(
            r##"{"karaokeStyle":"fill","horizontalMaxWidth":640,"verticalMaxHeight":480}"##,
        )
        .unwrap();
        let style = style.normalized();
        assert_eq!(style.karaoke_style, KaraokeStyle::Sweep);
        assert_eq!(style.horizontal_max_width, Some(640.0));
        assert_eq!(style.vertical_max_height, Some(480.0));
        let serialized = serde_json::to_string(&style).unwrap();
        assert!(serialized.contains(r#""karaokeStyle":"sweep""#));
        assert!(serialized.contains(r#""horizontalMaxWidth":640.0"#));
        assert!(serialized.contains(r#""verticalMaxHeight":480.0"#));
    }

    #[test]
    fn reset_bounds_response_clears_only_manual_axes() {
        let mut style = OverlayStyleSettings {
            font_size: 46,
            horizontal_max_width: Some(920.0),
            vertical_max_height: Some(700.0),
            ..OverlayStyleSettings::default()
        };
        clear_manual_overlay_bounds(&mut style);

        assert_eq!(style.font_size, 46);
        assert_eq!(style.horizontal_max_width, None);
        assert_eq!(style.vertical_max_height, None);
        let response = serde_json::to_value(style).unwrap();
        assert_eq!(response["horizontalMaxWidth"], serde_json::Value::Null);
        assert_eq!(response["verticalMaxHeight"], serde_json::Value::Null);
    }

    #[test]
    fn reset_bounds_restores_horizontal_width_and_preserves_height() {
        assert_eq!(
            reset_overlay_dimensions(OverlayOrientation::Horizontal, 920.0, 184.0),
            (760.0, 184.0)
        );
        assert_eq!(
            reset_overlay_dimensions(OverlayOrientation::Horizontal, 920.0, 40.0),
            (760.0, 76.0)
        );
    }

    #[test]
    fn reset_bounds_restores_vertical_height_and_preserves_width() {
        assert_eq!(
            reset_overlay_dimensions(OverlayOrientation::Vertical, 260.0, 780.0),
            (260.0, 620.0)
        );
        assert_eq!(
            reset_overlay_dimensions(OverlayOrientation::Vertical, 120.0, 780.0),
            (190.0, 620.0)
        );
    }

    #[test]
    fn old_romanization_only_style_migrates_to_romanization_mode() {
        let style: OverlayStyleSettings =
            serde_json::from_str(r##"{"translationEnabled":false,"romanizationEnabled":true}"##)
                .unwrap();
        assert_eq!(
            style.normalized().secondary_display,
            SecondaryDisplayMode::Romanization
        );
    }

    fn search_result(score: f64, translation: bool, romanization: bool) -> LyricsSearchResult {
        LyricsSearchResult {
            id: format!("{score}"),
            provider_id: "test".into(),
            title: "歌曲".into(),
            artist: "歌手".into(),
            album: None,
            duration_ms: None,
            source: "测试".into(),
            synced: true,
            has_translation: translation,
            has_word_timing: false,
            has_romanization: romanization,
            score,
            lyrics: "[00:01.00]歌词".into(),
        }
    }

    #[test]
    fn translation_preference_wins_inside_quality_window() {
        let mut results = vec![
            search_result(0.91, false, false),
            search_result(0.88, true, false),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Translation);
        assert!(results[0].has_translation);
    }

    #[test]
    fn translation_preference_does_not_cross_quality_window() {
        let mut results = vec![
            search_result(0.91, false, false),
            search_result(0.86, true, false),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Translation);
        assert!(!results[0].has_translation);
    }

    #[test]
    fn combined_mode_prefers_results_with_both_tracks() {
        let mut results = vec![
            search_result(0.92, false, false),
            search_result(0.91, true, false),
            search_result(0.90, true, true),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::TranslationRomanization);
        assert!(results[0].has_translation && results[0].has_romanization);
    }

    fn word_timed_result(score: f64, word_timing: bool, translation: bool) -> LyricsSearchResult {
        let mut result = search_result(score, translation, false);
        result.has_word_timing = word_timing;
        result
    }

    #[test]
    fn word_timing_wins_inside_quality_window() {
        let mut results = vec![
            word_timed_result(0.91, false, false),
            word_timed_result(0.88, true, false),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Next);
        assert!(results[0].has_word_timing);
    }

    #[test]
    fn word_timing_does_not_cross_quality_window() {
        let mut results = vec![
            word_timed_result(0.91, false, false),
            word_timed_result(0.86, true, false),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Next);
        assert!(!results[0].has_word_timing);
    }

    #[test]
    fn word_timing_precedes_auxiliary_track_preference() {
        let mut results = vec![
            word_timed_result(0.92, false, true),
            word_timed_result(0.91, true, false),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Translation);
        assert!(results[0].has_word_timing);
    }

    #[test]
    fn auxiliary_preference_breaks_word_timing_ties() {
        let mut results = vec![
            word_timed_result(0.92, true, false),
            word_timed_result(0.91, true, true),
        ];
        prefer_candidate_capabilities(&mut results, SecondaryDisplayMode::Translation);
        assert!(results[0].has_translation);
    }

    #[test]
    fn overlay_style_normalizes_unsafe_ranges_and_empty_colors() {
        let style = OverlayStyleSettings {
            font_size: 200,
            active_color: String::new(),
            inactive_color: String::new(),
            opacity: -1.0,
            background_opacity: 2.0,
            background_blur: 100.0,
            secondary_font_scale: 0.1,
            solid_color: String::new(),
            horizontal_max_width: Some(80.0),
            vertical_max_height: Some(120.0),
            ..OverlayStyleSettings::default()
        }
        .normalized();
        assert_eq!(style.font_size, 72);
        assert_eq!(style.opacity, 0.2);
        assert_eq!(style.background_opacity, 1.0);
        assert_eq!(style.background_blur, 40.0);
        assert_eq!(style.secondary_font_scale, 0.35);
        assert_eq!(style.active_color, "#a3e635");
        assert_eq!(style.solid_color, "#171821");
        assert_eq!(style.horizontal_max_width, Some(320.0));
        assert_eq!(style.vertical_max_height, Some(280.0));
    }

    #[test]
    fn overlay_layout_and_orientation_serialize_independently() {
        for (layout, orientation, expected_layout, expected_orientation) in [
            (
                OverlayLayout::Single,
                OverlayOrientation::Horizontal,
                "single",
                "horizontal",
            ),
            (
                OverlayLayout::Double,
                OverlayOrientation::Horizontal,
                "double",
                "horizontal",
            ),
            (
                OverlayLayout::Single,
                OverlayOrientation::Vertical,
                "single",
                "vertical",
            ),
            (
                OverlayLayout::Double,
                OverlayOrientation::Vertical,
                "double",
                "vertical",
            ),
        ] {
            let style = OverlayStyleSettings {
                layout,
                orientation,
                ..OverlayStyleSettings::default()
            };
            let value = serde_json::to_value(style).unwrap();
            assert_eq!(value["layout"], expected_layout);
            assert_eq!(value["orientation"], expected_orientation);
        }
    }

    #[test]
    fn adaptive_bounds_keep_the_user_position_stable() {
        let (position, size) = fit_overlay_bounds(
            tauri::PhysicalPosition::new(500, 300),
            600.0,
            300.0,
            1.0,
            tauri::PhysicalPosition::new(0, 0),
            tauri::PhysicalSize::new(1920, 1080),
        );
        assert_eq!(size, tauri::PhysicalSize::new(600, 300));
        assert_eq!(position, tauri::PhysicalPosition::new(500, 300));
    }

    #[test]
    fn adaptive_bounds_allow_monitor_edges_and_respect_minimums() {
        let (large_position, large_size) = fit_overlay_bounds(
            tauri::PhysicalPosition::new(300, 200),
            2_000.0,
            2_000.0,
            1.0,
            tauri::PhysicalPosition::new(0, 0),
            tauri::PhysicalSize::new(1_000, 800),
        );
        assert_eq!(large_size, tauri::PhysicalSize::new(1_000, 800));
        assert_eq!(large_position, tauri::PhysicalPosition::new(0, 0));

        let (_, small_size) = fit_overlay_bounds(
            tauri::PhysicalPosition::new(300, 200),
            10.0,
            10.0,
            1.0,
            tauri::PhysicalPosition::new(0, 0),
            tauri::PhysicalSize::new(1_000, 800),
        );
        assert_eq!(small_size, tauri::PhysicalSize::new(190, 76));
    }

    #[test]
    fn manual_edge_resize_keeps_the_opposite_edge_anchored() {
        let monitor_position = tauri::PhysicalPosition::new(0, 0);
        let monitor_size = tauri::PhysicalSize::new(2880, 1800);
        let horizontal_position = tauri::PhysicalPosition::new(400, 500);
        let horizontal_size = tauri::PhysicalSize::new(800, 600);
        let (left_position, left_size) = resize_overlay_edge_bounds(
            horizontal_position,
            horizontal_size,
            OverlayResizeEdge::Left,
            500.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(left_position.x as i64 + left_size.width as i64, 1200);
        assert_eq!(left_size.width, 1000);
        let (right_position, right_size) = resize_overlay_edge_bounds(
            horizontal_position,
            horizontal_size,
            OverlayResizeEdge::Right,
            500.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(right_position.x, horizontal_position.x);
        assert_eq!(right_size.width, 1000);

        let vertical_position = tauri::PhysicalPosition::new(400, 500);
        let vertical_size = tauri::PhysicalSize::new(800, 1200);
        let (top_position, top_size) = resize_overlay_edge_bounds(
            vertical_position,
            vertical_size,
            OverlayResizeEdge::Top,
            400.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(top_position.y as i64 + top_size.height as i64, 1700);
        assert_eq!(top_size.height, 800);
        let (bottom_position, bottom_size) = resize_overlay_edge_bounds(
            vertical_position,
            vertical_size,
            OverlayResizeEdge::Bottom,
            400.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(bottom_position.y, vertical_position.y);
        assert_eq!(bottom_size.height, 800);
    }

    #[test]
    fn manual_edge_resize_respects_minimums_and_monitor_edges() {
        let position = tauri::PhysicalPosition::new(400, 300);
        let size = tauri::PhysicalSize::new(800, 700);
        let monitor_position = tauri::PhysicalPosition::new(0, 0);
        let monitor_size = tauri::PhysicalSize::new(2880, 1800);
        let (_, minimum) = resize_overlay_edge_bounds(
            position,
            size,
            OverlayResizeEdge::Right,
            10.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(minimum.width, 640);
        let (_, maximum) = resize_overlay_edge_bounds(
            position,
            size,
            OverlayResizeEdge::Right,
            10_000.0,
            0.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(position.x as i64 + maximum.width as i64, 2880);
    }

    #[test]
    fn manual_edge_resize_respects_toolbar_minimums() {
        let position = tauri::PhysicalPosition::new(400, 300);
        let size = tauri::PhysicalSize::new(800, 900);
        let monitor_position = tauri::PhysicalPosition::new(0, 0);
        let monitor_size = tauri::PhysicalSize::new(2880, 1800);
        let (_, horizontal) = resize_overlay_edge_bounds(
            position,
            size,
            OverlayResizeEdge::Right,
            10.0,
            380.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(horizontal.width, 760);

        let (_, vertical) = resize_overlay_edge_bounds(
            position,
            size,
            OverlayResizeEdge::Bottom,
            10.0,
            360.0,
            2.0,
            monitor_position,
            monitor_size,
        );
        assert_eq!(vertical.height, 720);
    }

    #[test]
    fn content_fit_cannot_override_the_fixed_layout_axis() {
        let horizontal = OverlayStyleSettings {
            layout: OverlayLayout::Double,
            orientation: OverlayOrientation::Horizontal,
            horizontal_max_width: Some(540.0),
            ..OverlayStyleSettings::default()
        };
        assert_eq!(
            fixed_axis_content_size(&horizontal, 1200.0, 180.0, 320.0, 280.0, false),
            (540.0, 180.0)
        );
        assert_eq!(
            fixed_axis_content_size(&horizontal, 1200.0, 180.0, 320.0, 280.0, true),
            (320.0, 180.0)
        );

        let vertical = OverlayStyleSettings {
            layout: OverlayLayout::Double,
            orientation: OverlayOrientation::Vertical,
            vertical_max_height: Some(430.0),
            ..OverlayStyleSettings::default()
        };
        assert_eq!(
            fixed_axis_content_size(&vertical, 220.0, 900.0, 320.0, 280.0, false),
            (220.0, 430.0)
        );
        assert_eq!(
            fixed_axis_content_size(&vertical, 220.0, 900.0, 320.0, 280.0, true),
            (220.0, 280.0)
        );
    }

    #[test]
    fn legacy_edge_alignments_migrate_to_distributed() {
        let left: OverlayAlignment = serde_json::from_str(r#""left""#).unwrap();
        let right: OverlayAlignment = serde_json::from_str(r#""right""#).unwrap();
        assert_eq!(left, OverlayAlignment::Distributed);
        assert_eq!(right, OverlayAlignment::Distributed);
        assert_eq!(
            serde_json::to_string(&OverlayAlignment::Distributed).unwrap(),
            r#""distributed""#
        );
    }

    #[test]
    fn resolves_a_macos_application_bundle() {
        let root =
            std::env::temp_dir().join(format!("lyrics-plus-app-resolver-{}", std::process::id()));
        let application = root.join("Example.app");
        std::fs::create_dir_all(application.join("Contents")).unwrap();
        std::fs::write(
            application.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>org.example.Player</string><key>CFBundleName</key><string>Example Player</string></dict></plist>"#,
        )
        .unwrap();
        let resolved = resolve_registered_application(&application).unwrap();
        assert_eq!(resolved.bundle_id, "org.example.Player");
        assert_eq!(resolved.name, "Example Player");
        assert_eq!(
            resolve_system_media_applications(vec![application.clone(), application.clone()])
                .unwrap()
                .len(),
            1
        );
        assert!(resolve_registered_application(&root).is_err());
        let missing_plist = root.join("Missing.app");
        std::fs::create_dir_all(&missing_plist).unwrap();
        assert!(resolve_registered_application(&missing_plist).is_err());
        let missing_bundle_id = root.join("NoId.app/Contents");
        std::fs::create_dir_all(&missing_bundle_id).unwrap();
        std::fs::write(
            missing_bundle_id.join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleName</key><string>No ID</string></dict></plist>"#,
        )
        .unwrap();
        assert!(resolve_registered_application(&root.join("NoId.app")).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn application_icons_are_small_pngs_and_skip_missing_apps() {
        use base64::Engine as _;

        let icon = application_icon_at_path("/System/Applications/Music.app")
            .expect("Music.app should have a readable native icon");
        let icons = collect_application_icons(vec!["invalid.lyrics-plus.icon-test".into()]);
        let png = base64::engine::general_purpose::STANDARD
            .decode(icon.split_once(',').unwrap().1)
            .unwrap();

        assert!(icon.starts_with("data:image/png;base64,iVBORw0KGgo"));
        assert_eq!(&png[16..24], &[0, 0, 0, 64, 0, 0, 0, 64]);
        assert!(icons.is_empty());
    }
}
