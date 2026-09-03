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

        let config = state.config.snapshot();
        let mut presented = snapshot;
        presented.document = presented
            .document
            .map(|document| {
                document.converted_for_output(
                    config.lyrics.chinese_conversion,
                    config.lyrics.repair_simplified_japanese,
                )
            });
        let _ = app.emit("lyrics://runtime-changed", &presented);
    } else {
        let _ = app.emit("lyrics://runtime-changed", &snapshot);
    }
    crate::sync_lyrics_surfaces(app);
}

pub(crate) fn republish_lyrics_runtime(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let snapshot = state
        .lyrics_runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    publish_lyrics_runtime(app, snapshot);
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

const MAX_RANK_DIAGNOSTIC_RESULTS: usize = 24;

async fn perform_lyrics_search(
    state: &AppState,
    input: &LyricsSearchInput,
    intent: LyricsSearchIntent,
) -> Result<SearchResponse, String> {
    let (local_result, provider_result) = tokio::join!(
        search_local_lyrics(state, input),
        state
            .providers
            .search_with_cache(&state.http, input, intent.is_manual()),
    );
    let (
        mut local_results,
        local_auto_apply_threshold,
        local_duration_guard_enabled,
        local_duration_tolerance_seconds,
    ) = local_result?;
    let (
        mut outcome,
        fallback_statuses,
        fallback_mode,
        fallback_order,
        fallback_prefer,
        fallback_tolerance,
        fallback_duration_guard_enabled,
        fallback_duration_tolerance_seconds,
    ) = match provider_result {
        Ok(outcome) => (
            Some(outcome),
            Vec::<ProviderStatus>::new(),
            ProviderOrderMode::Smart,
            Vec::<String>::new(),
            true,
            DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE,
            local_duration_guard_enabled,
            local_duration_tolerance_seconds,
        ),
        Err(_error) if !local_results.is_empty() => {
            let view = state.providers.settings_view();
            (
                None,
                view.statuses,
                view.settings.mode,
                view.settings
                    .providers
                    .into_iter()
                    .map(|provider| provider.id)
                    .collect(),
                view.settings.prefer_capabilities,
                view.settings.capability_preference_tolerance,
                local_duration_guard_enabled,
                local_duration_tolerance_seconds,
            )
        }
        Err(error) => return Err(error),
    };
    let secondary_display = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .secondary_display;
    let had_local_results = !local_results.is_empty();
    let (
        mode,
        provider_order,
        prefer_capabilities,
        capability_preference_tolerance,
        provider_threshold,
        duration_guard_enabled,
        duration_tolerance_seconds,
    ) = outcome
        .as_ref()
        .map(|outcome| {
            (
                outcome.mode,
                outcome.provider_order.clone(),
                outcome.prefer_capabilities,
                outcome.capability_preference_tolerance,
                outcome.auto_apply_threshold,
                outcome.auto_apply_duration_guard_enabled,
                outcome.auto_apply_duration_tolerance_seconds,
            )
        })
        .unwrap_or((
            fallback_mode,
            fallback_order,
            fallback_prefer,
            fallback_tolerance,
            local_auto_apply_threshold,
            fallback_duration_guard_enabled,
            fallback_duration_tolerance_seconds,
        ));
    let online_results = outcome
        .as_mut()
        .map(|outcome| std::mem::take(&mut outcome.results))
        .unwrap_or_default();
    let online_count = online_results.len();
    let mut candidates = local_results
        .drain(..)
        .enumerate()
        .map(|(index, result)| {
            analyze_candidate(
                result,
                input.duration_ms,
                duration_guard_enabled,
                duration_tolerance_seconds,
                index,
                true,
            )
        })
        .collect::<Vec<_>>();
    let local_count = candidates.len();
    candidates.extend(
        online_results
            .into_iter()
            .enumerate()
            .map(|(index, result)| {
                analyze_candidate(
                    result,
                    input.duration_ms,
                    duration_guard_enabled,
                    duration_tolerance_seconds,
                    local_count + index,
                    false,
                )
            }),
    );
    let analyzed_count = candidates.len();
    deduplicate_analyzed_candidates(
        &mut candidates,
        mode,
        &provider_order,
        prefer_capabilities,
        capability_preference_tolerance,
        secondary_display,
        provider_threshold,
    );
    let deduplicated_count = candidates.len();
    sort_analyzed_candidates(
        &mut candidates,
        mode,
        &provider_order,
        prefer_capabilities,
        capability_preference_tolerance,
        secondary_display,
        provider_threshold,
    );
    let auto_apply = can_auto_apply_analyzed(&candidates, provider_threshold);
    let sorted_count = candidates.len();
    candidates.truncate(MAX_RANK_DIAGNOSTIC_RESULTS);
    let returned_count = candidates.len();
    let score_band = if prefer_capabilities {
        f64::from(capability_preference_tolerance) / 100.0
    } else {
        f64::from(DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE) / 100.0
    };
    let top_score = candidates
        .first()
        .map(|candidate| candidate.result.score)
        .unwrap_or_default();
    log::debug!(
        "lyrics.rank search title={:?} artist={:?} intent={intent:?} mode={mode:?} prefer_capabilities={prefer_capabilities} capability_tolerance_percent={capability_preference_tolerance} score_band={score_band:.4} secondary_display={secondary_display:?} auto_apply_threshold_percent={provider_threshold} duration_guard_enabled={duration_guard_enabled} duration_tolerance_seconds={duration_tolerance_seconds} local_candidates={local_count} online_candidates={online_count} analyzed={analyzed_count} deduplicated={deduplicated_count} sorted={sorted_count} returned={returned_count} omitted={} auto_apply={auto_apply} provider_order={provider_order:?}",
        input.title,
        input.artist,
        sorted_count.saturating_sub(returned_count),
    );
    log_ranked_candidates(
        &candidates,
        mode,
        &provider_order,
        prefer_capabilities,
        secondary_display,
        top_score,
        score_band,
    );
    let results = candidates
        .into_iter()
        .map(|candidate| candidate.result)
        .collect::<Vec<_>>();
    Ok(SearchResponse {
        auto_apply,
        results,
        provider_statuses: outcome
            .as_ref()
            .map(|outcome| outcome.statuses.clone())
            .unwrap_or(fallback_statuses),
        error: (!had_local_results)
            .then(|| outcome.and_then(|outcome| outcome.error))
            .flatten(),
    })
}

struct AnalyzedCandidate {
    result: LyricsSearchResult,
    #[allow(dead_code)]
    document: Option<LyricsDocument>,
    quality: LyricsQualityReport,
    fingerprint: String,
    stable_index: usize,
    is_local: bool,
}

fn analyze_candidate(
    result: LyricsSearchResult,
    duration_ms: Option<u64>,
    duration_guard_enabled: bool,
    duration_tolerance_seconds: u8,
    stable_index: usize,
    is_local: bool,
) -> AnalyzedCandidate {
    let document = parse_lrc_with_options(&result.lyrics, &result.source, false).ok();
    let duration_tolerance_ms = duration_guard_enabled
        .then(|| u64::from(duration_tolerance_seconds).saturating_mul(1_000));
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
        if candidate_capability_count(&left.result)
            != candidate_capability_count(&right.result)
        {
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
            candidate_capability_count(&right.result)
                .cmp(&candidate_capability_count(&left.result))
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
    let left_meets_threshold =
        left.result.score * 100.0 >= f64::from(auto_apply_threshold);
    let right_meets_threshold =
        right.result.score * 100.0 >= f64::from(auto_apply_threshold);
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

fn deduplicate_analyzed_candidates(
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

fn sort_analyzed_candidates(
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
                            && candidate.result.score * 100.0
                                >= f64::from(auto_apply_threshold)
                    })
                    .count();
                candidates[..band_len].sort_by(|left, right| {
                    smart_sort_order(left, right, prefer_capabilities, secondary_display)
                });
            }
        }
    }
}

fn can_auto_apply_analyzed(candidates: &[AnalyzedCandidate], threshold_percent: u8) -> bool {
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

async fn search_local_lyrics(
    state: &AppState,
    input: &LyricsSearchInput,
) -> Result<(Vec<LyricsSearchResult>, u8, bool, u8), String> {
    let (input, threshold, duration_guard_enabled, duration_tolerance_seconds) =
        state.providers.local_search_context(input)?;
    let storage = state.storage.clone();
    let results = tauri::async_runtime::spawn_blocking(move || storage.search_local_lyrics(&input))
        .await
        .map_err(|error| format!("本地歌词搜索任务失败：{error}"))??;
    Ok((
        results,
        threshold,
        duration_guard_enabled,
        duration_tolerance_seconds,
    ))
}

fn save_automatic_search_result(
    state: &AppState,
    track_key: &str,
    title: &str,
    artist: &str,
    result: &LyricsSearchResult,
) -> Result<LyricsDocument, String> {
    let request = SaveRequest {
        track_key,
        title,
        artist,
        source: &result.source,
        raw: &result.lyrics,
        provider_id: Some(&result.provider_id),
        provider_item_id: Some(&result.id),
        kind: SaveKind::Automatic,
    };
    if result.provider_id == LOCAL_PROVIDER_ID {
        state.storage.associate_local_lyrics(request)
    } else {
        state.storage.save(request)
    }
}

async fn search_lyrics_for_session(
    state: &AppState,
    track_key: &str,
    input: LyricsSearchInput,
    intent: LyricsSearchIntent,
) -> Result<SearchResponse, String> {
    if input.title.trim().is_empty() || input.artist.trim().is_empty() {
        return Err("搜索歌词需要歌曲名和歌手".into());
    }

    let request_key = LyricsSearchRequestKey::new(&input);
    let reuse_completed = matches!(intent, LyricsSearchIntent::Automatic);
    let (activation, request_id, flight, should_debounce) = {
        let mut session = state
            .lyrics_search_session
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if session.track_key.as_deref() != Some(track_key) {
            return Err("当前歌曲已发生变化".into());
        }
        let same_request = session.request_key.as_ref() == Some(&request_key);
        if reuse_completed && same_request {
            if let Some(completed) = &session.completed {
                return completed.clone();
            }
        }
        if same_request {
            if let Some(flight) = &session.in_flight {
                (
                    session.activation,
                    session.request_id,
                    flight.clone(),
                    intent.uses_debounce(),
                )
            } else {
                session.request_id = session.request_id.wrapping_add(1);
                session.completed = None;
                let flight = Arc::new(LyricsSearchFlight::new());
                session.in_flight = Some(flight.clone());
                (
                    session.activation,
                    session.request_id,
                    flight,
                    intent.uses_debounce(),
                )
            }
        } else {
            session.request_id = session.request_id.wrapping_add(1);
            session.request_key = Some(request_key.clone());
            session.completed = None;
            let flight = Arc::new(LyricsSearchFlight::new());
            session.in_flight = Some(flight.clone());
            (
                session.activation,
                session.request_id,
                flight,
                intent.uses_debounce(),
            )
        }
    };

    if should_debounce {
        let debounce = state.providers.auto_search_debounce();
        if !debounce.is_zero() {
            log::debug!(
                "歌词搜索进入防抖等待：track_key={track_key} intent={intent:?} debounce_ms={}",
                debounce.as_millis()
            );
            tokio::time::sleep(debounce).await;
            let still_active = {
                let session = state
                    .lyrics_search_session
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                session.activation == activation
                    && session.request_id == request_id
                    && session.track_key.as_deref() == Some(track_key)
            };
            if !still_active {
                log::debug!(
                    "歌词搜索防抖取消：track_key={track_key} intent={intent:?} 原会话已失效"
                );
                return Err(LYRICS_SEARCH_INVALIDATED.into());
            }
        }
    }

    let result = flight
        .get_or_init(|| perform_lyrics_search(state, &input, intent))
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

fn completed_lyrics_search(state: &AppState, track_key: &str) -> Option<SearchResponse> {
    let session = state
        .lyrics_search_session
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if session.track_key.as_deref() != Some(track_key) {
        return None;
    }
    session
        .completed
        .as_ref()
        .and_then(|completed| completed.as_ref().ok().cloned())
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
    let playback = state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    sync_lyrics_runtime_inner(app, &playback, true);
}

pub(crate) fn sync_lyrics_runtime(app: &tauri::AppHandle, playback: &PlaybackSnapshot) {
    sync_lyrics_runtime_inner(app, playback, false);
}

fn sync_lyrics_runtime_inner(app: &tauri::AppHandle, playback: &PlaybackSnapshot, force: bool) {
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
    if force || current_key != next_key {
        if let (Some(track_key), Some(title), Some(artist)) = (
            next_key.as_deref(),
            playback.title.as_deref(),
            playback.artist.as_deref(),
        ) {
            if let Err(error) = state.storage.ensure_track_alias(
                track_key,
                title,
                artist,
                playback.album.as_deref(),
                playback.duration_ms,
            ) {
                log::warn!("整理当前歌曲歌词关联失败：{error}");
            }
        }
    }
    if !force && current_key == next_key {
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
        match state.storage.load_with_status(&track_key) {
            Ok(crate::storage::LyricsLoadResult::Ready(document)) => {
                if !current() {
                    return;
                }
                publish_lyrics_runtime(
                    &worker_app,
                    LyricsRuntimeSnapshot {
                        track_key: Some(track_key.clone()),
                        document: Some(document),
                        status: LyricsRuntimeStatus::Ready,
                        error: None,
                    },
                );
                return;
            }
            Ok(crate::storage::LyricsLoadResult::Invalid(error)) => {
                log::warn!("当前歌曲歌词关联内容无效，准备解除关联：{error}");
                if let Err(invalidation_error) = state.storage.remove(&track_key) {
                    if current() {
                        publish_lyrics_runtime(
                            &worker_app,
                            LyricsRuntimeSnapshot {
                                track_key: Some(track_key),
                                document: None,
                                status: LyricsRuntimeStatus::Error,
                                error: Some(format!("解除无效歌词关联失败：{invalidation_error}")),
                            },
                        );
                    }
                    return;
                }
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
            Ok(crate::storage::LyricsLoadResult::Missing) => {}
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
        match search_lyrics_for_session(&state, &track_key, input, LyricsSearchIntent::Automatic)
            .await
        {
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
                        save_automatic_search_result(&state, &track_key, &title, &artist, result)
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
            Err(error) if current() && error == LYRICS_SEARCH_INVALIDATED => {
                publish_lyrics_runtime(
                    &worker_app,
                    LyricsRuntimeSnapshot {
                        track_key: Some(track_key),
                        document: None,
                        status: LyricsRuntimeStatus::NotFound,
                        error: None,
                    },
                )
            }
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
