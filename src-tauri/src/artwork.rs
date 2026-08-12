use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, GenericImageView, ImageReader};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use strsim::normalized_levenshtein;
use tokio::sync::Mutex;

use crate::config::{
    is_dedicated_player_bundle_id, normalize_registered_application, RegisteredApplication,
};
use crate::player::automation::{
    export_apple_music_artwork, spotify_artwork_url, AppleMusicArtworkExport,
};
use crate::player::{PlaybackSnapshot, PlayerKind};

pub const CACHE_DIRECTORY_PREFERENCE: &str = "artwork.cache_directory";
const MAX_SOURCE_BYTES: usize = 10 * 1024 * 1024;
const MAX_ARTWORK_DIMENSION: u32 = 384;
const MIN_CLEAR_DIMENSION: u32 = 256;
const JPEG_QUALITY: u8 = 85;
const MAX_CACHE_FILES: usize = 200;
const MISSING_ARTWORK_TTL: Duration = Duration::from_secs(5 * 60);
const NETWORK_MISSING_TTL_SECS: u64 = 24 * 60 * 60;
const NETWORK_MATCH_VERSION: &str = "v2";
const ITUNES_REQUEST_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkProviderPreference {
    pub id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ItunesStorefront {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "CN")]
    Cn,
    #[serde(rename = "TW")]
    Tw,
    #[serde(rename = "HK")]
    Hk,
    #[serde(rename = "US")]
    Us,
}

impl ItunesStorefront {
    pub fn effective_country(self, automatic_country: &str) -> Result<&str, String> {
        match self {
            Self::Auto => validate_itunes_country(automatic_country),
            Self::Cn => Ok("CN"),
            Self::Tw => Ok("TW"),
            Self::Hk => Ok("HK"),
            Self::Us => Ok("US"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct ArtworkSettings {
    pub network_fallback: bool,
    pub itunes_storefront: ItunesStorefront,
    pub always_network_applications: Vec<RegisteredApplication>,
    pub providers: Vec<ArtworkProviderPreference>,
}

impl Default for ArtworkSettings {
    fn default() -> Self {
        Self {
            network_fallback: false,
            itunes_storefront: ItunesStorefront::Auto,
            always_network_applications: Vec::new(),
            providers: default_providers(),
        }
    }
}

pub fn validate_itunes_country(country: &str) -> Result<&str, String> {
    ["CN", "TW", "HK", "US"]
        .contains(&country)
        .then_some(country)
        .ok_or_else(|| format!("不支持的 iTunes 商店地区：{country}"))
}

pub fn normalize_settings(settings: &mut ArtworkSettings) -> Result<(), String> {
    let mut bundle_ids = HashSet::new();
    let mut applications = Vec::new();
    for application in std::mem::take(&mut settings.always_network_applications) {
        let application = normalize_registered_application(application)?;
        if is_dedicated_player_bundle_id(&application.bundle_id) {
            return Err("Apple Music 和 Spotify 使用专用通道，不能强制使用网络封面".into());
        }
        if bundle_ids.insert(application.bundle_id.clone()) {
            applications.push(application);
        }
    }
    settings.always_network_applications = applications;

    let valid = ["cover_art_archive", "itunes"];
    let mut seen = HashSet::new();
    for provider in &settings.providers {
        if !valid.contains(&provider.id.as_str()) {
            return Err(format!("不支持的封面来源：{}", provider.id));
        }
        if !seen.insert(provider.id.clone()) {
            return Err(format!("封面来源重复：{}", provider.id));
        }
    }
    for provider in default_providers() {
        if !seen.contains(&provider.id) {
            settings.providers.push(provider);
        }
    }
    Ok(())
}

fn default_providers() -> Vec<ArtworkProviderPreference> {
    ["cover_art_archive", "itunes"]
        .into_iter()
        .map(|id| ArtworkProviderPreference {
            id: id.into(),
            enabled: true,
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkProviderStatus {
    pub provider_id: String,
    pub name: String,
    pub available: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkSettingsView {
    pub settings: ArtworkSettings,
    pub statuses: Vec<ArtworkProviderStatus>,
}

impl ArtworkSettingsView {
    pub fn new(settings: ArtworkSettings) -> Self {
        Self {
            settings,
            statuses: default_providers()
                .into_iter()
                .map(|provider| provider_status(&provider.id, false, None))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkCacheStatus {
    pub directory: String,
    pub file_count: u64,
    pub total_bytes: u64,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkAsset {
    pub player: PlayerKind,
    pub track_id: String,
    pub file_path: String,
    pub source: String,
    pub source_link: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct CacheIndex {
    tracks: HashMap<String, CacheReference>,
    albums: HashMap<String, String>,
    network_ids: HashMap<String, String>,
    blobs: HashMap<String, BlobMetadata>,
    missing: HashMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheReference {
    hash: String,
    low_quality: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlobMetadata {
    source: String,
    source_link: Option<String>,
    width: u32,
    height: u32,
    last_accessed: u64,
}

#[derive(Default)]
struct MissingArtworkCache(HashMap<String, Instant>);

impl MissingArtworkCache {
    fn contains_recent(&mut self, key: &str, now: Instant) -> bool {
        self.0
            .retain(|_, at| now.saturating_duration_since(*at) < MISSING_ARTWORK_TTL);
        self.0.contains_key(key)
    }

    fn record(&mut self, key: &str, result: AppleMusicArtworkExport, now: Instant) {
        match result {
            AppleMusicArtworkExport::Missing => {
                self.0.insert(key.into(), now);
            }
            AppleMusicArtworkExport::Exported => {
                self.0.remove(key);
            }
            AppleMusicArtworkExport::Stale | AppleMusicArtworkExport::Unavailable => {}
        }
    }
}

struct ArtworkSource {
    bytes: Vec<u8>,
    source: String,
    source_link: Option<String>,
    network_id: Option<String>,
}

pub struct ArtworkService {
    cache_dir: RwLock<PathBuf>,
    warning: RwLock<Option<String>>,
    operation_lock: Mutex<()>,
    missing_artwork: Mutex<MissingArtworkCache>,
    last_itunes_request: Mutex<Option<Instant>>,
}

impl ArtworkService {
    pub fn new(cache_dir: PathBuf) -> std::io::Result<Self> {
        initialize_directory(&cache_dir)?;
        Ok(Self {
            cache_dir: RwLock::new(cache_dir),
            warning: RwLock::new(None),
            operation_lock: Mutex::new(()),
            missing_artwork: Mutex::new(MissingArtworkCache::default()),
            last_itunes_request: Mutex::new(None),
        })
    }

    pub fn cache_directory(&self) -> PathBuf {
        self.cache_dir
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn set_warning(&self, warning: Option<String>) {
        *self
            .warning
            .write()
            .unwrap_or_else(|error| error.into_inner()) = warning;
    }

    pub async fn set_directory(&self, path: PathBuf) -> Result<ArtworkCacheStatus, String> {
        let _guard = self.operation_lock.lock().await;
        initialize_directory(&path).map_err(|error| format!("无法使用封面缓存目录：{error}"))?;
        verify_writable(&path)?;
        *self
            .cache_dir
            .write()
            .unwrap_or_else(|error| error.into_inner()) = path;
        self.set_warning(None);
        self.cache_status()
    }

    pub fn cache_status(&self) -> Result<ArtworkCacheStatus, String> {
        let directory = self.cache_directory();
        let (file_count, total_bytes) = fs::read_dir(directory.join("blobs"))
            .map_err(|error| format!("读取封面缓存失败：{error}"))?
            .flatten()
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                (metadata.is_file()
                    && entry.path().extension().and_then(|value| value.to_str()) == Some("jpg"))
                .then_some(metadata.len())
            })
            .fold((0, 0), |(count, bytes), size| (count + 1, bytes + size));
        Ok(ArtworkCacheStatus {
            directory: directory.to_string_lossy().into_owned(),
            file_count,
            total_bytes,
            warning: self
                .warning
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone(),
        })
    }

    pub async fn clear(&self) -> Result<ArtworkCacheStatus, String> {
        let _guard = self.operation_lock.lock().await;
        let directory = self.cache_directory();
        let blobs = directory.join("blobs");
        if blobs.exists() {
            fs::remove_dir_all(&blobs).map_err(|error| format!("清空封面缓存失败：{error}"))?;
        }
        fs::create_dir_all(&blobs).map_err(|error| format!("重建封面缓存失败：{error}"))?;
        for name in ["index.json", ".index.tmp"] {
            let path = directory.join(name);
            if path.exists() {
                fs::remove_file(path).map_err(|error| format!("清空封面索引失败：{error}"))?;
            }
        }
        self.cache_status()
    }

    pub async fn resolve(
        &self,
        snapshot: &PlaybackSnapshot,
        http: &reqwest::Client,
        settings: &ArtworkSettings,
        system_image: Option<DynamicImage>,
        allow_network: bool,
        itunes_country: &str,
    ) -> Result<Option<ArtworkAsset>, String> {
        let itunes_country = settings
            .itunes_storefront
            .effective_country(itunes_country)?;
        let (Some(player), Some(track_id)) = (snapshot.player, snapshot.track_id.as_deref()) else {
            return Ok(None);
        };
        if track_id.trim().is_empty() {
            return Ok(None);
        }

        let _guard = self.operation_lock.lock().await;
        let directory = self.cache_directory();
        let mut index = load_index(&directory);
        let track_key = format!("{}:{track_id}", player_name(player));
        let force_network = settings.network_fallback
            && player == PlayerKind::System
            && snapshot
                .source_app_bundle_id
                .as_deref()
                .is_some_and(|bundle_id| {
                    settings
                        .always_network_applications
                        .iter()
                        .any(|application| application.bundle_id == bundle_id)
                });
        let cached = cached_asset(&directory, &mut index, player, track_id, &track_key);
        if cached
            .as_ref()
            .is_some_and(|(asset, low)| !low && (!force_network || asset.source != "player"))
        {
            save_index(&directory, &index)?;
            return Ok(cached.map(|(asset, _)| asset));
        }

        if cached.is_some() && (!settings.network_fallback || !allow_network) {
            save_index(&directory, &index)?;
            if player == PlayerKind::System && !allow_network {
                return Ok(None);
            }
            return Ok(cached.map(|(asset, _)| asset));
        }

        let mut fallback = cached.map(|(asset, _)| asset);
        if fallback.is_none() {
            if let Some(source) = self
                .player_source(snapshot, http, system_image, &track_key)
                .await?
            {
                let (width, height) = image_dimensions(&source.bytes)?;
                let low_quality = width.min(height) < MIN_CLEAR_DIMENSION;
                let asset = store_source(
                    &directory,
                    &mut index,
                    player,
                    track_id,
                    snapshot,
                    source,
                    low_quality,
                )?;
                if (!low_quality && !force_network) || !settings.network_fallback || !allow_network
                {
                    prune_cache(&directory, &mut index, MAX_CACHE_FILES);
                    save_index(&directory, &index)?;
                    if player == PlayerKind::System
                        && !allow_network
                        && (low_quality || force_network)
                    {
                        return Ok(None);
                    }
                    return Ok(Some(asset));
                }
                fallback = Some(asset);
            }
        }

        let album_key = album_key(snapshot);
        let album_hash = album_key
            .as_ref()
            .and_then(|key| index.albums.get(key))
            .cloned();
        if let Some(asset) =
            album_hash.and_then(|hash| blob_asset(&directory, &mut index, player, track_id, hash))
        {
            let hash = hash_from_path(&asset.file_path).unwrap_or_default();
            index.tracks.insert(
                track_key.clone(),
                CacheReference {
                    hash,
                    low_quality: false,
                },
            );
            if !force_network || asset.source != "player" {
                save_index(&directory, &index)?;
                return Ok(Some(asset));
            }
            fallback.get_or_insert(asset);
        }

        if !settings.network_fallback || !allow_network {
            save_index(&directory, &index)?;
            return Ok(fallback);
        }

        let missing_key =
            network_missing_key(itunes_country, album_key.as_deref().unwrap_or(&track_key));
        if index
            .missing
            .get(&missing_key)
            .is_some_and(|at| now_secs().saturating_sub(*at) < NETWORK_MISSING_TTL_SECS)
        {
            return Ok(fallback);
        }

        let mut temporary_error = false;
        for provider in settings
            .providers
            .iter()
            .filter(|provider| provider.enabled)
        {
            let result = match provider.id.as_str() {
                "cover_art_archive" => search_cover_art_archive(http, snapshot).await,
                "itunes" => self.search_itunes(http, snapshot, itunes_country).await,
                _ => continue,
            };
            match result {
                Ok(Some(source)) => {
                    let asset = store_source(
                        &directory, &mut index, player, track_id, snapshot, source, false,
                    )?;
                    index.missing.remove(&missing_key);
                    prune_cache(&directory, &mut index, MAX_CACHE_FILES);
                    save_index(&directory, &index)?;
                    return Ok(Some(asset));
                }
                Ok(None) => {}
                Err(error) => {
                    temporary_error = true;
                    log::warn!("Artwork provider {} failed: {error}", provider.id);
                }
            }
        }
        if !temporary_error {
            index.missing.insert(missing_key, now_secs());
            save_index(&directory, &index)?;
        }
        Ok(fallback)
    }

    async fn player_source(
        &self,
        snapshot: &PlaybackSnapshot,
        http: &reqwest::Client,
        system_image: Option<DynamicImage>,
        track_key: &str,
    ) -> Result<Option<ArtworkSource>, String> {
        let title = snapshot.title.clone().unwrap_or_default();
        let artist = snapshot.artist.clone().unwrap_or_default();
        let album = snapshot.album.clone().unwrap_or_default();
        match snapshot.player.expect("player checked") {
            PlayerKind::System => system_image
                .map(|image| dynamic_image_bytes(image).map(|bytes| source(bytes, "player", None)))
                .transpose(),
            PlayerKind::Spotify => {
                let expected_id = snapshot.track_id.clone().unwrap_or_default();
                let url = tauri::async_runtime::spawn_blocking(move || {
                    spotify_artwork_url(&expected_id, &title, &artist, &album)
                })
                .await
                .map_err(|error| format!("Spotify 封面任务失败：{error}"))??;
                match url {
                    Some(url) => Ok(Some(source(
                        download_artwork(http, &url).await?,
                        "player",
                        None,
                    ))),
                    None => Ok(None),
                }
            }
            PlayerKind::AppleMusic => {
                if self
                    .missing_artwork
                    .lock()
                    .await
                    .contains_recent(track_key, Instant::now())
                {
                    return Ok(None);
                }
                let raw_path = self.temp_path("apple-music");
                let expected_id = snapshot.track_id.clone().unwrap_or_default();
                let export_path = raw_path.clone();
                let result = tauri::async_runtime::spawn_blocking(move || {
                    export_apple_music_artwork(&expected_id, &title, &artist, &album, &export_path)
                })
                .await
                .map_err(|error| format!("Apple Music 封面任务失败：{error}"))??;
                self.missing_artwork
                    .lock()
                    .await
                    .record(track_key, result, Instant::now());
                if result != AppleMusicArtworkExport::Exported {
                    let _ = fs::remove_file(raw_path);
                    return Ok(None);
                }
                let bytes = fs::read(&raw_path)
                    .map_err(|error| format!("读取 Apple Music 封面失败：{error}"));
                let _ = fs::remove_file(raw_path);
                Ok(Some(source(bytes?, "player", None)))
            }
        }
    }

    async fn search_itunes(
        &self,
        http: &reqwest::Client,
        snapshot: &PlaybackSnapshot,
        country: &str,
    ) -> Result<Option<ArtworkSource>, String> {
        let mut last = self.last_itunes_request.lock().await;
        if let Some(wait) = last.and_then(|at| ITUNES_REQUEST_INTERVAL.checked_sub(at.elapsed())) {
            tokio::time::sleep(wait).await;
        }
        *last = Some(Instant::now());
        drop(last);
        search_itunes(http, snapshot, country).await
    }

    pub async fn test_provider(
        &self,
        http: &reqwest::Client,
        provider_id: &str,
        itunes_country: &str,
    ) -> ArtworkProviderStatus {
        let itunes_country = match validate_itunes_country(itunes_country) {
            Ok(country) => country,
            Err(error) => return provider_status(provider_id, false, Some(error)),
        };
        let result = match provider_id {
            "cover_art_archive" => {
                http.get("https://musicbrainz.org/ws/2/release-group/")
                    .query(&[
                        ("query", "releasegroup:test"),
                        ("fmt", "json"),
                        ("limit", "1"),
                    ])
                    .send()
                    .await
            }
            "itunes" => {
                http.get("https://itunes.apple.com/search")
                    .query(&[
                        ("term", "test"),
                        ("country", itunes_country),
                        ("media", "music"),
                        ("limit", "1"),
                    ])
                    .send()
                    .await
            }
            _ => return provider_status(provider_id, false, Some("未知封面源".into())),
        };
        match result.and_then(reqwest::Response::error_for_status) {
            Ok(_) => provider_status(provider_id, true, None),
            Err(error) => provider_status(provider_id, false, Some(error.to_string())),
        }
    }

    fn temp_path(&self, kind: &str) -> PathBuf {
        self.cache_directory().join(format!(
            ".{kind}-{}-{}.tmp",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }
}

fn provider_status(id: &str, available: bool, message: Option<String>) -> ArtworkProviderStatus {
    ArtworkProviderStatus {
        provider_id: id.into(),
        name: match id {
            "cover_art_archive" => "Cover Art Archive",
            "itunes" => "iTunes",
            _ => id,
        }
        .into(),
        available,
        message,
    }
}

pub(crate) fn player_name(player: PlayerKind) -> &'static str {
    match player {
        PlayerKind::AppleMusic => "apple_music",
        PlayerKind::Spotify => "spotify",
        PlayerKind::System => "system",
    }
}

fn initialize_directory(directory: &Path) -> std::io::Result<()> {
    fs::create_dir_all(directory.join("blobs"))?;
    for entry in fs::read_dir(directory)?.flatten() {
        let path = entry.path();
        if path.parent() == Some(directory)
            && path.extension().and_then(|value| value.to_str()) == Some("jpg")
        {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn verify_writable(directory: &Path) -> Result<(), String> {
    let path = directory.join(".write-test");
    fs::write(&path, b"ok").map_err(|error| format!("所选封面目录不可写：{error}"))?;
    fs::remove_file(path).map_err(|error| format!("清理目录写入测试失败：{error}"))
}

fn load_index(directory: &Path) -> CacheIndex {
    fs::read(directory.join("index.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_index(directory: &Path, index: &CacheIndex) -> Result<(), String> {
    let bytes =
        serde_json::to_vec_pretty(index).map_err(|error| format!("序列化封面索引失败：{error}"))?;
    write_atomically(
        &directory.join("index.json"),
        &directory.join(".index.tmp"),
        &bytes,
    )
}

fn cached_asset(
    directory: &Path,
    index: &mut CacheIndex,
    player: PlayerKind,
    track_id: &str,
    track_key: &str,
) -> Option<(ArtworkAsset, bool)> {
    let reference = index.tracks.get(track_key)?.clone();
    blob_asset(directory, index, player, track_id, reference.hash)
        .map(|asset| (asset, reference.low_quality))
}

fn blob_asset(
    directory: &Path,
    index: &mut CacheIndex,
    player: PlayerKind,
    track_id: &str,
    hash: String,
) -> Option<ArtworkAsset> {
    let path = directory.join("blobs").join(format!("{hash}.jpg"));
    if !is_nonempty_file(&path) {
        index.blobs.remove(&hash);
        return None;
    }
    let metadata = index.blobs.get_mut(&hash)?;
    metadata.last_accessed = now_secs();
    Some(ArtworkAsset {
        player,
        track_id: track_id.into(),
        file_path: path.to_string_lossy().into_owned(),
        source: metadata.source.clone(),
        source_link: metadata.source_link.clone(),
    })
}

fn store_source(
    directory: &Path,
    index: &mut CacheIndex,
    player: PlayerKind,
    track_id: &str,
    snapshot: &PlaybackSnapshot,
    source: ArtworkSource,
    low_quality: bool,
) -> Result<ArtworkAsset, String> {
    let (width, height) = image_dimensions(&source.bytes)?;
    let normalized = normalize_image(&source.bytes)?;
    let hash = hex_digest(&normalized);
    let path = directory.join("blobs").join(format!("{hash}.jpg"));
    if !is_nonempty_file(&path) {
        write_atomically(&path, &directory.join(format!(".{hash}.tmp")), &normalized)?;
    }
    index.blobs.insert(
        hash.clone(),
        BlobMetadata {
            source: source.source.clone(),
            source_link: source.source_link.clone(),
            width,
            height,
            last_accessed: now_secs(),
        },
    );
    index.tracks.insert(
        format!("{}:{track_id}", player_name(player)),
        CacheReference {
            hash: hash.clone(),
            low_quality,
        },
    );
    if !low_quality {
        if let Some(key) = album_key(snapshot) {
            index.albums.insert(key, hash.clone());
        }
        if let Some(id) = source.network_id {
            index.network_ids.insert(id, hash.clone());
        }
    }
    Ok(ArtworkAsset {
        player,
        track_id: track_id.into(),
        file_path: path.to_string_lossy().into_owned(),
        source: source.source,
        source_link: source.source_link,
    })
}

fn prune_cache(directory: &Path, index: &mut CacheIndex, keep: usize) {
    if index.blobs.len() <= keep {
        return;
    }
    let mut blobs = index
        .blobs
        .iter()
        .map(|(hash, metadata)| (hash.clone(), metadata.last_accessed))
        .collect::<Vec<_>>();
    blobs.sort_by_key(|(_, last_accessed)| std::cmp::Reverse(*last_accessed));
    let retained = blobs
        .into_iter()
        .take(keep)
        .map(|(hash, _)| hash)
        .collect::<HashSet<_>>();
    for hash in index
        .blobs
        .keys()
        .filter(|hash| !retained.contains(*hash))
        .cloned()
        .collect::<Vec<_>>()
    {
        let _ = fs::remove_file(directory.join("blobs").join(format!("{hash}.jpg")));
        index.blobs.remove(&hash);
    }
    index
        .tracks
        .retain(|_, reference| retained.contains(&reference.hash));
    index.albums.retain(|_, hash| retained.contains(hash));
    index.network_ids.retain(|_, hash| retained.contains(hash));
}

fn album_key(snapshot: &PlaybackSnapshot) -> Option<String> {
    let artist = normalize_text(snapshot.artist.as_deref()?);
    let album = normalize_text(snapshot.album.as_deref()?);
    (!artist.is_empty() && !album.is_empty()).then(|| format!("{artist}|{album}"))
}

fn network_missing_key(itunes_country: &str, identity: &str) -> String {
    format!("{NETWORK_MATCH_VERSION}:{itunes_country}:{identity}")
}

fn normalize_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn hash_from_path(path: &str) -> Option<String> {
    Path::new(path).file_stem()?.to_str().map(str::to_owned)
}

fn source(bytes: Vec<u8>, name: &str, source_link: Option<String>) -> ArtworkSource {
    ArtworkSource {
        bytes,
        source: name.into(),
        source_link,
        network_id: None,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct MusicBrainzResponse {
    release_groups: Vec<MusicBrainzReleaseGroup>,
}

#[derive(Deserialize)]
struct MusicBrainzReleaseGroup {
    id: String,
    title: String,
    score: u8,
    #[serde(rename = "artist-credit")]
    artist_credit: Vec<MusicBrainzArtistCredit>,
}

#[derive(Deserialize)]
struct MusicBrainzArtistCredit {
    name: String,
}

async fn search_cover_art_archive(
    http: &reqwest::Client,
    snapshot: &PlaybackSnapshot,
) -> Result<Option<ArtworkSource>, String> {
    let (Some(artist), Some(album)) = (snapshot.artist.as_deref(), snapshot.album.as_deref())
    else {
        return Ok(None);
    };
    let query = format!("releasegroup:\"{album}\" AND artist:\"{artist}\"");
    let response = http
        .get("https://musicbrainz.org/ws/2/release-group/")
        .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "5")])
        .send()
        .await
        .map_err(|error| format!("MusicBrainz 请求失败：{error}"))?
        .error_for_status()
        .map_err(|error| format!("MusicBrainz 请求失败：{error}"))?
        .json::<MusicBrainzResponse>()
        .await
        .map_err(|error| format!("MusicBrainz 响应无效：{error}"))?;
    let expected_artist = normalize_text(artist);
    let expected_album = normalize_text(album);
    let Some(group) = response.release_groups.into_iter().find(|group| {
        group.score >= 80
            && normalize_text(&group.title) == expected_album
            && normalize_text(
                &group
                    .artist_credit
                    .iter()
                    .map(|credit| credit.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            ) == expected_artist
    }) else {
        return Ok(None);
    };
    let url = format!(
        "https://coverartarchive.org/release-group/{}/front-500",
        group.id
    );
    let bytes = match download_artwork(http, &url).await {
        Ok(bytes) => bytes,
        Err(error) if error.contains("404") => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(Some(ArtworkSource {
        bytes,
        source: "cover_art_archive".into(),
        source_link: Some(format!(
            "https://musicbrainz.org/release-group/{}",
            group.id
        )),
        network_id: Some(format!("cover_art_archive:{}", group.id)),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesResponse {
    results: Vec<ItunesTrack>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesTrack {
    track_name: String,
    artist_name: String,
    collection_name: Option<String>,
    collection_id: Option<u64>,
    artwork_url100: Option<String>,
    collection_view_url: Option<String>,
    track_time_millis: Option<u64>,
}

async fn search_itunes(
    http: &reqwest::Client,
    snapshot: &PlaybackSnapshot,
    country: &str,
) -> Result<Option<ArtworkSource>, String> {
    let (Some(title), Some(artist)) = (snapshot.title.as_deref(), snapshot.artist.as_deref())
    else {
        return Ok(None);
    };
    let term = format!("{title} {artist}");
    let response = http
        .get("https://itunes.apple.com/search")
        .query(&[
            ("term", term.as_str()),
            ("country", country),
            ("media", "music"),
            ("entity", "song"),
            ("limit", "20"),
        ])
        .send()
        .await
        .map_err(|error| format!("iTunes 请求失败：{error}"))?
        .error_for_status()
        .map_err(|error| format!("iTunes 请求失败：{error}"))?
        .json::<ItunesResponse>()
        .await
        .map_err(|error| format!("iTunes 响应无效：{error}"))?;
    let Some(track) = response
        .results
        .into_iter()
        .filter_map(|track| itunes_match_score(snapshot, &track).map(|score| (score, track)))
        .max_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, track)| track)
    else {
        return Ok(None);
    };
    let Some(url) = track
        .artwork_url100
        .map(|url| url.replace("100x100", "600x600"))
    else {
        return Ok(None);
    };
    Ok(Some(ArtworkSource {
        bytes: download_artwork(http, &url).await?,
        source: "itunes".into(),
        source_link: track.collection_view_url,
        network_id: track.collection_id.map(|id| format!("itunes:{id}")),
    }))
}

fn itunes_match_score(snapshot: &PlaybackSnapshot, track: &ItunesTrack) -> Option<f64> {
    let title = normalized_levenshtein(
        &normalize_text(snapshot.title.as_deref()?),
        &normalize_text(&track.track_name),
    );
    let artist = normalized_levenshtein(
        &normalize_text(snapshot.artist.as_deref()?),
        &normalize_text(&track.artist_name),
    );
    if title < 0.3 || artist < 0.6 {
        return None;
    }
    let album = match (snapshot.album.as_deref(), track.collection_name.as_deref()) {
        (Some(expected), Some(actual)) => {
            normalized_levenshtein(&normalize_text(expected), &normalize_text(actual))
        }
        _ => 0.6,
    };
    let duration = match (snapshot.duration_ms, track.track_time_millis) {
        (Some(expected), Some(actual)) => {
            (1.0 - expected.abs_diff(actual) as f64 / 15_000.0).clamp(0.0, 1.0)
        }
        _ => 0.6,
    };
    let score = title * 0.35 + artist * 0.35 + album * 0.1 + duration * 0.2;
    (score >= 0.55).then_some(score)
}

async fn download_artwork(http: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let parsed = reqwest::Url::parse(url).map_err(|error| format!("封面地址无效：{error}"))?;
    if parsed.scheme() != "https" {
        return Err("封面地址必须使用 HTTPS".into());
    }
    let response = http
        .get(parsed)
        .send()
        .await
        .map_err(|error| format!("下载封面失败：{error}"))?
        .error_for_status()
        .map_err(|error| format!("下载封面失败：{error}"))?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SOURCE_BYTES as u64)
    {
        return Err("封面超过 10 MB".into());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取封面失败：{error}"))?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err("封面超过 10 MB".into());
    }
    Ok(bytes.to_vec())
}

fn image_dimensions(source: &[u8]) -> Result<(u32, u32), String> {
    let image = decode_image(source)?;
    Ok(image.dimensions())
}

fn decode_image(source: &[u8]) -> Result<DynamicImage, String> {
    if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
        return Err("封面内容为空或超过 10 MB".into());
    }
    ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|error| format!("无法识别封面格式：{error}"))?
        .decode()
        .map_err(|error| format!("无法解码封面：{error}"))
}

fn normalize_image(source: &[u8]) -> Result<Vec<u8>, String> {
    normalize_dynamic_image(decode_image(source)?)
}

fn dynamic_image_bytes(image: DynamicImage) -> Result<Vec<u8>, String> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|error| format!("无法读取播放器封面：{error}"))?;
    Ok(bytes.into_inner())
}

fn normalize_dynamic_image(image: DynamicImage) -> Result<Vec<u8>, String> {
    let image = image
        .thumbnail(MAX_ARTWORK_DIMENSION, MAX_ARTWORK_DIMENSION)
        .to_rgb8();
    let mut output = Vec::new();
    JpegEncoder::new_with_quality(&mut output, JPEG_QUALITY)
        .encode_image(&image)
        .map_err(|error| format!("无法编码封面：{error}"))?;
    Ok(output)
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_atomically(final_path: &Path, temp_path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(temp_path, bytes).map_err(|error| format!("写入封面缓存失败：{error}"))?;
    if let Err(error) = fs::rename(temp_path, final_path) {
        let _ = fs::remove_file(temp_path);
        return Err(format!("提交封面缓存失败：{error}"));
    }
    Ok(())
}

fn is_nonempty_file(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, RgbImage};
    use tempfile::tempdir;

    fn sample_png(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(
            width,
            height,
            image::Rgb([80, 120, 180]),
        ));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn normalizes_and_hashes_identical_images_once() {
        let root = tempdir().unwrap();
        let directory = root.path();
        initialize_directory(directory).unwrap();
        let mut index = CacheIndex::default();
        let snapshot = PlaybackSnapshot {
            artist: Some("Artist".into()),
            album: Some("Album".into()),
            ..Default::default()
        };
        for track in ["one", "two"] {
            store_source(
                directory,
                &mut index,
                PlayerKind::Spotify,
                track,
                &snapshot,
                source(sample_png(900, 600), "player", None),
                false,
            )
            .unwrap();
        }
        assert_eq!(index.blobs.len(), 1);
        assert_eq!(fs::read_dir(directory.join("blobs")).unwrap().count(), 1);
    }

    #[test]
    fn detects_low_resolution_before_normalizing() {
        assert!(image_dimensions(&sample_png(200, 400)).unwrap().0 < MIN_CLEAR_DIMENSION);
        assert_eq!(image_dimensions(&sample_png(900, 600)).unwrap(), (900, 600));
    }

    #[tokio::test]
    async fn clear_preserves_unmanaged_files() {
        let root = tempdir().unwrap();
        let service = ArtworkService::new(root.path().to_path_buf()).unwrap();
        fs::write(root.path().join("keep.txt"), b"keep").unwrap();
        fs::write(root.path().join("blobs/a.jpg"), b"image").unwrap();
        service.clear().await.unwrap();
        assert!(root.path().join("keep.txt").exists());
        assert!(!root.path().join("blobs/a.jpg").exists());
    }

    #[tokio::test]
    async fn system_low_resolution_waits_until_network_attempt() {
        let root = tempdir().unwrap();
        let service = ArtworkService::new(root.path().to_path_buf()).unwrap();
        let snapshot = PlaybackSnapshot {
            player: Some(PlayerKind::System),
            track_id: Some("track".into()),
            ..PlaybackSnapshot::default()
        };
        let image = DynamicImage::new_rgb8(128, 128);
        let settings = ArtworkSettings::default();

        assert!(service
            .resolve(
                &snapshot,
                &reqwest::Client::new(),
                &settings,
                Some(image),
                false,
                "US",
            )
            .await
            .unwrap()
            .is_none());
        assert!(service
            .resolve(
                &snapshot,
                &reqwest::Client::new(),
                &settings,
                None,
                true,
                "US",
            )
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn selected_system_app_waits_then_falls_back_to_clear_player_artwork() {
        let root = tempdir().unwrap();
        let service = ArtworkService::new(root.path().to_path_buf()).unwrap();
        let snapshot = PlaybackSnapshot {
            player: Some(PlayerKind::System),
            track_id: Some("forced-track".into()),
            source_app_bundle_id: Some("com.example.music".into()),
            ..PlaybackSnapshot::default()
        };
        let mut settings = ArtworkSettings {
            network_fallback: true,
            always_network_applications: vec![RegisteredApplication {
                name: "Example Music".into(),
                bundle_id: "com.example.music".into(),
            }],
            ..ArtworkSettings::default()
        };
        for provider in &mut settings.providers {
            provider.enabled = false;
        }

        assert!(service
            .resolve(
                &snapshot,
                &reqwest::Client::new(),
                &settings,
                Some(DynamicImage::new_rgb8(512, 512)),
                false,
                "US",
            )
            .await
            .unwrap()
            .is_none());
        let fallback = service
            .resolve(
                &snapshot,
                &reqwest::Client::new(),
                &settings,
                None,
                true,
                "US",
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fallback.source, "player");
    }

    #[tokio::test]
    async fn disabled_network_fallback_ignores_selected_app() {
        let root = tempdir().unwrap();
        let service = ArtworkService::new(root.path().to_path_buf()).unwrap();
        let snapshot = PlaybackSnapshot {
            player: Some(PlayerKind::System),
            track_id: Some("disabled-track".into()),
            source_app_bundle_id: Some("com.example.music".into()),
            ..PlaybackSnapshot::default()
        };
        let settings = ArtworkSettings {
            always_network_applications: vec![RegisteredApplication {
                name: "Example Music".into(),
                bundle_id: "com.example.music".into(),
            }],
            ..ArtworkSettings::default()
        };

        let asset = service
            .resolve(
                &snapshot,
                &reqwest::Client::new(),
                &settings,
                Some(DynamicImage::new_rgb8(512, 512)),
                false,
                "US",
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(asset.source, "player");
    }

    #[test]
    fn artwork_applications_are_deduplicated_and_reject_dedicated_players() {
        let application = RegisteredApplication {
            name: " Example ".into(),
            bundle_id: "com.example.music".into(),
        };
        let mut settings = ArtworkSettings {
            always_network_applications: vec![application.clone(), application],
            ..ArtworkSettings::default()
        };
        normalize_settings(&mut settings).unwrap();
        assert_eq!(settings.always_network_applications.len(), 1);
        assert_eq!(settings.always_network_applications[0].name, "Example");

        settings.always_network_applications = vec![RegisteredApplication {
            name: "Spotify".into(),
            bundle_id: "com.spotify.client".into(),
        }];
        assert!(normalize_settings(&mut settings).is_err());
    }

    #[test]
    fn itunes_match_accepts_traditional_metadata_and_prefers_the_album() {
        let snapshot = PlaybackSnapshot {
            title: Some("恶作剧".into()),
            artist: Some("王蓝茵".into()),
            album: Some("恶作剧之吻 电视剧原声带".into()),
            duration_ms: Some(227_000),
            ..PlaybackSnapshot::default()
        };
        let track = |artist: &str, album: &str, duration| ItunesTrack {
            track_name: "惡作劇".into(),
            artist_name: artist.into(),
            collection_name: Some(album.into()),
            collection_id: Some(1),
            artwork_url100: Some("https://example.com/100x100.jpg".into()),
            collection_view_url: None,
            track_time_millis: Some(duration),
        };
        let soundtrack = track("王藍茵", "電視《惡作劇之吻》原聲帶", 226_773);
        let single = track("王藍茵", "惡作劇 - Single", 225_652);
        let wrong_artist = track("林依晨", "電視《惡作劇之吻》原聲帶", 226_773);

        let soundtrack_score = itunes_match_score(&snapshot, &soundtrack);
        let single_score = itunes_match_score(&snapshot, &single);
        assert!(soundtrack_score.is_some());
        assert!(single_score.is_none_or(|score| soundtrack_score.unwrap() > score));
        assert!(itunes_match_score(&snapshot, &wrong_artist).is_none());
    }

    #[test]
    fn itunes_country_is_validated_and_separates_negative_cache() {
        for country in ["CN", "TW", "HK", "US"] {
            assert_eq!(validate_itunes_country(country).unwrap(), country);
        }
        assert!(validate_itunes_country("JP").is_err());
        assert_eq!(
            ItunesStorefront::Auto.effective_country("HK").unwrap(),
            "HK"
        );
        assert_eq!(ItunesStorefront::Cn.effective_country("HK").unwrap(), "CN");
        assert_ne!(
            network_missing_key("TW", "artist|album"),
            network_missing_key("HK", "artist|album")
        );
    }

    #[test]
    fn prunes_blobs_and_references() {
        let root = tempdir().unwrap();
        initialize_directory(root.path()).unwrap();
        let mut index = CacheIndex::default();
        for number in 0..3 {
            let hash = number.to_string();
            fs::write(root.path().join("blobs").join(format!("{hash}.jpg")), b"x").unwrap();
            index.blobs.insert(
                hash.clone(),
                BlobMetadata {
                    source: "player".into(),
                    source_link: None,
                    width: 1,
                    height: 1,
                    last_accessed: number,
                },
            );
            index.tracks.insert(
                hash.clone(),
                CacheReference {
                    hash,
                    low_quality: false,
                },
            );
        }
        prune_cache(root.path(), &mut index, 2);
        assert_eq!(index.blobs.len(), 2);
        assert_eq!(index.tracks.len(), 2);
    }
}
