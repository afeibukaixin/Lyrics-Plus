use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::future::join_all;
use serde::{Deserialize, Serialize};
use strsim::normalized_levenshtein;
use zhhz::{Config, Converter};

use super::kugou::KugouProvider;
use super::kuwo::KuwoProvider;
use super::lrclib::LrcLibProvider;
use super::migu::MiguProvider;
use super::musixmatch::MusixmatchProvider;
use super::netease::NeteaseProvider;
use super::qqmusic::QqMusicProvider;
use super::{
    amll_ttml::AmllTtmlProvider,
    credentials::{MusixmatchTokenType, ProviderCredentialStore, ProviderCredentialView},
};

pub const LRCLIB_DISPLAY_NAME: &str = "LRCLIB";
pub const KUGOU_DISPLAY_NAME: &str = "Kugou";
pub const QQMUSIC_DISPLAY_NAME: &str = "QQMusic";
pub const NETEASE_DISPLAY_NAME: &str = "Netease";
pub const KUWO_DISPLAY_NAME: &str = "Kuwo";
pub const AMLL_DISPLAY_NAME: &str = "AMLL TTML";
pub const MIGU_DISPLAY_NAME: &str = "Migu";
pub const MUSIXMATCH_DISPLAY_NAME: &str = "Musixmatch";
pub const DEFAULT_AMLL_BASE_URL: &str = "https://amlldb.bikonoo.com";
const LEGACY_AMLL_BASE_URLS: [&str; 2] = [
    "https://cdn.jsdelivr.net/gh/Steve-xmh/amll-ttml-db@main",
    "https://github.com/amll-dev/amll-ttml-db/raw/refs/heads/main",
];

pub type ProviderFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ProviderError>> + Send + 'a>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderHealth {
    Unknown,
    Available,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider_id: String,
    pub name: String,
    pub health: ProviderHealth,
    pub message: Option<String>,
    pub checked_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    Network,
    Http,
    InvalidResponse,
    Configuration,
    Unauthorized,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    pub provider_id: String,
    pub kind: ProviderErrorKind,
    pub message: String,
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}：{}", self.provider_id, self.message)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchWeights {
    pub title: u8,
    pub artist: u8,
    pub album: u8,
    pub duration: u8,
}

impl MatchWeights {
    fn total(self) -> u16 {
        u16::from(self.title)
            + u16::from(self.artist)
            + u16::from(self.album)
            + u16::from(self.duration)
    }
}

impl Default for MatchWeights {
    fn default() -> Self {
        Self {
            title: 39,
            artist: 36,
            album: 8,
            duration: 17,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ScoringSettings {
    title_filter_keywords: Vec<String>,
    match_weights: MatchWeights,
    normalize_chinese: bool,
}

impl Default for ScoringSettings {
    fn default() -> Self {
        Self {
            title_filter_keywords: Vec::new(),
            match_weights: MatchWeights::default(),
            normalize_chinese: default_normalize_chinese(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchInput {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    #[serde(skip)]
    pub(crate) scoring: Arc<ScoringSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchResult {
    pub id: String,
    pub provider_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub source: String,
    pub synced: bool,
    pub has_translation: bool,
    pub has_word_timing: bool,
    pub has_romanization: bool,
    pub score: f64,
    pub lyrics: String,
}

#[derive(Debug)]
pub struct ProviderSearchReport {
    pub results: Vec<LyricsSearchResult>,
    pub warning: Option<String>,
}

impl ProviderSearchReport {
    pub fn available(results: Vec<LyricsSearchResult>) -> Self {
        Self {
            results,
            warning: None,
        }
    }
}

pub(crate) fn collect_provider_results(
    outcomes: impl IntoIterator<Item = Result<Option<LyricsSearchResult>, ProviderError>>,
) -> Result<ProviderSearchReport, ProviderError> {
    let mut results = Vec::new();
    let mut first_error = None;
    let mut any_success = false;
    for outcome in outcomes {
        match outcome {
            Ok(result) => {
                any_success = true;
                results.extend(result);
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if !any_success {
        if let Some(error) = first_error {
            return Err(error);
        }
    }
    Ok(ProviderSearchReport {
        results,
        warning: first_error.map(|error| error.message),
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOrderMode {
    #[default]
    Smart,
    Strict,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreference {
    pub id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(default, rename_all = "camelCase")]
pub struct ProviderSettings {
    pub mode: ProviderOrderMode,
    pub providers: Vec<ProviderPreference>,
    #[serde(default = "default_auto_apply_threshold")]
    pub auto_apply_threshold: u8,
    #[serde(default)]
    pub prefer_capabilities: bool,
    #[serde(default)]
    pub match_weights: MatchWeights,
    #[serde(default = "default_normalize_chinese")]
    pub normalize_chinese: bool,
    #[serde(default = "default_title_filter_keywords")]
    pub title_filter_keywords: Vec<String>,
    #[serde(default = "default_amll_base_url")]
    pub amll_base_url: String,
}

const MAX_TITLE_FILTER_KEYWORDS: usize = 32;
const MAX_TITLE_FILTER_KEYWORD_LENGTH: usize = 64;

const fn default_auto_apply_threshold() -> u8 {
    60
}

const fn default_normalize_chinese() -> bool {
    true
}

fn default_title_filter_keywords() -> Vec<String> {
    [
        "feat",
        "ft",
        "featuring",
        "主题曲",
        "片头曲",
        "片尾曲",
        "插曲",
        "电影",
        "电视剧",
        "动画",
        "游戏",
        "ost",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn default_amll_base_url() -> String {
    DEFAULT_AMLL_BASE_URL.into()
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            mode: ProviderOrderMode::Smart,
            providers: provider_definitions()
                .into_iter()
                .map(|(id, _)| ProviderPreference {
                    id: id.into(),
                    enabled: true,
                })
                .collect(),
            auto_apply_threshold: default_auto_apply_threshold(),
            prefer_capabilities: false,
            match_weights: MatchWeights::default(),
            normalize_chinese: default_normalize_chinese(),
            title_filter_keywords: default_title_filter_keywords(),
            amll_base_url: default_amll_base_url(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsView {
    pub settings: ProviderSettings,
    pub statuses: Vec<ProviderStatus>,
}

#[derive(Clone)]
pub struct ProviderSearchOutcome {
    pub results: Vec<LyricsSearchResult>,
    pub statuses: Vec<ProviderStatus>,
    pub auto_apply_threshold: u8,
    pub prefer_capabilities: bool,
    pub error: Option<String>,
}

type SearchFlight = tokio::sync::OnceCell<Result<ProviderSearchOutcome, String>>;

#[derive(Clone, PartialEq, Eq, Hash)]
struct SearchKey {
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: Option<u64>,
    settings: ProviderSettings,
    revision: u64,
}

impl SearchKey {
    fn new(input: &LyricsSearchInput, settings: ProviderSettings, revision: u64) -> Self {
        Self {
            title: input.title.trim().into(),
            artist: input.artist.trim().into(),
            album: input.album.as_deref().map(str::trim).map(str::to_owned),
            duration_ms: input.duration_ms,
            settings,
            revision,
        }
    }
}

pub trait LyricsProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, ProviderSearchReport>;
}

pub struct ProviderRegistry {
    providers: Vec<Box<dyn LyricsProvider>>,
    settings: Arc<RwLock<ProviderSettings>>,
    credentials: Arc<ProviderCredentialStore>,
    statuses: RwLock<HashMap<String, ProviderStatus>>,
    in_flight: Mutex<HashMap<SearchKey, Weak<SearchFlight>>>,
    revision: AtomicU64,
    timeout: Duration,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new(ProviderSettings::default())
    }
}

impl ProviderRegistry {
    pub fn new(settings: ProviderSettings) -> Self {
        Self::build(settings, Arc::new(ProviderCredentialStore::memory()), None)
    }

    pub fn new_with_app_dir(settings: ProviderSettings, app_dir: &Path) -> Result<Self, String> {
        let credentials = Arc::new(ProviderCredentialStore::load(app_dir)?);
        Ok(Self::build(
            settings,
            credentials,
            Some(app_dir.join("cache").join("amll-index.json")),
        ))
    }

    fn build(
        settings: ProviderSettings,
        credentials: Arc<ProviderCredentialStore>,
        amll_cache_path: Option<std::path::PathBuf>,
    ) -> Self {
        let settings = Arc::new(RwLock::new(settings));
        let providers: Vec<Box<dyn LyricsProvider>> = vec![
            Box::new(NeteaseProvider),
            Box::new(QqMusicProvider),
            Box::new(KugouProvider),
            Box::new(LrcLibProvider),
            Box::new(KuwoProvider),
            Box::new(AmllTtmlProvider::new(settings.clone(), amll_cache_path)),
            Box::new(MiguProvider),
            Box::new(MusixmatchProvider::new(credentials.clone())),
        ];
        let statuses = providers
            .iter()
            .map(|provider| {
                (
                    provider.id().into(),
                    ProviderStatus {
                        provider_id: provider.id().into(),
                        name: provider.display_name().into(),
                        health: ProviderHealth::Unknown,
                        message: Some("尚未测试".into()),
                        checked_at_ms: None,
                    },
                )
            })
            .collect();
        Self {
            providers,
            settings,
            credentials,
            statuses: RwLock::new(statuses),
            in_flight: Mutex::new(HashMap::new()),
            revision: AtomicU64::new(0),
            timeout: Duration::from_secs(8),
        }
    }

    pub fn settings_view(&self) -> ProviderSettingsView {
        ProviderSettingsView {
            settings: self
                .settings
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone(),
            statuses: self.statuses(),
        }
    }

    pub fn set_settings(
        &self,
        mut settings: ProviderSettings,
    ) -> Result<ProviderSettingsView, String> {
        normalize_settings(&mut settings)?;
        *self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner()) = settings;
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(self.settings_view())
    }

    pub fn credential_view(&self) -> ProviderCredentialView {
        self.credentials.view()
    }

    pub fn set_musixmatch_token(
        &self,
        token_type: MusixmatchTokenType,
        token: String,
    ) -> Result<(ProviderCredentialView, ProviderSettingsView), String> {
        let credentials = self.credentials.set_musixmatch_token(token_type, token)?;
        let mut settings = self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(provider) = settings
            .providers
            .iter_mut()
            .find(|provider| provider.id == "musixmatch")
        {
            provider.enabled = true;
        }
        drop(settings);
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok((credentials, self.settings_view()))
    }

    pub fn clear_musixmatch_token(
        &self,
    ) -> Result<(ProviderCredentialView, ProviderSettingsView), String> {
        let credentials = self.credentials.clear_musixmatch_token()?;
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok((credentials, self.settings_view()))
    }

    pub async fn search(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
    ) -> Result<ProviderSearchOutcome, String> {
        let settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let key = SearchKey::new(
            input,
            settings.clone(),
            self.revision.load(Ordering::SeqCst),
        );
        let flight = {
            let mut in_flight = self
                .in_flight
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            in_flight.retain(|_, flight| flight.strong_count() > 0);
            if let Some(flight) = in_flight.get(&key).and_then(Weak::upgrade) {
                flight
            } else {
                let flight = Arc::new(SearchFlight::new());
                in_flight.insert(key, Arc::downgrade(&flight));
                flight
            }
        };
        flight
            .get_or_init(|| self.search_once(client, input, settings))
            .await
            .clone()
    }

    async fn search_once(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        settings: ProviderSettings,
    ) -> Result<ProviderSearchOutcome, String> {
        let priority = settings
            .providers
            .iter()
            .enumerate()
            .map(|(index, preference)| (preference.id.as_str(), index))
            .collect::<HashMap<_, _>>();
        let enabled = settings
            .providers
            .iter()
            .filter(|preference| preference.enabled)
            .filter_map(|preference| {
                self.providers
                    .iter()
                    .find(|provider| provider.id() == preference.id)
                    .map(|provider| provider.as_ref())
            })
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            return Err("请至少启用一个歌词源".into());
        }
        let enabled_ids = enabled
            .iter()
            .map(|provider| provider.id().to_owned())
            .collect::<Vec<_>>();

        let mut scoring_input = input.clone();
        scoring_input.scoring = Arc::new(ScoringSettings {
            title_filter_keywords: prepare_title_filter_keywords_with_normalization(
                &settings.title_filter_keywords,
                settings.normalize_chinese,
            )?,
            match_weights: settings.match_weights,
            normalize_chinese: settings.normalize_chinese,
        });
        let scoring_input = &scoring_input;
        let jobs = enabled.iter().map(|provider| async move {
            let outcome =
                tokio::time::timeout(self.timeout, provider.search(client, scoring_input)).await;
            (*provider, outcome)
        });
        let outcomes = join_all(jobs).await;
        let mut results = Vec::new();
        let mut errors = Vec::new();
        let mut any_success = false;
        for (provider, outcome) in outcomes {
            match outcome {
                Ok(Ok(mut report)) => {
                    any_success = true;
                    let (health, message) = report_status(&report);
                    self.record_status(provider, health, message);
                    results.append(&mut report.results);
                }
                Ok(Err(error)) => {
                    errors.push(error.to_string());
                    self.record_status(provider, ProviderHealth::Unavailable, Some(error.message));
                }
                Err(_) => {
                    let message = "搜索超时".to_string();
                    errors.push(format!("{}：{message}", provider.display_name()));
                    self.record_status(provider, ProviderHealth::Unavailable, Some(message));
                }
            }
        }

        match settings.mode {
            ProviderOrderMode::Strict => results.sort_by(|left, right| {
                priority
                    .get(left.provider_id.as_str())
                    .cmp(&priority.get(right.provider_id.as_str()))
                    .then_with(|| right.score.total_cmp(&left.score))
            }),
            ProviderOrderMode::Smart => {
                results.sort_by(|left, right| right.score.total_cmp(&left.score));
                let mut band_start = 0;
                while band_start < results.len() {
                    let band_score = results[band_start].score;
                    let band_len = results[band_start..]
                        .iter()
                        .take_while(|result| band_score - result.score <= 0.035)
                        .count();
                    let band_end = band_start + band_len;
                    results[band_start..band_end].sort_by(|left, right| {
                        priority
                            .get(left.provider_id.as_str())
                            .cmp(&priority.get(right.provider_id.as_str()))
                            .then_with(|| right.score.total_cmp(&left.score))
                    });
                    band_start = band_end;
                }
            }
        }
        deduplicate(&mut results);
        results.truncate(24);
        Ok(ProviderSearchOutcome {
            results,
            statuses: self.statuses_for(&enabled_ids),
            auto_apply_threshold: settings.auto_apply_threshold,
            prefer_capabilities: settings.prefer_capabilities,
            error: (!any_success && !errors.is_empty()).then(|| errors.join("；")),
        })
    }

    pub async fn test_provider(
        &self,
        client: &reqwest::Client,
        provider_id: &str,
    ) -> Result<ProviderStatus, String> {
        let provider = self
            .providers
            .iter()
            .find(|provider| provider.id() == provider_id)
            .ok_or_else(|| "未知歌词源".to_string())?;
        let input = LyricsSearchInput {
            title: "晴天".into(),
            artist: "周杰伦".into(),
            album: None,
            duration_ms: Some(269_000),
            scoring: Arc::default(),
        };
        match tokio::time::timeout(self.timeout, provider.search(client, &input)).await {
            Ok(Ok(report)) => {
                let (health, message) = report_status(&report);
                self.record_status(provider.as_ref(), health, message);
            }
            Ok(Err(error)) => self.record_status(
                provider.as_ref(),
                ProviderHealth::Unavailable,
                Some(error.message),
            ),
            Err(_) => self.record_status(
                provider.as_ref(),
                ProviderHealth::Unavailable,
                Some("测试超时".into()),
            ),
        }
        self.statuses
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get(provider_id)
            .cloned()
            .ok_or_else(|| "无法读取歌词源状态".into())
    }

    fn statuses(&self) -> Vec<ProviderStatus> {
        let settings = self
            .settings
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let statuses = self
            .statuses
            .read()
            .unwrap_or_else(|error| error.into_inner());
        settings
            .providers
            .iter()
            .filter_map(|preference| statuses.get(&preference.id).cloned())
            .collect()
    }

    fn statuses_for(&self, provider_ids: &[String]) -> Vec<ProviderStatus> {
        let statuses = self
            .statuses
            .read()
            .unwrap_or_else(|error| error.into_inner());
        provider_ids
            .iter()
            .filter_map(|provider_id| statuses.get(provider_id).cloned())
            .collect()
    }

    fn record_status(
        &self,
        provider: &dyn LyricsProvider,
        health: ProviderHealth,
        message: Option<String>,
    ) {
        self.statuses
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                provider.id().into(),
                ProviderStatus {
                    provider_id: provider.id().into(),
                    name: provider.display_name().into(),
                    health,
                    message,
                    checked_at_ms: Some(now_ms()),
                },
            );
    }
}

fn report_status(report: &ProviderSearchReport) -> (ProviderHealth, Option<String>) {
    if let Some(warning) = &report.warning {
        return (
            ProviderHealth::Degraded,
            Some(format!("部分请求失败：{warning}")),
        );
    }
    let message = if report.results.is_empty() {
        "连接正常，未找到同步歌词".into()
    } else {
        format!("连接正常，返回 {} 个候选", report.results.len())
    };
    (ProviderHealth::Available, Some(message))
}

fn provider_definitions() -> [(&'static str, &'static str); 8] {
    [
        ("lrclib", LRCLIB_DISPLAY_NAME),
        ("kugou", KUGOU_DISPLAY_NAME),
        ("qqmusic", QQMUSIC_DISPLAY_NAME),
        ("netease", NETEASE_DISPLAY_NAME),
        ("kuwo", KUWO_DISPLAY_NAME),
        ("amll_ttml", AMLL_DISPLAY_NAME),
        ("migu", MIGU_DISPLAY_NAME),
        ("musixmatch", MUSIXMATCH_DISPLAY_NAME),
    ]
}

pub(crate) fn validate_settings(settings: &ProviderSettings) -> Result<(), String> {
    if settings.auto_apply_threshold > 100 {
        return Err("自动匹配相似度必须在 0–100 之间".into());
    }
    if [
        settings.match_weights.title,
        settings.match_weights.artist,
        settings.match_weights.album,
        settings.match_weights.duration,
    ]
    .into_iter()
    .any(|weight| weight > 100)
    {
        return Err("歌词匹配重要度必须在 0–100 之间".into());
    }
    if settings.match_weights.total() == 0 {
        return Err("歌词匹配重要度不能全部为 0".into());
    }
    let known = provider_definitions()
        .into_iter()
        .map(|(id, _)| id)
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for preference in &settings.providers {
        if !known.contains(preference.id.as_str()) {
            return Err(format!("未知歌词源：{}", preference.id));
        }
        if !seen.insert(&preference.id) {
            return Err(format!("歌词源重复：{}", preference.id));
        }
    }
    if !settings.providers.iter().any(|provider| provider.enabled) {
        return Err("请至少启用一个歌词源".into());
    }
    let amll_url = reqwest::Url::parse(settings.amll_base_url.trim())
        .map_err(|_| "AMLL TTML 镜像地址必须是有效的绝对 URL".to_string())?;
    if !matches!(amll_url.scheme(), "http" | "https") {
        return Err("AMLL TTML 镜像地址只支持 http 或 https".into());
    }
    prepare_title_filter_keywords_with_normalization(
        &settings.title_filter_keywords,
        settings.normalize_chinese,
    )?;
    Ok(())
}

pub(crate) fn normalize_settings(settings: &mut ProviderSettings) -> Result<(), String> {
    for keyword in &mut settings.title_filter_keywords {
        *keyword = keyword.trim().to_string();
    }
    settings.amll_base_url = settings
        .amll_base_url
        .trim()
        .trim_end_matches('/')
        .to_string();
    if LEGACY_AMLL_BASE_URLS.contains(&settings.amll_base_url.as_str()) {
        settings.amll_base_url = DEFAULT_AMLL_BASE_URL.into();
    }
    validate_settings(settings)?;
    complete_settings(settings);
    Ok(())
}

#[cfg(test)]
fn prepare_title_filter_keywords(keywords: &[String]) -> Result<Vec<String>, String> {
    prepare_title_filter_keywords_with_normalization(keywords, true)
}

fn prepare_title_filter_keywords_with_normalization(
    keywords: &[String],
    normalize_chinese: bool,
) -> Result<Vec<String>, String> {
    if keywords.len() > MAX_TITLE_FILTER_KEYWORDS {
        return Err(format!("标题屏蔽内容最多 {MAX_TITLE_FILTER_KEYWORDS} 条"));
    }
    let mut seen = HashSet::new();
    keywords
        .iter()
        .enumerate()
        .map(|(index, keyword)| {
            let keyword = keyword.trim();
            if keyword.is_empty() {
                return Err(format!("第 {} 条标题屏蔽内容不能为空", index + 1));
            }
            if keyword.chars().count() > MAX_TITLE_FILTER_KEYWORD_LENGTH {
                return Err(format!(
                    "第 {} 条标题屏蔽内容不能超过 {MAX_TITLE_FILTER_KEYWORD_LENGTH} 个字符",
                    index + 1
                ));
            }
            let keyword = normalize_case(keyword, normalize_chinese);
            if !seen.insert(keyword.clone()) {
                return Err(format!("第 {} 条标题屏蔽内容重复", index + 1));
            }
            Ok(keyword)
        })
        .collect()
}

fn complete_settings(settings: &mut ProviderSettings) {
    for (id, _) in provider_definitions() {
        if !settings.providers.iter().any(|provider| provider.id == id) {
            settings.providers.push(ProviderPreference {
                id: id.into(),
                enabled: true,
            });
        }
    }
}

fn deduplicate(results: &mut Vec<LyricsSearchResult>) {
    let mut seen = HashSet::new();
    results.retain(|result| {
        let lyric_key = result
            .lyrics
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        seen.insert(lyric_key)
    });
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn simplify(value: &str) -> String {
    // ponytail: small candidate batches share one lock; use thread-local converters if scoring becomes hot.
    static CONVERTER: OnceLock<Mutex<Converter>> = OnceLock::new();

    CONVERTER
        .get_or_init(|| Mutex::new(Converter::new(Config::T2s)))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .convert(value)
        .to_lowercase()
}

fn normalize_case(value: &str, normalize_chinese: bool) -> String {
    if normalize_chinese {
        simplify(value)
    } else {
        value.to_lowercase()
    }
}

#[cfg(test)]
fn normalise(value: &str) -> String {
    normalise_with_options(value, true)
}

fn normalise_with_options(value: &str, normalize_chinese: bool) -> String {
    normalize_case(value, normalize_chinese)
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn keyword_position(title: &str, keyword: &str) -> Option<(usize, usize)> {
    let needs_ascii_boundaries = keyword
        .chars()
        .all(|character| character.is_ascii_alphanumeric());
    title.match_indices(keyword).find_map(|(start, matched)| {
        let end = start + matched.len();
        let boundary_matches = !needs_ascii_boundaries
            || (title[..start]
                .chars()
                .next_back()
                .is_none_or(|character| !character.is_ascii_alphanumeric())
                && title[end..]
                    .chars()
                    .next()
                    .is_none_or(|character| !character.is_ascii_alphanumeric()));
        boundary_matches.then_some((start, end))
    })
}

fn enclosing_bracket_range(title: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    [('(', ')'), ('[', ']'), ('（', '）'), ('【', '】')]
        .into_iter()
        .filter_map(|(open, close)| {
            let open_index = title[..start].rfind(open)?;
            if title[open_index + open.len_utf8()..start].contains(close) {
                return None;
            }
            let close_index = end + title[end..].find(close)? + close.len_utf8();
            Some((open_index, close_index))
        })
        .max_by_key(|(open_index, _)| *open_index)
}

fn suffix_delimiter_start(title: &str, before: usize) -> Option<usize> {
    title[..before]
        .char_indices()
        .filter_map(|(index, character)| ['-', '–', '—'].contains(&character).then_some(index))
        .next_back()
}

fn work_title_start(title: &str, before: usize) -> Option<usize> {
    [('《', '》'), ('「', '」'), ('『', '』')]
        .into_iter()
        .filter_map(|(open, close)| {
            let open_index = title[..before].rfind(open)?;
            title[open_index + open.len_utf8()..before]
                .contains(close)
                .then_some(open_index)
        })
        .max()
}

#[cfg(test)]
fn filter_title(value: &str, keywords: &[String]) -> String {
    filter_title_with_options(value, keywords, true)
}

fn filter_title_with_options(value: &str, keywords: &[String], normalize_chinese: bool) -> String {
    let mut title = normalize_case(value, normalize_chinese);
    for keyword in keywords {
        while let Some((start, end)) = keyword_position(&title, keyword) {
            if let Some((open, close)) = enclosing_bracket_range(&title, start, end) {
                title.replace_range(open..close, "");
            } else if let Some(delimiter) = suffix_delimiter_start(&title, start) {
                title.truncate(delimiter);
            } else if let Some(open) = work_title_start(&title, start) {
                title.truncate(open);
            } else if ["feat", "ft", "featuring"].contains(&keyword.as_str()) {
                title.truncate(start);
            } else {
                title.replace_range(start..end, "");
            }
            title = title.trim().to_string();
        }
    }
    title
}

pub fn score_candidate(input: &LyricsSearchInput, result: &LyricsSearchResult) -> f64 {
    let scoring = &input.scoring;
    let title = normalized_levenshtein(
        &normalise_with_options(
            &filter_title_with_options(
                &input.title,
                &scoring.title_filter_keywords,
                scoring.normalize_chinese,
            ),
            scoring.normalize_chinese,
        ),
        &normalise_with_options(
            &filter_title_with_options(
                &result.title,
                &scoring.title_filter_keywords,
                scoring.normalize_chinese,
            ),
            scoring.normalize_chinese,
        ),
    );
    let artist = normalized_levenshtein(
        &normalise_with_options(&input.artist, scoring.normalize_chinese),
        &normalise_with_options(&result.artist, scoring.normalize_chinese),
    );
    let album = match (&input.album, &result.album) {
        (Some(expected), Some(actual)) => normalized_levenshtein(
            &normalise_with_options(expected, scoring.normalize_chinese),
            &normalise_with_options(actual, scoring.normalize_chinese),
        ),
        _ => 0.6,
    };
    let duration = match (input.duration_ms, result.duration_ms) {
        (Some(expected), Some(actual)) => {
            let delta = expected.abs_diff(actual) as f64;
            (1.0 - delta / 12_000.0).clamp(0.0, 1.0)
        }
        _ => 0.6,
    };
    let weights = scoring.match_weights;
    let weight_total = f64::from(weights.total());
    (title * f64::from(weights.title) / weight_total
        + artist * f64::from(weights.artist) / weight_total
        + album * f64::from(weights.album) / weight_total
        + duration * f64::from(weights.duration) / weight_total
        + if result.synced { 0.04 } else { 0.0 })
    .clamp(0.0, 1.0)
}

pub fn can_auto_apply(results: &[LyricsSearchResult], threshold_percent: u8) -> bool {
    let Some(first) = results.first() else {
        return false;
    };
    first.score >= f64::from(threshold_percent) / 100.0 && first.synced
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn defaults_to_all_providers_enabled() {
        let settings = ProviderSettings::default();
        assert!(settings.providers.iter().all(|provider| provider.enabled));
    }

    struct MockProvider {
        id: &'static str,
        score: f64,
        fails: bool,
        warning: Option<&'static str>,
        empty: bool,
        lyrics: &'static str,
        calls: Option<Arc<AtomicUsize>>,
        delay: Duration,
    }

    impl LyricsProvider for MockProvider {
        fn id(&self) -> &'static str {
            self.id
        }

        fn display_name(&self) -> &'static str {
            self.id
        }

        fn search<'a>(
            &'a self,
            _client: &'a reqwest::Client,
            _input: &'a LyricsSearchInput,
        ) -> ProviderFuture<'a, ProviderSearchReport> {
            Box::pin(async move {
                if let Some(calls) = &self.calls {
                    calls.fetch_add(1, Ordering::SeqCst);
                }
                if !self.delay.is_zero() {
                    tokio::time::sleep(self.delay).await;
                }
                if self.fails {
                    return Err(ProviderError {
                        provider_id: self.id.into(),
                        kind: ProviderErrorKind::Network,
                        message: "mock failure".into(),
                    });
                }
                Ok(ProviderSearchReport {
                    results: (!self.empty)
                        .then(|| result(self.id, self.score, self.lyrics))
                        .into_iter()
                        .collect(),
                    warning: self.warning.map(str::to_owned),
                })
            })
        }
    }

    fn result(provider_id: &str, score: f64, lyrics: &str) -> LyricsSearchResult {
        LyricsSearchResult {
            id: format!("{provider_id}:1"),
            provider_id: provider_id.into(),
            title: "Hello".into(),
            artist: "Adele".into(),
            album: Some("25".into()),
            duration_ms: Some(295_100),
            source: provider_id.into(),
            synced: true,
            has_translation: false,
            has_word_timing: false,
            has_romanization: false,
            score,
            lyrics: lyrics.into(),
        }
    }

    #[test]
    fn exact_synced_result_scores_highly() {
        let input = LyricsSearchInput {
            title: "Hello".into(),
            artist: "Adele".into(),
            album: Some("25".into()),
            duration_ms: Some(295_000),
            scoring: Arc::default(),
        };
        assert!(score_candidate(&input, &result("lrclib", 0.0, "line")) > 0.98);
    }

    #[test]
    fn normalise_treats_traditional_and_simplified_chinese_as_equal() {
        assert_eq!(normalise("愛上你不是我決定"), normalise("爱上你不是我决定"));
        assert_eq!(normalise("蕭敬騰"), normalise("萧敬腾"));
    }

    #[test]
    fn traditional_metadata_scores_the_same_as_simplified_metadata() {
        let traditional = LyricsSearchInput {
            title: "愛上你不是我決定".into(),
            artist: "蕭敬騰".into(),
            album: Some("愛的時刻".into()),
            duration_ms: Some(295_000),
            scoring: Arc::default(),
        };
        let simplified = LyricsSearchInput {
            title: "爱上你不是我决定".into(),
            artist: "萧敬腾".into(),
            album: Some("爱的时刻".into()),
            duration_ms: Some(295_000),
            scoring: Arc::default(),
        };
        let mut candidate = result("lrclib", 0.0, "line");
        candidate.title = simplified.title.clone();
        candidate.artist = simplified.artist.clone();
        candidate.album = simplified.album.clone();
        candidate.duration_ms = simplified.duration_ms;

        let traditional_score = score_candidate(&traditional, &candidate);
        assert_eq!(traditional_score, score_candidate(&simplified, &candidate));
        assert!(traditional_score > 0.98);
    }

    #[test]
    fn default_title_filters_remove_only_matching_metadata() {
        let keywords = prepare_title_filter_keywords(&default_title_filter_keywords()).unwrap();
        assert_eq!(
            filter_title("All For You - 《蜘蛛人：重生日》電影片尾曲", &keywords),
            "all for you"
        );
        assert_eq!(
            filter_title("愛上你不是我決定 (feat. A-Lin)", &keywords),
            "爱上你不是我决定"
        );
        assert_eq!(
            filter_title("All For You 《蜘蛛人：重生日》電影片尾曲", &keywords),
            "all for you"
        );
        assert_eq!(filter_title("Song featuring Artist", &keywords), "song");
        for title in [
            "A-B",
            "Song (Live)",
            "Song - Remix",
            "Song (Acoustic)",
            "伴奏",
            "Soft Landing",
            "Most Wanted",
        ] {
            assert_eq!(filter_title(title, &keywords), simplify(title));
        }
        assert_eq!(filter_title("Song Demo", &["demo".into()]), "song");
        assert_eq!(filter_title("Song (Live)", &["live".into()]), "song");
        assert_eq!(filter_title("伴奏", &["伴奏".into()]), "");
    }

    #[test]
    fn title_filters_apply_to_both_sides_of_scoring() {
        let keywords =
            Arc::new(prepare_title_filter_keywords(&default_title_filter_keywords()).unwrap());
        let mut input = LyricsSearchInput {
            title: "All For You - 《蜘蛛人：重生日》電影片尾曲".into(),
            artist: "OneRepublic".into(),
            album: None,
            duration_ms: Some(240_000),
            scoring: Arc::new(ScoringSettings {
                title_filter_keywords: keywords.as_ref().clone(),
                ..ScoringSettings::default()
            }),
        };
        let mut candidate = result("lrclib", 0.0, "line");
        candidate.title = "All For You".into();
        candidate.artist = input.artist.clone();
        candidate.album = None;
        candidate.duration_ms = input.duration_ms;
        assert!(score_candidate(&input, &candidate) > 0.98);

        input.title = "愛上你不是我決定".into();
        candidate.title = "爱上你不是我决定 (feat. A-Lin)".into();
        candidate.artist = "蕭敬騰".into();
        input.artist = "萧敬腾".into();
        assert!(score_candidate(&input, &candidate) > 0.98);
    }

    #[test]
    fn title_filter_validation_rejects_invalid_lists() {
        assert!(prepare_title_filter_keywords(&[]).is_ok());
        for keywords in [
            vec![" ".into()],
            vec!["same".into(), "same".into()],
            vec!["OST".into(), "ost".into()],
            vec!["电影".into(), "電影".into()],
            vec!["x".repeat(MAX_TITLE_FILTER_KEYWORD_LENGTH + 1)],
            vec!["x".into(); MAX_TITLE_FILTER_KEYWORDS + 1],
        ] {
            assert!(prepare_title_filter_keywords(&keywords).is_err());
        }

        let mut settings = ProviderSettings {
            title_filter_keywords: vec!["  Live  ".into()],
            ..ProviderSettings::default()
        };
        normalize_settings(&mut settings).unwrap();
        assert_eq!(settings.title_filter_keywords, ["Live"]);
    }

    #[test]
    fn auto_apply_uses_configured_threshold_and_requires_synced_lyrics() {
        assert!(can_auto_apply(
            &[result("a", 0.94, "one"), result("b", 0.84, "two")],
            60
        ));
        assert!(can_auto_apply(
            &[result("a", 0.94, "one"), result("b", 0.90, "two")],
            60
        ));
        assert!(can_auto_apply(&[result("a", 0.60, "one")], 60));
        assert!(!can_auto_apply(&[result("a", 0.59, "one")], 60));
        assert!(can_auto_apply(&[result("a", 0.0, "one")], 0));
        assert!(can_auto_apply(&[result("a", 1.0, "one")], 100));
        assert!(!can_auto_apply(&[result("a", 0.99, "one")], 100));
        let mut unsynced = result("a", 0.99, "one");
        unsynced.synced = false;
        assert!(!can_auto_apply(&[unsynced], 60));
    }

    #[test]
    fn duplicate_lyrics_are_removed_across_sources() {
        let mut results = vec![
            result("netease", 0.9, "[00:01] Hello"),
            result("qqmusic", 0.8, "[00:01]Hello"),
        ];
        deduplicate(&mut results);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn invalid_settings_are_rejected() {
        assert!(validate_settings(&ProviderSettings {
            mode: ProviderOrderMode::Smart,
            providers: vec![ProviderPreference {
                id: "unknown".into(),
                enabled: true,
            }],
            auto_apply_threshold: 60,
            ..ProviderSettings::default()
        })
        .is_err());
        assert!(validate_settings(&ProviderSettings {
            mode: ProviderOrderMode::Smart,
            providers: vec![ProviderPreference {
                id: "lrclib".into(),
                enabled: false,
            }],
            auto_apply_threshold: 60,
            ..ProviderSettings::default()
        })
        .is_err());
        assert!(validate_settings(&ProviderSettings {
            auto_apply_threshold: 101,
            ..ProviderSettings::default()
        })
        .is_err());
    }

    #[test]
    fn provider_display_names_are_stable_brand_names() {
        let registry = ProviderRegistry::default();
        let names = registry
            .settings_view()
            .statuses
            .into_iter()
            .map(|status| (status.provider_id, status.name))
            .collect::<HashMap<_, _>>();

        for (provider_id, expected_name) in provider_definitions() {
            assert_eq!(
                names.get(provider_id).map(String::as_str),
                Some(expected_name)
            );
        }
    }

    #[test]
    fn default_settings_use_current_smart_priority() {
        let settings = ProviderSettings::default();
        assert_eq!(settings.mode, ProviderOrderMode::Smart);
        assert_eq!(settings.auto_apply_threshold, 60);
        assert_eq!(settings.title_filter_keywords.len(), 12);
        assert_eq!(
            settings
                .providers
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "lrclib",
                "kugou",
                "qqmusic",
                "netease",
                "kuwo",
                "amll_ttml",
                "migu",
                "musixmatch",
            ]
        );
    }

    #[test]
    fn explicitly_saved_legacy_order_is_preserved() {
        let registry = ProviderRegistry::default();
        let settings = ProviderSettings {
            mode: ProviderOrderMode::Smart,
            auto_apply_threshold: 60,
            prefer_capabilities: false,
            match_weights: MatchWeights::default(),
            normalize_chinese: true,
            providers: ["netease", "qqmusic", "kugou", "lrclib"]
                .into_iter()
                .map(|id| ProviderPreference {
                    id: id.into(),
                    enabled: true,
                })
                .collect(),
            title_filter_keywords: default_title_filter_keywords(),
            amll_base_url: default_amll_base_url(),
        };

        let view = registry.set_settings(settings.clone()).unwrap();
        let provider_ids = view
            .settings
            .providers
            .iter()
            .map(|provider| provider.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            &provider_ids[..settings.providers.len()],
            ["netease", "qqmusic", "kugou", "lrclib"]
        );
        assert_eq!(
            &provider_ids[settings.providers.len()..],
            ["kuwo", "amll_ttml", "migu", "musixmatch"]
        );
        assert!(view.settings.providers[settings.providers.len()..]
            .iter()
            .all(|provider| provider.enabled));
    }

    fn mock_registry(mode: ProviderOrderMode, netease_fails: bool) -> ProviderRegistry {
        let settings = ProviderSettings {
            mode,
            auto_apply_threshold: 60,
            prefer_capabilities: false,
            match_weights: MatchWeights::default(),
            normalize_chinese: true,
            providers: vec![
                ProviderPreference {
                    id: "lrclib".into(),
                    enabled: true,
                },
                ProviderPreference {
                    id: "netease".into(),
                    enabled: true,
                },
            ],
            title_filter_keywords: default_title_filter_keywords(),
            amll_base_url: default_amll_base_url(),
        };
        let statuses = ["lrclib", "netease"]
            .into_iter()
            .map(|id| {
                (
                    id.into(),
                    ProviderStatus {
                        provider_id: id.into(),
                        name: id.into(),
                        health: ProviderHealth::Unknown,
                        message: None,
                        checked_at_ms: None,
                    },
                )
            })
            .collect();
        ProviderRegistry {
            providers: vec![
                Box::new(MockProvider {
                    id: "lrclib",
                    score: 0.70,
                    fails: false,
                    warning: None,
                    empty: false,
                    lyrics: "[00:01]Same",
                    calls: None,
                    delay: Duration::ZERO,
                }),
                Box::new(MockProvider {
                    id: "netease",
                    score: 0.98,
                    fails: netease_fails,
                    warning: None,
                    empty: false,
                    lyrics: "[00:01]Same",
                    calls: None,
                    delay: Duration::ZERO,
                }),
            ],
            settings: Arc::new(RwLock::new(settings)),
            credentials: Arc::new(ProviderCredentialStore::memory()),
            statuses: RwLock::new(statuses),
            in_flight: Mutex::new(HashMap::new()),
            timeout: Duration::from_millis(100),
        }
    }

    fn single_mock_registry(warning: Option<&'static str>, empty: bool) -> ProviderRegistry {
        ProviderRegistry {
            providers: vec![Box::new(MockProvider {
                id: "lrclib",
                score: 0.70,
                fails: false,
                warning,
                empty,
                lyrics: "[00:01]lrclib",
                calls: None,
                delay: Duration::ZERO,
            })],
            settings: Arc::new(RwLock::new(ProviderSettings {
                mode: ProviderOrderMode::Smart,
                auto_apply_threshold: 60,
                prefer_capabilities: false,
                match_weights: MatchWeights::default(),
                normalize_chinese: true,
                providers: vec![ProviderPreference {
                    id: "lrclib".into(),
                    enabled: true,
                }],
                title_filter_keywords: default_title_filter_keywords(),
                amll_base_url: default_amll_base_url(),
            })),
            credentials: Arc::new(ProviderCredentialStore::memory()),
            statuses: RwLock::new(HashMap::from([(
                "lrclib".into(),
                ProviderStatus {
                    provider_id: "lrclib".into(),
                    name: "lrclib".into(),
                    health: ProviderHealth::Unknown,
                    message: None,
                    checked_at_ms: None,
                },
            )])),
            in_flight: Mutex::new(HashMap::new()),
            timeout: Duration::from_millis(100),
        }
    }

    fn counting_registry(calls: Arc<AtomicUsize>) -> ProviderRegistry {
        ProviderRegistry {
            providers: vec![Box::new(MockProvider {
                id: "lrclib",
                score: 0.90,
                fails: false,
                warning: None,
                empty: false,
                lyrics: "[00:01]Hello",
                calls: Some(calls),
                delay: Duration::from_millis(20),
            })],
            settings: Arc::new(RwLock::new(ProviderSettings {
                providers: vec![ProviderPreference {
                    id: "lrclib".into(),
                    enabled: true,
                }],
                ..ProviderSettings::default()
            })),
            credentials: Arc::new(ProviderCredentialStore::memory()),
            statuses: RwLock::new(HashMap::from([(
                "lrclib".into(),
                ProviderStatus {
                    provider_id: "lrclib".into(),
                    name: "lrclib".into(),
                    health: ProviderHealth::Unknown,
                    message: None,
                    checked_at_ms: None,
                },
            )])),
            in_flight: Mutex::new(HashMap::new()),
            timeout: Duration::from_millis(100),
        }
    }

    #[test]
    fn one_provider_failure_does_not_hide_other_results() {
        tauri::async_runtime::block_on(async {
            let client = reqwest::Client::new();
            let outcome = mock_registry(ProviderOrderMode::Smart, true)
                .search(
                    &client,
                    &LyricsSearchInput {
                        title: "Hello".into(),
                        artist: "Adele".into(),
                        album: None,
                        duration_ms: None,
                        scoring: Arc::default(),
                    },
                )
                .await
                .unwrap();
            assert_eq!(outcome.results.len(), 1);
            assert_eq!(outcome.results[0].provider_id, "lrclib");
            assert_eq!(
                outcome
                    .statuses
                    .iter()
                    .find(|status| status.provider_id == "netease")
                    .unwrap()
                    .health,
                ProviderHealth::Unavailable
            );
        });
    }

    #[test]
    fn partial_provider_failure_is_reported_as_degraded() {
        tauri::async_runtime::block_on(async {
            let outcome = single_mock_registry(Some("detail failed"), false)
                .search(
                    &reqwest::Client::new(),
                    &LyricsSearchInput {
                        title: "Hello".into(),
                        artist: "Adele".into(),
                        album: None,
                        duration_ms: None,
                        scoring: Arc::default(),
                    },
                )
                .await
                .unwrap();
            let status = &outcome.statuses[0];
            assert_eq!(status.health, ProviderHealth::Degraded);
            assert_eq!(
                status.message.as_deref(),
                Some("部分请求失败：detail failed")
            );
        });
    }

    #[test]
    fn successful_empty_provider_is_available() {
        tauri::async_runtime::block_on(async {
            let outcome = single_mock_registry(None, true)
                .search(
                    &reqwest::Client::new(),
                    &LyricsSearchInput {
                        title: "Missing".into(),
                        artist: "Artist".into(),
                        album: None,
                        duration_ms: None,
                        scoring: Arc::default(),
                    },
                )
                .await
                .unwrap();
            assert!(outcome.results.is_empty());
            assert_eq!(outcome.statuses[0].health, ProviderHealth::Available);
            assert_eq!(
                outcome.statuses[0].message.as_deref(),
                Some("连接正常，未找到同步歌词")
            );
        });
    }

    #[test]
    fn sorting_mode_switches_between_priority_and_score() {
        tauri::async_runtime::block_on(async {
            let client = reqwest::Client::new();
            let input = LyricsSearchInput {
                title: "Hello".into(),
                artist: "Adele".into(),
                album: None,
                duration_ms: None,
                scoring: Arc::default(),
            };
            let strict = mock_registry(ProviderOrderMode::Strict, false)
                .search(&client, &input)
                .await
                .unwrap();
            assert_eq!(strict.results.len(), 1);
            assert_eq!(strict.results[0].provider_id, "lrclib");
            let smart = mock_registry(ProviderOrderMode::Smart, false)
                .search(&client, &input)
                .await
                .unwrap();
            assert_eq!(smart.results.len(), 1);
            assert_eq!(smart.results[0].provider_id, "netease");
        });
    }

    #[test]
    fn concurrent_identical_searches_share_only_in_flight_work() {
        tauri::async_runtime::block_on(async {
            let calls = Arc::new(AtomicUsize::new(0));
            let registry = counting_registry(calls.clone());
            let client = reqwest::Client::new();
            let input = LyricsSearchInput {
                title: "Hello".into(),
                artist: "Adele".into(),
                album: None,
                duration_ms: None,
                scoring: Arc::default(),
            };

            let (first, second) = tokio::join!(
                registry.search(&client, &input),
                registry.search(&client, &input),
            );
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(first.unwrap().results[0].id, second.unwrap().results[0].id);

            registry.search(&client, &input).await.unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 2);

            let mut different = input.clone();
            different.title = "World".into();
            let _ = tokio::join!(
                registry.search(&client, &input),
                registry.search(&client, &different),
            );
            assert_eq!(calls.load(Ordering::SeqCst), 4);
        });
    }

    #[test]
    #[ignore = "需要访问外部歌词服务"]
    fn live_each_provider_returns_candidates() {
        tauri::async_runtime::block_on(async {
            let client = reqwest::Client::builder()
                .user_agent("Lyrics Plus integration test")
                .timeout(Duration::from_secs(8))
                .build()
                .unwrap();
            let input = LyricsSearchInput {
                title: "晴天".into(),
                artist: "周杰伦".into(),
                album: Some("叶惠美".into()),
                duration_ms: Some(269_000),
                scoring: Arc::default(),
            };
            for target in ["netease", "qqmusic", "kugou", "lrclib"] {
                let settings = ProviderSettings {
                    mode: ProviderOrderMode::Smart,
                    auto_apply_threshold: 60,
                    prefer_capabilities: false,
                    match_weights: MatchWeights::default(),
                    normalize_chinese: true,
                    providers: provider_definitions()
                        .into_iter()
                        .map(|(id, _)| ProviderPreference {
                            id: id.into(),
                            enabled: id == target,
                        })
                        .collect(),
                    title_filter_keywords: default_title_filter_keywords(),
                    amll_base_url: default_amll_base_url(),
                };
                let registry = ProviderRegistry::new(settings);
                let outcome = registry.search(&client, &input).await.unwrap();
                assert!(
                    !outcome.results.is_empty(),
                    "{target} did not return candidates"
                );
                assert!(outcome
                    .results
                    .iter()
                    .all(|result| result.provider_id == target));
            }
        });
    }
}
