use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::future::join_all;
use serde::{Deserialize, Serialize};
use strsim::normalized_levenshtein;

use super::kugou::KugouProvider;
use super::lrclib::LrcLibProvider;
use super::netease::NeteaseProvider;
use super::qqmusic::QqMusicProvider;

pub const LRCLIB_DISPLAY_NAME: &str = "LRCLIB";
pub const KUGOU_DISPLAY_NAME: &str = "Kugou";
pub const QQMUSIC_DISPLAY_NAME: &str = "QQMusic";
pub const NETEASE_DISPLAY_NAME: &str = "Netease";

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchInput {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOrderMode {
    #[default]
    Smart,
    Strict,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPreference {
    pub id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct ProviderSettings {
    pub mode: ProviderOrderMode,
    pub providers: Vec<ProviderPreference>,
    #[serde(default = "default_auto_apply_threshold")]
    pub auto_apply_threshold: u8,
}

const fn default_auto_apply_threshold() -> u8 {
    60
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            mode: ProviderOrderMode::Smart,
            providers: provider_definitions()
                .into_iter()
                .map(|(id, _)| ProviderPreference {
                    id: id.into(),
                    enabled: id == "lrclib",
                })
                .collect(),
            auto_apply_threshold: default_auto_apply_threshold(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsView {
    pub settings: ProviderSettings,
    pub statuses: Vec<ProviderStatus>,
}

pub struct ProviderSearchOutcome {
    pub results: Vec<LyricsSearchResult>,
    pub statuses: Vec<ProviderStatus>,
    pub auto_apply_threshold: u8,
}

pub trait LyricsProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn search<'a>(
        &'a self,
        client: &'a reqwest::Client,
        input: &'a LyricsSearchInput,
    ) -> ProviderFuture<'a, Vec<LyricsSearchResult>>;
}

pub struct ProviderRegistry {
    providers: Vec<Box<dyn LyricsProvider>>,
    settings: RwLock<ProviderSettings>,
    statuses: RwLock<HashMap<String, ProviderStatus>>,
    timeout: Duration,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new(ProviderSettings::default())
    }
}

impl ProviderRegistry {
    pub fn new(settings: ProviderSettings) -> Self {
        let providers: Vec<Box<dyn LyricsProvider>> = vec![
            Box::new(NeteaseProvider),
            Box::new(QqMusicProvider),
            Box::new(KugouProvider),
            Box::new(LrcLibProvider),
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
            settings: RwLock::new(settings),
            statuses: RwLock::new(statuses),
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

    pub fn set_settings(&self, settings: ProviderSettings) -> Result<ProviderSettingsView, String> {
        validate_settings(&settings)?;
        *self
            .settings
            .write()
            .unwrap_or_else(|error| error.into_inner()) = settings;
        Ok(self.settings_view())
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

        let jobs = enabled.iter().map(|provider| async move {
            let outcome = tokio::time::timeout(self.timeout, provider.search(client, input)).await;
            (*provider, outcome)
        });
        let outcomes = join_all(jobs).await;
        let mut results = Vec::new();
        let mut errors = Vec::new();
        let mut any_success = false;
        for (provider, outcome) in outcomes {
            match outcome {
                Ok(Ok(mut provider_results)) => {
                    any_success = true;
                    self.record_status(provider, ProviderHealth::Available, None);
                    results.append(&mut provider_results);
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

        deduplicate(&mut results);
        results.sort_by(|left, right| match settings.mode {
            ProviderOrderMode::Strict => priority
                .get(left.provider_id.as_str())
                .cmp(&priority.get(right.provider_id.as_str()))
                .then_with(|| right.score.total_cmp(&left.score)),
            ProviderOrderMode::Smart => {
                let score_delta = (left.score - right.score).abs();
                if score_delta > 0.035 {
                    right.score.total_cmp(&left.score)
                } else {
                    priority
                        .get(left.provider_id.as_str())
                        .cmp(&priority.get(right.provider_id.as_str()))
                        .then_with(|| right.score.total_cmp(&left.score))
                }
            }
        });
        results.truncate(24);
        if !any_success && !errors.is_empty() {
            Err(errors.join("；"))
        } else {
            Ok(ProviderSearchOutcome {
                results,
                statuses: self.statuses(),
                auto_apply_threshold: settings.auto_apply_threshold,
            })
        }
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
        };
        match tokio::time::timeout(self.timeout, provider.search(client, &input)).await {
            Ok(Ok(results)) => {
                let message = Some(format!("连接正常，返回 {} 个候选", results.len()));
                self.record_status(provider.as_ref(), ProviderHealth::Available, message);
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

fn provider_definitions() -> [(&'static str, &'static str); 4] {
    [
        ("lrclib", LRCLIB_DISPLAY_NAME),
        ("kugou", KUGOU_DISPLAY_NAME),
        ("qqmusic", QQMUSIC_DISPLAY_NAME),
        ("netease", NETEASE_DISPLAY_NAME),
    ]
}

pub(crate) fn validate_settings(settings: &ProviderSettings) -> Result<(), String> {
    if settings.auto_apply_threshold > 100 {
        return Err("自动匹配相似度必须在 0–100 之间".into());
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
    Ok(())
}

pub(crate) fn complete_settings(settings: &mut ProviderSettings) {
    for (id, _) in provider_definitions() {
        if !settings.providers.iter().any(|provider| provider.id == id) {
            settings.providers.push(ProviderPreference {
                id: id.into(),
                enabled: false,
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

fn normalise(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect()
}

pub fn score_candidate(input: &LyricsSearchInput, result: &LyricsSearchResult) -> f64 {
    let title = normalized_levenshtein(&normalise(&input.title), &normalise(&result.title));
    let artist = normalized_levenshtein(&normalise(&input.artist), &normalise(&result.artist));
    let album = match (&input.album, &result.album) {
        (Some(expected), Some(actual)) => {
            normalized_levenshtein(&normalise(expected), &normalise(actual))
        }
        _ => 0.6,
    };
    let duration = match (input.duration_ms, result.duration_ms) {
        (Some(expected), Some(actual)) => {
            let delta = expected.abs_diff(actual) as f64;
            (1.0 - delta / 12_000.0).clamp(0.0, 1.0)
        }
        _ => 0.6,
    };
    (title * 0.39
        + artist * 0.36
        + album * 0.08
        + duration * 0.17
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

    #[test]
    fn defaults_to_the_public_provider_only() {
        let settings = ProviderSettings::default();
        assert_eq!(
            settings
                .providers
                .iter()
                .filter(|provider| provider.enabled)
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            ["lrclib"]
        );
    }

    struct MockProvider {
        id: &'static str,
        score: f64,
        fails: bool,
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
        ) -> ProviderFuture<'a, Vec<LyricsSearchResult>> {
            Box::pin(async move {
                if self.fails {
                    return Err(ProviderError {
                        provider_id: self.id.into(),
                        kind: ProviderErrorKind::Network,
                        message: "mock failure".into(),
                    });
                }
                Ok(vec![result(
                    self.id,
                    self.score,
                    &format!("[00:01]{}", self.id),
                )])
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
        };
        assert!(score_candidate(&input, &result("lrclib", 0.0, "line")) > 0.98);
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
        })
        .is_err());
        assert!(validate_settings(&ProviderSettings {
            mode: ProviderOrderMode::Smart,
            providers: vec![ProviderPreference {
                id: "lrclib".into(),
                enabled: false,
            }],
            auto_apply_threshold: 60,
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
        assert_eq!(
            settings
                .providers
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            vec!["lrclib", "kugou", "qqmusic", "netease"]
        );
    }

    #[test]
    fn explicitly_saved_legacy_order_is_preserved() {
        let registry = ProviderRegistry::default();
        let settings = ProviderSettings {
            mode: ProviderOrderMode::Smart,
            auto_apply_threshold: 60,
            providers: ["netease", "qqmusic", "kugou", "lrclib"]
                .into_iter()
                .map(|id| ProviderPreference {
                    id: id.into(),
                    enabled: true,
                })
                .collect(),
        };

        let view = registry.set_settings(settings.clone()).unwrap();

        assert_eq!(view.settings, settings);
    }

    fn mock_registry(mode: ProviderOrderMode, netease_fails: bool) -> ProviderRegistry {
        let settings = ProviderSettings {
            mode,
            auto_apply_threshold: 60,
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
                }),
                Box::new(MockProvider {
                    id: "netease",
                    score: 0.98,
                    fails: netease_fails,
                }),
            ],
            settings: RwLock::new(settings),
            statuses: RwLock::new(statuses),
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
    fn sorting_mode_switches_between_priority_and_score() {
        tauri::async_runtime::block_on(async {
            let client = reqwest::Client::new();
            let input = LyricsSearchInput {
                title: "Hello".into(),
                artist: "Adele".into(),
                album: None,
                duration_ms: None,
            };
            let strict = mock_registry(ProviderOrderMode::Strict, false)
                .search(&client, &input)
                .await
                .unwrap();
            assert_eq!(strict.results[0].provider_id, "lrclib");
            let smart = mock_registry(ProviderOrderMode::Smart, false)
                .search(&client, &input)
                .await
                .unwrap();
            assert_eq!(smart.results[0].provider_id, "netease");
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
            };
            for target in ["netease", "qqmusic", "kugou", "lrclib"] {
                let settings = ProviderSettings {
                    mode: ProviderOrderMode::Smart,
                    auto_apply_threshold: 60,
                    providers: provider_definitions()
                        .into_iter()
                        .map(|(id, _)| ProviderPreference {
                            id: id.into(),
                            enabled: id == target,
                        })
                        .collect(),
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
