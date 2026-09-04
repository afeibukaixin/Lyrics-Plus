use crate::lyrics::provider::{
    LyricsSearchResult, ProviderOrderMode, DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE,
};
use crate::lyrics::{
    lyrics_quality_report, parse_lrc_with_options, semantic_fingerprint, LyricsDocument,
    LyricsQualityReport,
};
use crate::overlay_model::SecondaryDisplayMode;

pub(super) struct AnalyzedCandidate {
    pub(super) result: LyricsSearchResult,
    #[allow(dead_code)]
    pub(super) document: Option<LyricsDocument>,
    pub(super) quality: LyricsQualityReport,
    pub(super) fingerprint: String,
    pub(super) stable_index: usize,
    pub(super) is_local: bool,
}

pub(super) fn analyze_candidate(
    result: LyricsSearchResult,
    duration_ms: Option<u64>,
    duration_guard_enabled: bool,
    duration_tolerance_seconds: u8,
    stable_index: usize,
    is_local: bool,
) -> AnalyzedCandidate {
    let document = parse_lrc_with_options(&result.lyrics, &result.source, false).ok();
    let duration_tolerance_ms =
        duration_guard_enabled.then(|| u64::from(duration_tolerance_seconds).saturating_mul(1_000));
    let mut quality = document
        .as_ref()
        .map(|document| {
            lyrics_quality_report(
                document,
                duration_ms.or(result.duration_ms),
                duration_tolerance_ms,
            )
        })
        .unwrap_or(LyricsQualityReport {
            has_valid_synced_original: false,
            degraded_word_lines: 0,
            last_valid_time_ms: None,
            auto_applicable: false,
        });
    if !result.synced {
        quality.auto_applicable = false;
    }
    let fingerprint = document
        .as_ref()
        .map(semantic_fingerprint)
        .filter(|fingerprint| !fingerprint.is_empty())
        .unwrap_or_default();
    AnalyzedCandidate {
        result,
        document,
        quality,
        fingerprint,
        stable_index,
        is_local,
    }
}

/// 返回候选在质量排序链路中相对另一候选首次出现差异的字段，仅用于诊断日志。
fn quality_ranking_reason(
    left: &AnalyzedCandidate,
    right: &AnalyzedCandidate,
    prefer_capabilities: bool,
    _secondary_display: SecondaryDisplayMode,
) -> &'static str {
    if left.quality.auto_applicable != right.quality.auto_applicable {
        return "auto_applicable";
    }
    if prefer_capabilities {
        if candidate_capability_count(&left.result) != candidate_capability_count(&right.result) {
            return "capability_count";
        }
    } else if left.quality.degraded_word_lines != right.quality.degraded_word_lines {
        return "degraded_word_lines";
    }
    if left.result.score.total_cmp(&right.result.score) != std::cmp::Ordering::Equal {
        return "score";
    }
    if left.stable_index != right.stable_index {
        return "stable_order";
    }
    "tie"
}

fn smart_ranking_reason(
    left: &AnalyzedCandidate,
    right: &AnalyzedCandidate,
    prefer_capabilities: bool,
    secondary_display: SecondaryDisplayMode,
    top_score: f64,
    score_band: f64,
) -> &'static str {
    let left_in_band = smart_score_band_contains(left.result.score, top_score, score_band);
    let right_in_band = smart_score_band_contains(right.result.score, top_score, score_band);
    if left_in_band != right_in_band {
        return "score_band";
    }
    if !left_in_band {
        if left.result.score.total_cmp(&right.result.score) != std::cmp::Ordering::Equal {
            return "score";
        }
        if left.stable_index != right.stable_index {
            return "stable_order";
        }
        return "tie";
    }
    quality_ranking_reason(left, right, prefer_capabilities, secondary_display)
}

fn strict_ranking_reason(
    left: &AnalyzedCandidate,
    right: &AnalyzedCandidate,
    provider_order: &[String],
) -> &'static str {
    if provider_rank(left, provider_order) != provider_rank(right, provider_order) {
        return "provider_order";
    }
    if left.result.score.total_cmp(&right.result.score) != std::cmp::Ordering::Equal {
        return "score";
    }
    if left.stable_index != right.stable_index {
        return "stable_order";
    }
    "tie"
}

/// 记录排序后的候选摘要，不输出歌词正文或语义指纹内容。
fn log_ranked_candidates(
    candidates: &[AnalyzedCandidate],
    mode: ProviderOrderMode,
    provider_order: &[String],
    prefer_capabilities: bool,
    secondary_display: SecondaryDisplayMode,
    top_score: f64,
    score_band: f64,
) {
    let Some(winner) = candidates.first() else {
        return;
    };
    for (index, candidate) in candidates.iter().enumerate() {
        let reason = if index == 0 {
            "winner"
        } else {
            match mode {
                ProviderOrderMode::Strict => {
                    strict_ranking_reason(candidate, winner, provider_order)
                }
                ProviderOrderMode::Smart => smart_ranking_reason(
                    winner,
                    candidate,
                    prefer_capabilities,
                    secondary_display,
                    top_score,
                    score_band,
                ),
            }
        };
        log::debug!(
            "lyrics.rank candidate rank={} provider={} id={:?} reason={reason} score={:.4} stable_index={} provider_rank={} parse_ok={} valid_synced_original={} auto_applicable={} degraded_word_lines={} synced={} word_timing={} translation={} romanization={} capability_count={} capability_rank={:?}",
            index + 1,
            candidate.result.provider_id,
            candidate.result.id,
            candidate.result.score,
            candidate.stable_index,
            provider_rank(candidate, provider_order),
            candidate.document.is_some(),
            candidate.quality.has_valid_synced_original,
            candidate.quality.auto_applicable,
            candidate.quality.degraded_word_lines,
            candidate.result.synced,
            candidate.result.has_word_timing,
            candidate.result.has_translation,
            candidate.result.has_romanization,
            candidate_capability_count(&candidate.result),
            candidate_capability_rank(&candidate.result, secondary_display),
        );
    }
}

fn provider_rank(candidate: &AnalyzedCandidate, provider_order: &[String]) -> usize {
    if candidate.is_local {
        return 0;
    }
    provider_order
        .iter()
        .position(|provider| provider == &candidate.result.provider_id)
        .map(|index| index + 1)
        .unwrap_or(usize::MAX)
}

/// 与候选界面展示的四个能力标签保持一致，作为能力优选范围内的唯一能力分。
fn candidate_capability_count(result: &LyricsSearchResult) -> u8 {
    u8::from(result.synced)
        + u8::from(result.has_word_timing)
        + u8::from(result.has_translation)
        + u8::from(result.has_romanization)
}

pub(crate) fn candidate_capability_rank(
    result: &LyricsSearchResult,
    secondary_display: SecondaryDisplayMode,
) -> (u8, u8) {
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
}

fn quality_order(
    left: &AnalyzedCandidate,
    right: &AnalyzedCandidate,
    prefer_capabilities: bool,
    _secondary_display: SecondaryDisplayMode,
) -> std::cmp::Ordering {
    let base = right
        .quality
        .auto_applicable
        .cmp(&left.quality.auto_applicable);
    let with_capabilities = if prefer_capabilities {
        base.then_with(|| {
            candidate_capability_count(&right.result).cmp(&candidate_capability_count(&left.result))
        })
    } else {
        base.then_with(|| {
            left.quality
                .degraded_word_lines
                .cmp(&right.quality.degraded_word_lines)
        })
    };
    with_capabilities
        .then_with(|| right.result.score.total_cmp(&left.result.score))
        .then_with(|| left.stable_index.cmp(&right.stable_index))
}

fn smart_sort_order(
    left: &AnalyzedCandidate,
    right: &AnalyzedCandidate,
    prefer_capabilities: bool,
    secondary_display: SecondaryDisplayMode,
) -> std::cmp::Ordering {
    quality_order(left, right, prefer_capabilities, secondary_display)
}

fn smart_score_band_contains(score: f64, top_score: f64, score_band: f64) -> bool {
    top_score - score <= score_band + f64::EPSILON
}

fn smart_order_with_score_band(
    left: &AnalyzedCandidate,
    right: &AnalyzedCandidate,
    prefer_capabilities: bool,
    secondary_display: SecondaryDisplayMode,
    top_score: f64,
    score_band: f64,
) -> std::cmp::Ordering {
    let left_in_band = smart_score_band_contains(left.result.score, top_score, score_band);
    let right_in_band = smart_score_band_contains(right.result.score, top_score, score_band);
    right_in_band.cmp(&left_in_band).then_with(|| {
        if left_in_band {
            smart_sort_order(left, right, prefer_capabilities, secondary_display)
        } else {
            right
                .result
                .score
                .total_cmp(&left.result.score)
                .then_with(|| left.stable_index.cmp(&right.stable_index))
        }
    })
}

fn smart_order_with_threshold(
    left: &AnalyzedCandidate,
    right: &AnalyzedCandidate,
    prefer_capabilities: bool,
    secondary_display: SecondaryDisplayMode,
    top_score: f64,
    score_band: f64,
    auto_apply_threshold: u8,
) -> std::cmp::Ordering {
    let left_meets_threshold = left.result.score * 100.0 >= f64::from(auto_apply_threshold);
    let right_meets_threshold = right.result.score * 100.0 >= f64::from(auto_apply_threshold);
    if left_meets_threshold != right_meets_threshold {
        return right_meets_threshold.cmp(&left_meets_threshold);
    }
    smart_order_with_score_band(
        left,
        right,
        prefer_capabilities,
        secondary_display,
        top_score,
        score_band,
    )
}

pub(super) fn deduplicate_analyzed_candidates(
    candidates: &mut Vec<AnalyzedCandidate>,
    mode: ProviderOrderMode,
    provider_order: &[String],
    prefer_capabilities: bool,
    capability_preference_tolerance: u8,
    secondary_display: SecondaryDisplayMode,
    auto_apply_threshold: u8,
) {
    let top_score = candidates
        .iter()
        .map(|candidate| candidate.result.score)
        .max_by(|left, right| left.total_cmp(right))
        .unwrap_or_default();
    let score_band = if prefer_capabilities {
        f64::from(capability_preference_tolerance) / 100.0
    } else {
        f64::from(DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE) / 100.0
    };
    let mut deduplicated = Vec::with_capacity(candidates.len());
    for candidate in candidates.drain(..) {
        if candidate.fingerprint.is_empty() {
            deduplicated.push(candidate);
            continue;
        }
        let Some(existing_index) = deduplicated
            .iter()
            .position(|existing: &AnalyzedCandidate| existing.fingerprint == candidate.fingerprint)
        else {
            deduplicated.push(candidate);
            continue;
        };
        let existing = &deduplicated[existing_index];
        let (replace, reason) = match (existing.is_local, candidate.is_local) {
            (true, true) => {
                let quality = smart_order_with_threshold(
                    &candidate,
                    existing,
                    prefer_capabilities,
                    secondary_display,
                    top_score,
                    score_band,
                    auto_apply_threshold,
                );
                (
                    matches!(mode, ProviderOrderMode::Smart) && quality == std::cmp::Ordering::Less,
                    if matches!(mode, ProviderOrderMode::Smart) {
                        smart_ranking_reason(
                            &candidate,
                            existing,
                            prefer_capabilities,
                            secondary_display,
                            top_score,
                            score_band,
                        )
                    } else {
                        "strict_keeps_existing"
                    },
                )
            }
            (true, false) => (false, "local_precedence"),
            (false, true) => (true, "local_precedence"),
            (false, false) => {
                if matches!(mode, ProviderOrderMode::Strict) {
                    (
                        provider_rank(&candidate, provider_order)
                            < provider_rank(existing, provider_order),
                        strict_ranking_reason(&candidate, existing, provider_order),
                    )
                } else {
                    (
                        smart_order_with_threshold(
                            &candidate,
                            existing,
                            prefer_capabilities,
                            secondary_display,
                            top_score,
                            score_band,
                            auto_apply_threshold,
                        ) == std::cmp::Ordering::Less,
                        smart_ranking_reason(
                            &candidate,
                            existing,
                            prefer_capabilities,
                            secondary_display,
                            top_score,
                            score_band,
                        ),
                    )
                }
            }
        };
        log::debug!(
            "lyrics.rank dedup action={} reason={reason} kept_provider={} kept_id={:?} candidate_provider={} candidate_id={:?} mode={mode:?}",
            if replace { "replace_candidate" } else { "drop_candidate" },
            if replace {
                candidate.result.provider_id.as_str()
            } else {
                existing.result.provider_id.as_str()
            },
            if replace {
                &candidate.result.id
            } else {
                &existing.result.id
            },
            candidate.result.provider_id,
            candidate.result.id,
        );
        if replace {
            deduplicated[existing_index] = candidate;
        }
    }
    *candidates = deduplicated;
}

pub(super) fn sort_analyzed_candidates(
    candidates: &mut [AnalyzedCandidate],
    mode: ProviderOrderMode,
    provider_order: &[String],
    prefer_capabilities: bool,
    capability_preference_tolerance: u8,
    secondary_display: SecondaryDisplayMode,
    auto_apply_threshold: u8,
) {
    match mode {
        ProviderOrderMode::Strict => candidates.sort_by(|left, right| {
            provider_rank(left, provider_order)
                .cmp(&provider_rank(right, provider_order))
                .then_with(|| right.result.score.total_cmp(&left.result.score))
                .then_with(|| left.stable_index.cmp(&right.stable_index))
        }),
        ProviderOrderMode::Smart => {
            let score_band = if prefer_capabilities {
                f64::from(capability_preference_tolerance) / 100.0
            } else {
                f64::from(DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE) / 100.0
            };
            candidates.sort_by(|left, right| {
                right
                    .result
                    .score
                    .total_cmp(&left.result.score)
                    .then_with(|| left.stable_index.cmp(&right.stable_index))
            });
            if let Some(top_score) = candidates.first().map(|candidate| candidate.result.score) {
                let band_len = candidates
                    .iter()
                    .take_while(|candidate| {
                        smart_score_band_contains(candidate.result.score, top_score, score_band)
                            && candidate.result.score * 100.0 >= f64::from(auto_apply_threshold)
                    })
                    .count();
                candidates[..band_len].sort_by(|left, right| {
                    smart_sort_order(left, right, prefer_capabilities, secondary_display)
                });
            }
        }
    }
}

pub(super) fn can_auto_apply_analyzed(
    candidates: &[AnalyzedCandidate],
    threshold_percent: u8,
) -> bool {
    let Some(first) = candidates.first() else {
        return false;
    };
    if first.result.score * 100.0 < f64::from(threshold_percent) || !first.quality.auto_applicable {
        if first.result.score * 100.0 < f64::from(threshold_percent) {
            log::debug!(
                "歌词候选因相似度未达到阈值而拦截：score={:.4} threshold={threshold_percent}",
                first.result.score
            );
        } else {
            log::debug!(
                "歌词候选因质量门槛而拦截：provider={}",
                first.result.provider_id
            );
        }
        return false;
    }
    true
}

pub(super) fn log_ranked_search(
    candidates: &[AnalyzedCandidate],
    mode: ProviderOrderMode,
    provider_order: &[String],
    prefer_capabilities: bool,
    secondary_display: SecondaryDisplayMode,
    top_score: f64,
    score_band: f64,
) {
    log_ranked_candidates(
        candidates,
        mode,
        provider_order,
        prefer_capabilities,
        secondary_display,
        top_score,
        score_band,
    );
}
