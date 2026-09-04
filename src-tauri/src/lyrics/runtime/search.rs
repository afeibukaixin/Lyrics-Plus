use std::sync::Arc;

use crate::lyrics::provider::{
    LyricsSearchInput, LyricsSearchResult, ProviderOrderMode, ProviderStatus,
    DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE,
};
use crate::lyrics::LyricsDocument;
use crate::state::AppState;
use crate::storage::{SaveKind, SaveRequest, LOCAL_PROVIDER_ID};

use super::model::{
    LyricsSearchFlight, LyricsSearchIntent, LyricsSearchRequestKey, SearchResponse,
    LYRICS_SEARCH_INVALIDATED,
};
use super::ranking::{
    analyze_candidate, can_auto_apply_analyzed, deduplicate_analyzed_candidates, log_ranked_search,
    sort_analyzed_candidates,
};

const MAX_RANK_DIAGNOSTIC_RESULTS: usize = 24;

pub(super) fn reset_lyrics_search_session(state: &AppState, track_key: Option<String>) {
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

pub(crate) fn invalidate_lyrics_search_session(state: &AppState) {
    let track_key = state
        .lyrics_search_session
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .track_key
        .clone();
    reset_lyrics_search_session(state, track_key);
}

pub(super) async fn perform_lyrics_search(
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
    log_ranked_search(
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

pub(super) fn save_automatic_search_result(
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

pub(crate) async fn search_lyrics_for_session(
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

pub(crate) fn completed_lyrics_search(state: &AppState, track_key: &str) -> Option<SearchResponse> {
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
