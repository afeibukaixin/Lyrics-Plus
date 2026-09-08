use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::join_all;
use futures::stream::{self, StreamExt};

use crate::lyrics::{parse_lrc_with_options, semantic_fingerprint};

use super::super::{
    exact_provider_identity, prepare_title_filter_keywords_with_normalization, score_candidate,
    title_matches,
    LyricsProvider, LyricsSearchInput, LyricsSearchResult, ProviderCandidate,
    ProviderCandidateReport, ProviderError, ProviderErrorKind, ProviderHealth, ProviderOrderMode,
    ProviderSearchOutcome, ProviderSearchReport, ProviderSettings, ProviderStatusDetail,
    ScoringSettings, DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE,
};
use super::ProviderRegistry;

const MAX_PROVIDER_CONCURRENCY: usize = 4;

struct CandidateWork<'a> {
    provider: &'a dyn LyricsProvider,
    candidate: ProviderCandidate,
    score: f64,
}

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
        confirmed_artist_aliases: input.scoring.confirmed_artist_aliases.clone(),
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
    let mut candidates = Vec::new();
    let mut provider_elapsed_ms: HashMap<String, u64> = HashMap::new();
    // 使用下标作为异步流元素，避免借用的 trait object 出现在异步闭包参数中，
    // 否则外层 Future 需要 Send 时会触发 FnOnce 高阶生命周期推断失败。
    let active = &active;
    let outcomes = stream::iter(0..active.len())
        .map(move |index| async move {
            let provider = active[index];
            let started = Instant::now();
            let exact_id = input
                .platform_item_id
                .as_deref()
                .filter(|_| input.platform.as_deref() == Some(provider.id()));
            let request = if let Some(exact_id) = exact_id {
                provider.lookup_by_id(client, scoring_input, exact_id)
            } else {
                provider.search_candidates(client, scoring_input)
            };
            let outcome =
                tokio::time::timeout(provider_timeout(registry, provider.id()), request).await;
            (provider, started.elapsed(), outcome)
        })
        .buffer_unordered(MAX_PROVIDER_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    for (provider, elapsed, outcome) in outcomes {
        let elapsed_ms = duration_millis(elapsed);
        provider_elapsed_ms
            .entry(provider.id().to_owned())
            .and_modify(|total| *total = total.saturating_add(elapsed_ms))
            .or_insert(elapsed_ms);
        match outcome {
            Ok(Ok(mut report)) => {
                retain_valid_provider_candidates(provider.id(), &mut report.candidates);
                retain_title_matching_provider_candidates(scoring_input, &mut report.candidates);
                report.candidates.sort_by(|left, right| {
                    let left_score = score_candidate(scoring_input, &left.metadata_result());
                    let right_score = score_candidate(scoring_input, &right.metadata_result());
                    right_score.total_cmp(&left_score)
                });
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
                let provider_candidates = report
                    .candidates
                    .into_iter()
                    .map(|candidate| {
                        let mut metadata = candidate.metadata_result();
                        metadata.score = score_candidate(scoring_input, &metadata);
                        CandidateWork {
                            provider,
                            candidate,
                            score: metadata.score,
                        }
                    })
                    .collect::<Vec<_>>();
                candidates.extend(provider_candidates);
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

    let mut seen_candidate_ids = HashSet::new();
    candidates.retain(|work| {
        seen_candidate_ids.insert((
            work.candidate.provider_id.clone(),
            work.candidate.provider_item_id.clone(),
        ))
    });
    candidates.sort_by(|left, right| {
        right.score.total_cmp(&left.score).then_with(|| {
            priority
                .get(left.provider.id())
                .cmp(&priority.get(right.provider.id()))
        })
    });

    let mut exact_indices = Vec::new();
    let mut remaining_indices = Vec::new();
    for (index, work) in candidates.iter().enumerate() {
        if is_exact_candidate(input, &work.candidate) {
            exact_indices.push(index);
        } else {
            remaining_indices.push(index);
        }
    }
    let sort_fetch_indices = |indices: &mut Vec<usize>| {
        indices.sort_by(|left, right| {
            let left = &candidates[*left];
            let right = &candidates[*right];
            match settings.mode {
                ProviderOrderMode::Strict => priority
                    .get(left.provider.id())
                    .cmp(&priority.get(right.provider.id()))
                    .then_with(|| right.score.total_cmp(&left.score)),
                ProviderOrderMode::Smart => right.score.total_cmp(&left.score).then_with(|| {
                    priority
                        .get(left.provider.id())
                        .cmp(&priority.get(right.provider.id()))
                }),
            }
        });
    };
    sort_fetch_indices(&mut exact_indices);
    sort_fetch_indices(&mut remaining_indices);
    let mut fetch_indices = exact_indices;
    fetch_indices.extend(remaining_indices);
    let mut results = Vec::new();
    let mut next_fetch_index = 0;
    while next_fetch_index < fetch_indices.len() {
        let batch_end =
            (next_fetch_index + MAX_PROVIDER_CONCURRENCY).min(fetch_indices.len());
        let batch = &fetch_indices[next_fetch_index..batch_end];
        next_fetch_index = batch_end;
        let jobs = batch.iter().map(|index| {
            let work = &candidates[*index];
            async move {
                let started = Instant::now();
                let outcome = tokio::time::timeout(
                    provider_timeout(registry, work.provider.id()),
                    work.provider.fetch(client, scoring_input, &work.candidate),
                )
                .await;
                (*index, started.elapsed(), outcome)
            }
        });
        for (index, elapsed, outcome) in join_all(jobs).await {
            let work = &candidates[index];
            let elapsed_ms = duration_millis(elapsed);
            provider_elapsed_ms
                .entry(work.provider.id().to_owned())
                .and_modify(|total| *total = total.saturating_add(elapsed_ms))
                .or_insert(elapsed_ms);
            match outcome {
                Ok(Ok(Some(mut result))) => {
                    if result.provider_id != work.provider.id()
                        || result.id.trim().is_empty()
                        || result.lyrics.trim().is_empty()
                    {
                        log::debug!(
                            "丢弃来源返回的无效歌词正文：provider={} id={:?}",
                            work.provider.id(),
                            result.id
                        );
                        continue;
                    }
                    if !exact_provider_identity(scoring_input, &result)
                        && !title_matches(scoring_input, &result)
                    {
                        log::debug!(
                            "丢弃来源返回的标题不匹配歌词正文：provider={} id={:?}",
                            work.provider.id(),
                            result.id
                        );
                        continue;
                    }
                    result.score = score_candidate(scoring_input, &result);
                    results.push(result);
                }
                Ok(Ok(None)) => {}
                Ok(Err(error)) => {
                    errors.push(error.to_string());
                    registry.record_failure(work.provider, &error);
                    registry.record_status(
                        work.provider,
                        ProviderHealth::Degraded,
                        ProviderStatusDetail::Failure {
                            error_kind: error.kind,
                            status_code: error.status_code,
                        },
                    );
                }
                Err(_) => {
                    let error = ProviderError::new(
                        work.provider.id(),
                        ProviderErrorKind::Network,
                        "歌词正文获取超时",
                    );
                    errors.push(error.to_string());
                    registry.record_failure(work.provider, &error);
                    registry.record_status(
                        work.provider,
                        ProviderHealth::Degraded,
                        ProviderStatusDetail::Timeout,
                    );
                }
            }
        }
        deduplicate_fetched_results(&mut results);
    }

    deduplicate_fetched_results(&mut results);
    let any_success = !results.is_empty();

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
        provider_elapsed_ms,
        auto_decision_stable: true,
        auto_apply_threshold: settings.auto_apply_threshold,
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

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

pub(super) fn provider_timeout(registry: &ProviderRegistry, provider_id: &str) -> Duration {
    super::provider_manifests()
        .into_iter()
        .find(|manifest| manifest.id == provider_id)
        .and_then(|manifest| {
            (manifest.request_timeout_ms > 0).then_some(Duration::from_millis(u64::from(
                manifest.request_timeout_ms,
            )))
        })
        .unwrap_or(registry.timeout)
}

fn is_exact_candidate(input: &LyricsSearchInput, candidate: &ProviderCandidate) -> bool {
    input.platform.as_deref() == Some(candidate.provider_id.as_str())
        && input
            .platform_item_id
            .as_deref()
            .is_some_and(|id| id == candidate.provider_item_id)
}

/// 在线来源的元数据候选必须先通过与本地歌词相同的标题门槛；
/// 精确平台 ID 是可信身份，允许绕过文本标题匹配。
fn retain_title_matching_provider_candidates(
    input: &LyricsSearchInput,
    candidates: &mut Vec<ProviderCandidate>,
) {
    candidates.retain(|candidate| {
        if is_exact_candidate(input, candidate) {
            return true;
        }
        let metadata = candidate.metadata_result();
        let matches = title_matches(input, &metadata);
        if !matches {
            log::debug!(
                "丢弃来源返回的标题不匹配候选：provider={} id={:?}",
                candidate.provider_id,
                candidate.provider_item_id
            );
        }
        matches
    });
}

/// 只让带有来源标识和来源内歌曲标识的候选进入统一搜索结果。
/// provider_id 与 provider_item_id 共同构成可持久化的来源身份，不能只依赖其中一个字段。
fn retain_valid_provider_candidates(provider_id: &str, candidates: &mut Vec<ProviderCandidate>) {
    candidates.retain(|candidate| {
        if candidate.provider_id != provider_id || candidate.provider_item_id.trim().is_empty() {
            log::debug!(
                "丢弃缺少有效来源 ID 的歌词候选：provider={} result_provider={} id={:?}",
                provider_id,
                candidate.provider_id,
                candidate.provider_item_id
            );
            return false;
        }
        if candidate.title.contains('\u{FFFD}')
            || candidate
                .artists
                .iter()
                .any(|artist| artist.contains('\u{FFFD}'))
        {
            log::debug!(
                "丢弃包含替换字符的元数据候选：provider={} id={:?}",
                provider_id,
                candidate.provider_item_id
            );
            return false;
        }
        true
    });
}

fn deduplicate_fetched_results(results: &mut Vec<LyricsSearchResult>) {
    let mut seen = HashSet::new();
    results.retain(|result| {
        let fingerprint = parse_lrc_with_options(&result.lyrics, &result.source, false)
            .ok()
            .map(|document| semantic_fingerprint(&document))
            .filter(|fingerprint| !fingerprint.is_empty())
            .map(|fingerprint| format!("lrc:{fingerprint}"))
            .or_else(|| {
                let raw_fingerprint = result
                    .lyrics
                    .chars()
                    .filter(|character| !character.is_whitespace())
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                (!raw_fingerprint.is_empty()).then_some(format!("raw:{raw_fingerprint}"))
            });
        fingerprint.is_none_or(|fingerprint| seen.insert(fingerprint))
    });
}

fn report_status(report: &ProviderCandidateReport) -> (ProviderHealth, ProviderStatusDetail) {
    if let Some(warning) = &report.warning {
        return (
            ProviderHealth::Degraded,
            ProviderStatusDetail::PartialFailure {
                result_count: report.candidates.len(),
                error_kind: warning.kind.clone(),
            },
        );
    }
    (
        ProviderHealth::Available,
        ProviderStatusDetail::Success {
            result_count: report.candidates.len(),
        },
    )
}

pub(super) fn fetched_report_status(
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
