use std::collections::HashMap;
use std::sync::Arc;

use futures::future::join_all;

use super::super::{
    prepare_title_filter_keywords_with_normalization, LyricsSearchInput, LyricsSearchResult,
    ProviderError, ProviderErrorKind, ProviderHealth, ProviderOrderMode, ProviderSearchOutcome,
    ProviderSearchReport, ProviderSettings, ProviderStatusDetail, ScoringSettings,
    DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE,
};
use super::ProviderRegistry;

pub(super) fn with_scoring_settings(
    input: &LyricsSearchInput,
    settings: &ProviderSettings,
) -> Result<LyricsSearchInput, String> {
    let mut scoring_input = input.clone();
    scoring_input.scoring = Arc::new(ScoringSettings {
        title_filter_keywords: prepare_title_filter_keywords_with_normalization(
            &settings.title_filter_keywords,
            settings.normalize_chinese,
        )?,
        match_weights: settings.match_weights,
        normalize_chinese: settings.normalize_chinese,
    });
    Ok(scoring_input)
}

pub(super) async fn search_once(
    registry: &ProviderRegistry,
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
            registry
                .providers
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

    let mut active = Vec::with_capacity(enabled.len());
    let mut errors = Vec::new();
    for provider in &enabled {
        if let Some(error) = registry.cooldown_error(provider.id()) {
            log::debug!(
                "歌词源处于冷却中，跳过请求：{}：{}",
                provider.id(),
                error.message
            );
            errors.push(error.to_string());
            registry.record_cooldown_status(*provider, &error);
        } else {
            active.push(*provider);
        }
    }
    let scoring_input = with_scoring_settings(input, &settings)?;
    let scoring_input = &scoring_input;
    let jobs = active.iter().map(|provider| async move {
        let outcome =
            tokio::time::timeout(registry.timeout, provider.search(client, scoring_input)).await;
        (*provider, outcome)
    });
    let outcomes = join_all(jobs).await;
    let mut results = Vec::new();
    let mut any_success = false;
    for (provider, outcome) in outcomes {
        match outcome {
            Ok(Ok(mut report)) => {
                any_success = true;
                retain_valid_provider_results(provider.id(), &mut report.results);
                let (health, detail) = report_status(&report);
                if let Some(warning) = &report.warning {
                    log::debug!(
                        "歌词源搜索部分失败：provider={} kind={:?} status={:?} message={}",
                        provider.id(),
                        &warning.kind,
                        warning.status_code,
                        &warning.message
                    );
                    registry.record_failure(provider, warning);
                } else {
                    registry.record_success(provider.id());
                }
                registry.record_status(provider, health, detail);
                results.append(&mut report.results);
            }
            Ok(Err(error)) => {
                errors.push(error.to_string());
                registry.record_failure(provider, &error);
                registry.record_status(
                    provider,
                    ProviderHealth::Unavailable,
                    ProviderStatusDetail::Failure {
                        error_kind: error.kind,
                        status_code: error.status_code,
                    },
                );
            }
            Err(_) => {
                let error =
                    ProviderError::new(provider.id(), ProviderErrorKind::Network, "搜索超时");
                errors.push(error.to_string());
                registry.record_failure(provider, &error);
                registry.record_status(
                    provider,
                    ProviderHealth::Unavailable,
                    ProviderStatusDetail::Timeout,
                );
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
            let score_band = if settings.prefer_capabilities {
                f64::from(settings.capability_preference_tolerance) / 100.0
            } else {
                f64::from(DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE) / 100.0
            };
            results.sort_by(|left, right| right.score.total_cmp(&left.score));
            if let Some(top_score) = results.first().map(|result| result.score) {
                let band_len = results
                    .iter()
                    .take_while(|result| top_score - result.score <= score_band + f64::EPSILON)
                    .count();
                results[..band_len].sort_by(|left, right| {
                    priority
                        .get(left.provider_id.as_str())
                        .cmp(&priority.get(right.provider_id.as_str()))
                        .then_with(|| right.score.total_cmp(&left.score))
                });
            }
        }
    }
    let error = (!any_success && !errors.is_empty()).then(|| {
        log::debug!("歌词源搜索失败：{}", errors.join("；"));
        "provider_search_failed".to_string()
    });
    Ok(ProviderSearchOutcome {
        results,
        statuses: registry.statuses_for(&enabled_ids),
        auto_apply_threshold: settings.auto_apply_threshold,
        auto_apply_duration_guard_enabled: settings.auto_apply_duration_guard_enabled,
        auto_apply_duration_tolerance_seconds: settings.auto_apply_duration_tolerance_seconds,
        prefer_capabilities: settings.prefer_capabilities,
        capability_preference_tolerance: settings.capability_preference_tolerance,
        mode: settings.mode,
        provider_order: settings
            .providers
            .iter()
            .map(|provider| provider.id.clone())
            .collect(),
        error,
    })
}

/// 只让带有来源标识和来源内歌曲标识的候选进入统一搜索结果。
/// provider_id 与 id 共同构成可持久化的来源身份，不能只依赖其中一个字段。
fn retain_valid_provider_results(provider_id: &str, results: &mut Vec<LyricsSearchResult>) {
    results.retain(|result| {
        if result.provider_id != provider_id || result.id.trim().is_empty() {
            log::debug!(
                "丢弃缺少有效来源 ID 的歌词候选：provider={} result_provider={} id={:?}",
                provider_id,
                result.provider_id,
                result.id
            );
            return false;
        }
        if result.lyrics.contains('\u{FFFD}') {
            log::debug!(
                "丢弃包含替换字符的歌词候选：provider={} id={:?}",
                provider_id,
                result.id
            );
            return false;
        }
        true
    });
}

pub(super) fn report_status(
    report: &ProviderSearchReport,
) -> (ProviderHealth, ProviderStatusDetail) {
    if let Some(warning) = &report.warning {
        return (
            ProviderHealth::Degraded,
            ProviderStatusDetail::PartialFailure {
                result_count: report.results.len(),
                error_kind: warning.kind.clone(),
            },
        );
    }
    (
        ProviderHealth::Available,
        ProviderStatusDetail::Success {
            result_count: report.results.len(),
        },
    )
}

impl ProviderRegistry {
    pub(super) async fn search_once(
        &self,
        client: &reqwest::Client,
        input: &LyricsSearchInput,
        settings: ProviderSettings,
    ) -> Result<ProviderSearchOutcome, String> {
        search_once(self, client, input, settings).await
    }
}
