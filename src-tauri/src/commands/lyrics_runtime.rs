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
    let (local_result, provider_result) = tokio::join!(
        search_local_lyrics(state, input),
        state.providers.search(&state.http, input),
    );
    let (mut local_results, auto_apply_threshold) = local_result?;
    let mut outcome = match provider_result {
        Ok(outcome) => Some(outcome),
        Err(_error) if !local_results.is_empty() => None,
        Err(error) => return Err(error),
    };
    let secondary_display = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .secondary_display;
    if outcome
        .as_ref()
        .is_some_and(|outcome| outcome.prefer_capabilities)
    {
        prefer_candidate_capabilities(&mut local_results, secondary_display);
        if let Some(outcome) = &mut outcome {
            prefer_candidate_capabilities(&mut outcome.results, secondary_display);
        }
    }
    let had_local_results = !local_results.is_empty();
    let prefer_local = local_results.first().is_some_and(|result| {
        result.provider_id == LOCAL_PROVIDER_ID
            && can_auto_apply_local(&local_results, auto_apply_threshold)
    });
    let mut seen_lyrics = local_results
        .iter()
        .map(|result| lyric_content_key(&result.lyrics))
        .collect::<std::collections::HashSet<_>>();
    let mut online_results = outcome
        .as_mut()
        .map(|outcome| std::mem::take(&mut outcome.results))
        .unwrap_or_default();
    online_results.retain(|result| seen_lyrics.insert(lyric_content_key(&result.lyrics)));
    local_results.append(&mut online_results);

    let prefer_capabilities = outcome
        .as_ref()
        .is_some_and(|outcome| outcome.prefer_capabilities);
    let recommended_index = if prefer_local {
        Some(0)
    } else {
        best_result_index(&local_results, prefer_capabilities, secondary_display)
    };
    if let Some(recommended_index) = recommended_index {
        let recommended = local_results.remove(recommended_index);
        local_results.insert(0, recommended);
    }
    let auto_apply = local_results.first().is_some_and(|result| {
        if result.provider_id == LOCAL_PROVIDER_ID {
            can_auto_apply_local(&local_results, auto_apply_threshold)
        } else {
            outcome
                .as_ref()
                .is_some_and(|outcome| can_auto_apply(&local_results, outcome.auto_apply_threshold))
        }
    });
    Ok(SearchResponse {
        auto_apply,
        results: local_results,
        provider_statuses: outcome
            .as_ref()
            .map(|outcome| outcome.statuses.clone())
            .unwrap_or_default(),
        error: (!had_local_results)
            .then(|| outcome.and_then(|outcome| outcome.error))
            .flatten(),
    })
}

async fn search_local_lyrics(
    state: &AppState,
    input: &LyricsSearchInput,
) -> Result<(Vec<LyricsSearchResult>, u8), String> {
    let (input, threshold) = state.providers.local_search_context(input)?;
    let storage = state.storage.clone();
    let results = tauri::async_runtime::spawn_blocking(move || storage.search_local_lyrics(&input))
        .await
        .map_err(|error| format!("本地歌词搜索任务失败：{error}"))??;
    Ok((results, threshold))
}

fn can_auto_apply_local(results: &[LyricsSearchResult], threshold_percent: u8) -> bool {
    let Some(first) = results.first() else {
        return false;
    };
    if !can_auto_apply(std::slice::from_ref(first), threshold_percent) {
        return false;
    }
    results
        .iter()
        .skip(1)
        .filter(|result| result.provider_id == LOCAL_PROVIDER_ID)
        .max_by(|left, right| left.score.total_cmp(&right.score))
        .is_none_or(|second| first.score - second.score >= 0.05)
}

fn lyric_content_key(lyrics: &str) -> String {
    lyrics
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
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
    let force = intent.is_manual();
    let (activation, request_id, flight) = {
        let mut session = state
            .lyrics_search_session
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if session.track_key.as_deref() != Some(track_key) {
            return Err("当前歌曲已发生变化".into());
        }
        let same_request = session.request_key.as_ref() == Some(&request_key);
        if !force && same_request {
            if let Some(completed) = &session.completed {
                return completed.clone();
            }
        }
        if same_request {
            if let Some(flight) = &session.in_flight {
                (session.activation, session.request_id, flight.clone())
            } else {
                session.request_id = session.request_id.wrapping_add(1);
                if force {
                    session.completed = None;
                }
                let flight = Arc::new(LyricsSearchFlight::new());
                session.in_flight = Some(flight.clone());
                (session.activation, session.request_id, flight)
            }
        } else {
            session.request_id = session.request_id.wrapping_add(1);
            session.request_key = Some(request_key.clone());
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
                        let request = SaveRequest {
                            track_key: &track_key,
                            title: &title,
                            artist: &artist,
                            source: &result.source,
                            raw: &result.lyrics,
                            provider_id: Some(&result.provider_id),
                            provider_item_id: Some(&result.id),
                            kind: SaveKind::Automatic,
                        };
                        if result.provider_id == LOCAL_PROVIDER_ID {
                            state.storage.associate_local_lyrics(request).ok()
                        } else {
                            state.storage.save(request).ok()
                        }
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
