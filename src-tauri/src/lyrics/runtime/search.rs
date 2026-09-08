use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::Emitter;

use crate::lyrics::provider::{
    has_identity_conflict, LyricsSearchInput, LyricsSearchResult, ProviderOrderMode,
    ProviderStatus, DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE,
};
use crate::lyrics::LyricsDocument;
use crate::state::AppState;
use crate::storage::{SaveKind, SaveRequest, LOCAL_PROVIDER_ID};

use super::model::{
    LyricsCandidateRef, LyricsSearchFlight, LyricsSearchIntent, LyricsSearchProgress,
    LyricsSearchRequestKey, SearchResponse, LYRICS_SEARCH_INVALIDATED,
};
use super::ranking::{
    analyze_candidate, auto_apply_analyzed, deduplicate_analyzed_candidates, log_ranked_search,
    sort_analyzed_candidates,
};

const PROVIDER_STATUSES_EVENT: &str = "lyrics://provider-statuses";
static NEXT_UNTRACKED_RUN_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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
    app: tauri::AppHandle,
    state: &AppState,
    track_key: &str,
    input: &LyricsSearchInput,
    intent: LyricsSearchIntent,
) -> Result<SearchResponse, String> {
    let started = Instant::now();
    let identify_started = Instant::now();
    let identify_status = match state.storage.ensure_lyrics_observation(track_key, input) {
        Ok(_) => "completed",
        Err(error) => {
            log::warn!("保存歌词身份观察失败，继续搜索：{error}");
            "failed"
        }
    };
    let identify_elapsed = identify_started.elapsed();
    let binding_started = Instant::now();
    let (input, binding_status) = match state
        .storage
        .enrich_lyrics_input_with_confirmed_aliases(track_key, input)
    {
        Ok(input) => (input, "completed"),
        Err(error) => {
            log::warn!("读取已确认歌手别名失败，使用原始搜索参数继续：{error}");
            (input.clone(), "failed")
        }
    };
    // 评分输入贯穿整轮搜索，确保排名、自动采用和搜索摘要使用同一套配置。
    let (input, auto_apply_threshold) = state.providers.scoring_context(&input)?;
    let binding_elapsed = binding_started.elapsed();
    let run_id = match state.storage.begin_lyrics_search_run(
        track_key,
        match intent {
            LyricsSearchIntent::Automatic | LyricsSearchIntent::Refresh => "automatic",
            LyricsSearchIntent::Manual => "manual",
        },
        &input.title,
        &input.artist,
    ) {
        Ok(run_id) => Some(run_id),
        Err(error) => {
            log::warn!("创建歌词搜索过程记录失败，继续搜索：{error}");
            None
        }
    };
    // 存储不可用时仍为前端生成唯一 runId，避免重新搜索串入上一轮实时状态。
    let fallback_run_id = format!(
        "untracked-{}-{}",
        started.elapsed().as_nanos(),
        NEXT_UNTRACKED_RUN_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    );
    let event_run_id = run_id.as_deref().unwrap_or(&fallback_run_id);
    emit_search_progress(
        &app,
        state,
        run_id.as_deref(),
        event_run_id,
        "identify",
        None,
        identify_status,
        identify_elapsed,
        started.elapsed(),
        0,
    );
    emit_search_progress(
        &app,
        state,
        run_id.as_deref(),
        event_run_id,
        "existing_binding",
        None,
        binding_status,
        binding_elapsed,
        started.elapsed(),
        0,
    );
    emit_search_progress(
        &app,
        state,
        run_id.as_deref(),
        event_run_id,
        "local_search",
        None,
        "running",
        Duration::ZERO,
        started.elapsed(),
        0,
    );
    emit_search_progress(
        &app,
        state,
        run_id.as_deref(),
        event_run_id,
        "online_search",
        None,
        "running",
        Duration::ZERO,
        started.elapsed(),
        0,
    );
    let result = perform_lyrics_search_inner(
        &app,
        run_id.as_deref(),
        event_run_id,
        state,
        &input,
        intent,
        auto_apply_threshold,
        started,
    )
    .await;
    let search_duration_ms = duration_ms(started.elapsed());
    match &result {
        Ok(response) => {
            let selected = response
                .auto_apply_candidate
                .as_ref()
                .map(|candidate| (candidate.provider_id.as_str(), candidate.id.as_str()));
            if let Some(run_id) = run_id.as_deref() {
                if let Err(error) = state.storage.finish_lyrics_search_run(
                    run_id,
                    if response.error.is_some() {
                        "failed"
                    } else {
                        "completed"
                    },
                    response.error.as_deref(),
                    &response.provider_statuses,
                    &response.provider_elapsed_ms,
                    &response.results,
                    &input,
                    selected,
                    response
                        .auto_apply
                        .then_some("automatic_score_and_quality_gate"),
                    duration_ms(started.elapsed()),
                ) {
                    log::warn!("保存歌词搜索摘要失败：{error}");
                }
            }
            emit_search_progress(
                &app,
                state,
                run_id.as_deref(),
                event_run_id,
                if response.auto_apply {
                    "auto_apply"
                } else {
                    "await_selection"
                },
                selected.map(|(provider_id, _)| provider_id.to_owned()),
                if response.error.is_some() {
                    "failed"
                } else {
                    "completed"
                },
                Duration::from_millis(response.decision_elapsed_ms),
                started.elapsed(),
                response.results.len(),
            );
            // 统计实际执行的搜索；并发复用与已完成结果复用不会再次进入这里。
            state
                .telemetry
                .track_search(intent, response, search_duration_ms);
        }
        Err(error) => {
            let final_started = Instant::now();
            if let Some(run_id) = run_id.as_deref() {
                if let Err(storage_error) = state.storage.fail_lyrics_search_run(
                    run_id,
                    "search_failed",
                    duration_ms(started.elapsed()),
                ) {
                    log::warn!("保存失败的歌词搜索摘要失败：{storage_error}");
                }
            }
            emit_search_progress(
                &app,
                state,
                run_id.as_deref(),
                event_run_id,
                "await_selection",
                None,
                "failed",
                final_started.elapsed(),
                started.elapsed(),
                0,
            );
            log::debug!("歌词搜索失败：{error}");
        }
    }
    result
}

async fn perform_lyrics_search_inner(
    app: &tauri::AppHandle,
    stored_run_id: Option<&str>,
    event_run_id: &str,
    state: &AppState,
    input: &LyricsSearchInput,
    intent: LyricsSearchIntent,
    auto_apply_threshold: u8,
    search_started: Instant,
) -> Result<SearchResponse, String> {
    let (local_result, provider_result) = tokio::join!(
        async {
            let started = Instant::now();
            let result = search_local_lyrics(state, input).await;
            let elapsed = started.elapsed();
            emit_search_progress(
                app,
                state,
                stored_run_id,
                event_run_id,
                "local_search",
                None,
                if result.is_ok() {
                    "completed"
                } else {
                    "failed"
                },
                elapsed,
                search_started.elapsed(),
                result.as_ref().map(|value| value.len()).unwrap_or(0),
            );
            result
        },
        async {
            let started = Instant::now();
            let result = state
                .providers
                .search_with_cache(
                    &state.http,
                    input,
                    matches!(intent, LyricsSearchIntent::Manual),
                )
                .await;
            let elapsed = started.elapsed();
            emit_search_progress(
                app,
                state,
                stored_run_id,
                event_run_id,
                "online_search",
                None,
                if result.is_ok() {
                    "completed"
                } else {
                    "failed"
                },
                elapsed,
                search_started.elapsed(),
                result
                    .as_ref()
                    .map(|value| value.results.len())
                    .unwrap_or(0),
            );
            result
        },
    );
    let local_error = local_result.as_ref().err().cloned();
    let (mut local_results, local_auto_apply_threshold) = match local_result {
        Ok(value) => (value, auto_apply_threshold),
        Err(error) => {
            log::debug!("本地歌词搜索失败，继续在线搜索：{error}");
            (Vec::new(), auto_apply_threshold)
        }
    };
    let provider_error = provider_result.as_ref().err().cloned();
    let (
        mut outcome,
        fallback_statuses,
        fallback_mode,
        fallback_order,
        fallback_prefer,
        fallback_tolerance,
        fallback_auto_apply_threshold,
    ) = match provider_result {
        Ok(outcome) => (
            Some(outcome),
            Vec::<ProviderStatus>::new(),
            ProviderOrderMode::Smart,
            Vec::<String>::new(),
            true,
            DEFAULT_CAPABILITY_PREFERENCE_TOLERANCE,
            local_auto_apply_threshold,
        ),
        Err(_error) => {
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
                view.settings.auto_apply_threshold,
            )
        }
    };
    let secondary_display = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .secondary_display;
    let (
        mode,
        provider_order,
        prefer_capabilities,
        capability_preference_tolerance,
        auto_apply_threshold,
    ) = outcome
        .as_ref()
        .map(|outcome| {
            (
                outcome.mode,
                outcome.provider_order.clone(),
                outcome.prefer_capabilities,
                outcome.capability_preference_tolerance,
                outcome.auto_apply_threshold,
            )
        })
        .unwrap_or((
            fallback_mode,
            fallback_order,
            fallback_prefer,
            fallback_tolerance,
            fallback_auto_apply_threshold,
        ));
    let online_results = outcome
        .as_mut()
        .map(|outcome| std::mem::take(&mut outcome.results))
        .unwrap_or_default();
    let auto_decision_stable = outcome
        .as_ref()
        .map(|outcome| outcome.auto_decision_stable)
        .unwrap_or(true);
    let online_count = online_results.len();
    let compare_started = Instant::now();
    emit_search_progress(
        app,
        state,
        stored_run_id,
        event_run_id,
        "compare_versions",
        None,
        "running",
        Duration::ZERO,
        search_started.elapsed(),
        0,
    );
    let mut candidates = local_results
        .drain(..)
        .enumerate()
        .map(|(index, result)| analyze_candidate(result, index, true))
        .collect::<Vec<_>>();
    let local_count = candidates.len();
    candidates.extend(
        online_results
            .into_iter()
            .enumerate()
            .map(|(index, result)| analyze_candidate(result, local_count + index, false)),
    );
    let analyzed_count = candidates.len();
    for candidate in &mut candidates {
        if has_identity_conflict(input, &candidate.result) {
            candidate.quality.auto_applicable = false;
        }
    }
    deduplicate_analyzed_candidates(
        &mut candidates,
        mode,
        &provider_order,
        prefer_capabilities,
        capability_preference_tolerance,
        secondary_display,
        auto_apply_threshold,
    );
    let deduplicated_count = candidates.len();
    sort_analyzed_candidates(
        &mut candidates,
        mode,
        &provider_order,
        prefer_capabilities,
        capability_preference_tolerance,
        secondary_display,
        auto_apply_threshold,
    );
    let decision_started = Instant::now();
    let auto_apply_candidate = auto_decision_stable
        .then(|| {
            auto_apply_analyzed(&candidates, auto_apply_threshold, input).map(|candidate| {
                LyricsCandidateRef {
                    provider_id: candidate.result.provider_id.clone(),
                    id: candidate.result.id.clone(),
                }
            })
        })
        .flatten();
    let decision_elapsed = decision_started.elapsed();
    let auto_apply = auto_apply_candidate.is_some();
    emit_search_progress(
        app,
        state,
        stored_run_id,
        event_run_id,
        "compare_versions",
        None,
        "completed",
        compare_started.elapsed(),
        search_started.elapsed(),
        candidates.len(),
    );
    let sorted_count = candidates.len();
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
        "lyrics.rank search title={:?} artist={:?} intent={intent:?} mode={mode:?} prefer_capabilities={prefer_capabilities} capability_tolerance_percent={capability_preference_tolerance} score_band={score_band:.4} secondary_display={secondary_display:?} auto_apply_threshold_percent={auto_apply_threshold} auto_decision_stable={auto_decision_stable} local_candidates={local_count} online_candidates={online_count} analyzed={analyzed_count} deduplicated={deduplicated_count} sorted={sorted_count} auto_apply={auto_apply} provider_order={provider_order:?}",
        input.title,
        input.artist,
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
    let search_error = if results.is_empty() {
        outcome
            .as_ref()
            .and_then(|outcome| outcome.error.clone())
            .or(provider_error)
            .or(local_error)
    } else {
        None
    };
    let provider_elapsed_ms = outcome
        .as_ref()
        .map(|outcome| outcome.provider_elapsed_ms.clone())
        .unwrap_or_default();
    Ok(SearchResponse {
        auto_apply,
        auto_apply_candidate,
        results,
        provider_statuses: outcome
            .as_ref()
            .map(|outcome| outcome.statuses.clone())
            .unwrap_or(fallback_statuses),
        error: search_error,
        provider_elapsed_ms,
        decision_elapsed_ms: duration_ms(decision_elapsed),
    })
}

fn emit_search_progress(
    app: &tauri::AppHandle,
    state: &AppState,
    stored_run_id: Option<&str>,
    event_run_id: &str,
    stage: &str,
    provider_id: Option<String>,
    status: &str,
    elapsed: std::time::Duration,
    total_elapsed: std::time::Duration,
    candidate_count: usize,
) {
    let stage_elapsed_ms = duration_ms(elapsed);
    let total_elapsed_ms = duration_ms(total_elapsed);
    if let Some(run_id) = stored_run_id {
        if let Err(error) = state.storage.record_lyrics_search_stage(
            run_id,
            stage,
            provider_id.as_deref(),
            status,
            stage_elapsed_ms,
            candidate_count,
        ) {
            log::debug!("保存歌词搜索阶段失败：{error}");
        }
    }
    let _ = app.emit(
        "lyrics-search-progress",
        LyricsSearchProgress {
            run_id: event_run_id.to_owned(),
            stage: stage.to_owned(),
            provider_id,
            status: status.to_owned(),
            elapsed_ms: stage_elapsed_ms,
            total_elapsed_ms,
            candidate_count,
        },
    );
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

async fn search_local_lyrics(
    state: &AppState,
    input: &LyricsSearchInput,
) -> Result<Vec<LyricsSearchResult>, String> {
    let storage = state.storage.clone();
    let input = (*input).clone();
    let results = tauri::async_runtime::spawn_blocking(move || storage.search_local_lyrics(&input))
        .await
        .map_err(|error| format!("本地歌词搜索任务失败：{error}"))??;
    Ok(results)
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
        album: None,
        duration_ms: None,
        source: &result.source,
        raw: &result.lyrics,
        provider_id: Some(&result.provider_id),
        provider_item_id: Some(&result.id),
        confidence: Some((result.score * 100.0).round().clamp(0.0, 100.0) as u8),
        kind: SaveKind::Automatic,
    };
    if result.provider_id == LOCAL_PROVIDER_ID {
        state.storage.associate_local_lyrics(request)
    } else {
        state.storage.save(request)
    }
}

pub(crate) async fn search_lyrics_for_session(
    app: tauri::AppHandle,
    state: &AppState,
    track_key: &str,
    input: LyricsSearchInput,
    intent: LyricsSearchIntent,
) -> Result<SearchResponse, String> {
    if input.title.trim().is_empty() || input.artist.trim().is_empty() {
        return Err("搜索歌词需要歌曲名和歌手".into());
    }

    let request_key = LyricsSearchRequestKey::new(&input);
    let (activation, request_id, flight, should_debounce) = {
        let mut session = state
            .lyrics_search_session
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if session.track_key.as_deref() != Some(track_key) {
            return Err("当前歌曲已发生变化".into());
        }
        let same_request = session.request_key.as_ref() == Some(&request_key);
        // 自动搜索只复用相同条件；快速窗口刷新则复用当前歌曲最近一次搜索，
        // 包括手动条件、空结果和失败，避免每次开窗重复请求歌词源。
        let reuse_current = matches!(intent, LyricsSearchIntent::Refresh)
            || (matches!(intent, LyricsSearchIntent::Automatic) && same_request);
        if reuse_current {
            if let Some(completed) = &session.completed {
                return completed.clone();
            }
        }
        if reuse_current {
            if let Some(flight) = &session.in_flight {
                (
                    session.activation,
                    session.request_id,
                    flight.clone(),
                    intent.uses_debounce(),
                )
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

    let search_app = app.clone();
    let result = flight
        .get_or_init(|| perform_lyrics_search(search_app, state, track_key, &input, intent))
        .await
        .clone();
    let mut session = state
        .lyrics_search_session
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if session.activation != activation || session.request_id != request_id {
        return Err(LYRICS_SEARCH_INVALIDATED.into());
    }
    // 只有最初持有本次搜索 flight 的调用方负责广播，避免同一轮搜索的并发复用方重复发送事件。
    let should_publish_provider_statuses = session
        .in_flight
        .as_ref()
        .is_some_and(|current| Arc::ptr_eq(current, &flight));
    session.completed = Some(result.clone());
    session.in_flight = None;
    drop(session);
    if should_publish_provider_statuses {
        if let Ok(response) = &result {
            let _ = app.emit(PROVIDER_STATUSES_EVENT, response.provider_statuses.clone());
        }
    }
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
