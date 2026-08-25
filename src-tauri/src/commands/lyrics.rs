fn best_result_index(
    results: &[LyricsSearchResult],
    prefer_capabilities: bool,
    secondary_display: SecondaryDisplayMode,
) -> Option<usize> {
    let max_score = results
        .iter()
        .map(|result| result.score)
        .max_by(|left, right| left.total_cmp(right))?;
    let capability_band = if prefer_capabilities { 0.04 } else { 0.0 };
    results
        .iter()
        .enumerate()
        .filter(|(_, result)| max_score - result.score <= capability_band + f64::EPSILON)
        .min_by(|(left_index, left), (right_index, right)| {
            if prefer_capabilities {
                candidate_capability_rank(left, secondary_display)
                    .cmp(&candidate_capability_rank(right, secondary_display))
                    .then_with(|| right.score.total_cmp(&left.score))
                    .then_with(|| left_index.cmp(right_index))
            } else {
                right
                    .score
                    .total_cmp(&left.score)
                    .then_with(|| left_index.cmp(right_index))
            }
        })
        .map(|(index, _)| index)
}

fn prefer_candidate_capabilities(
    results: &mut [LyricsSearchResult],
    secondary_display: SecondaryDisplayMode,
) {
    if results.len() < 2 {
        return;
    }
    let mut ranked = results.iter().cloned().enumerate().collect::<Vec<_>>();

    let mut band_start = 0;
    while band_start < ranked.len() {
        let band_score = ranked[band_start].1.score;
        let band_len = ranked[band_start..]
            .iter()
            .take_while(|(_, result)| (band_score - result.score).abs() <= 0.04 + f64::EPSILON)
            .count();
        let band_end = band_start + band_len;
        ranked[band_start..band_end].sort_by(|(left_index, left), (right_index, right)| {
            candidate_capability_rank(left, secondary_display)
                .cmp(&candidate_capability_rank(right, secondary_display))
                .then_with(|| left_index.cmp(right_index))
        });
        band_start = band_end;
    }

    for (target, (_, result)) in results.iter_mut().zip(ranked) {
        *target = result;
    }
}

#[tauri::command]
pub fn get_provider_settings(state: State<'_, AppState>) -> ProviderSettingsView {
    state.providers.settings_view()
}

#[tauri::command]
pub fn get_provider_credentials(state: State<'_, AppState>) -> ProviderCredentialView {
    state.providers.credential_view()
}

#[tauri::command]
pub fn set_musixmatch_token(
    token_type: MusixmatchTokenType,
    token: String,
    state: State<'_, AppState>,
) -> Result<ProviderCredentialUpdate, String> {
    let (credentials, provider_view) = state.providers.set_musixmatch_token(token_type, token)?;
    state
        .config
        .update(|config| config.lyrics.providers = provider_view.settings.clone())?;
    invalidate_lyrics_search_session(&state);
    Ok(ProviderCredentialUpdate {
        credentials,
        provider_view,
    })
}

#[tauri::command]
pub fn clear_musixmatch_token(
    state: State<'_, AppState>,
) -> Result<ProviderCredentialUpdate, String> {
    let (credentials, provider_view) = state.providers.clear_musixmatch_token()?;
    state
        .config
        .update(|config| config.lyrics.providers = provider_view.settings.clone())?;
    invalidate_lyrics_search_session(&state);
    Ok(ProviderCredentialUpdate {
        credentials,
        provider_view,
    })
}

#[tauri::command]
pub fn set_provider_settings(
    settings: ProviderSettings,
    state: State<'_, AppState>,
) -> Result<ProviderSettingsView, String> {
    let view = state.providers.set_settings(settings)?;
    state
        .config
        .update(|config| config.lyrics.providers = view.settings.clone())?;
    invalidate_lyrics_search_session(&state);
    Ok(view)
}

#[tauri::command]
pub async fn test_provider(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<ProviderStatus, String> {
    state
        .providers
        .test_provider(&state.http, &provider_id)
        .await
}

#[tauri::command]
pub fn get_cached_lyrics(
    track_key: String,
    state: State<'_, AppState>,
) -> Result<Option<LyricsDocument>, String> {
    state.storage.load(&track_key)
}

#[tauri::command]
pub fn get_lyrics_runtime_snapshot(state: State<'_, AppState>) -> LyricsRuntimeSnapshot {
    state
        .lyrics_runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

#[tauri::command]
pub fn get_notch_layout_metrics(state: State<'_, AppState>) -> NotchLayoutMetrics {
    state
        .notch_layout_metrics
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

#[tauri::command]
pub fn get_lyrics_monitors(app: tauri::AppHandle) -> Result<Vec<LyricsMonitor>, String> {
    let primary_id = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| crate::notch_monitor_id(&monitor));
    app.available_monitors()
        .map_err(|error| error.to_string())
        .map(|monitors| {
            monitors
                .into_iter()
                .map(|monitor| {
                    let id = crate::notch_monitor_id(&monitor);
                    let size = monitor.size();
                    LyricsMonitor {
                        is_primary: primary_id.as_deref() == Some(id.as_str()),
                        id,
                        name: monitor.name().cloned().unwrap_or_default(),
                        width: size.width,
                        height: size.height,
                    }
                })
                .collect()
        })
}

fn save_and_emit(
    app: &tauri::AppHandle,
    state: &AppState,
    input: SaveLyricsInput,
    kind: SaveKind,
) -> Result<LyricsDocument, String> {
    state.storage.ensure_track_alias(
        &input.track_key,
        &input.title,
        &input.artist,
        input.album.as_deref(),
        input.duration_ms,
    )?;
    let request = SaveRequest {
        track_key: &input.track_key,
        title: &input.title,
        artist: &input.artist,
        source: &input.source,
        raw: &input.lyrics,
        provider_id: input.provider_id.as_deref(),
        provider_item_id: input.provider_item_id.as_deref(),
        kind,
    };
    let document = if input.provider_id.as_deref() == Some(LOCAL_PROVIDER_ID) {
        state.storage.associate_local_lyrics(request)?
    } else {
        state.storage.save(request)?
    };
    app.emit("lyrics://changed", &input.track_key)
        .map_err(|error| error.to_string())?;
    set_runtime_document_if_active(app, &input.track_key, Some(document.clone()));
    Ok(document)
}

#[tauri::command]
pub fn save_lyrics(
    app: tauri::AppHandle,
    input: SaveLyricsInput,
    state: State<'_, AppState>,
) -> Result<LyricsDocument, String> {
    let kind = if input.manual_selected {
        SaveKind::ManualSelection
    } else {
        SaveKind::Automatic
    };
    save_and_emit(&app, &state, input, kind)
}

#[tauri::command]
pub fn import_lyrics(
    app: tauri::AppHandle,
    input: SaveLyricsInput,
    state: State<'_, AppState>,
) -> Result<LyricsDocument, String> {
    save_and_emit(&app, &state, input, SaveKind::Import)
}

#[tauri::command]
pub fn set_lyrics_offset(
    app: tauri::AppHandle,
    track_key: String,
    offset_ms: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.storage.set_offset(&track_key, offset_ms)?;
    app.emit("lyrics://changed", &track_key)
        .map_err(|error| error.to_string())?;
    let document = state.storage.load(&track_key)?;
    set_runtime_document_if_active(&app, &track_key, document);
    Ok(())
}

#[tauri::command]
pub fn remove_lyrics_association(
    app: tauri::AppHandle,
    track_key: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.storage.remove(&track_key)?;
    app.emit("lyrics://changed", &track_key)
        .map_err(|error| error.to_string())?;
    set_runtime_document_if_active(&app, &track_key, None);
    Ok(())
}

pub(crate) fn start_library_scan(app: &tauri::AppHandle) -> LibraryScanStatus {
    let storage = app.state::<AppState>().storage.clone();
    let status = storage.begin_library_scan();
    let scan_id = status.scan_id;
    let worker_app = app.clone();
    let _ = app.emit("lyrics://library-scan-progress", &status);
    tauri::async_runtime::spawn_blocking(move || {
        let result = storage.run_library_scan(scan_id, |status| {
            let _ = worker_app.emit("lyrics://library-scan-progress", status);
        });
        match result {
            Ok(true) => {
                reload_active_lyrics_runtime(&worker_app);
                let _ = worker_app.emit("lyrics://library-changed", ());
            }
            Ok(false) => {}
            Err(error) => {
                log::warn!("Failed to scan the lyrics library: {error}");
                if let Some(status) = storage.fail_library_scan(scan_id, error) {
                    let _ = worker_app.emit("lyrics://library-scan-progress", status);
                }
            }
        }
    });
    status
}

#[tauri::command]
pub fn get_library_scan_status(state: State<'_, AppState>) -> LibraryScanStatus {
    state.storage.library_scan_status()
}

#[tauri::command]
pub fn rescan_lyrics_library(app: tauri::AppHandle) -> LibraryScanStatus {
    start_library_scan(&app)
}

#[tauri::command]
pub fn set_lyrics_directory(
    app: tauri::AppHandle,
    path: String,
    state: State<'_, AppState>,
) -> Result<LibraryScanStatus, String> {
    state.storage.set_library_directory(&path)?;
    Ok(start_library_scan(&app))
}

#[tauri::command]
pub fn open_lyrics_directory(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    app.opener()
        .open_path(
            state.storage.library_directory().to_string_lossy(),
            None::<&str>,
        )
        .map_err(|error| format!("打开歌词目录失败：{error}"))
}

pub fn update_overlay_visible(app: &tauri::AppHandle, visible: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let config = state
        .config
        .update(|config| config.overlay.visible = visible)?;
    state
        .overlay_settings
        .write()
        .unwrap_or_else(|error| error.into_inner())
        .visible = visible;
    crate::reconcile_overlay_visibility(app)?;
    crate::sync_tray_overlay_checked(app, visible);
    app.emit("overlay://settings", get_overlay_settings_inner(&state))
        .map_err(|error| error.to_string())?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_overlay_visible(app: tauri::AppHandle, visible: bool) -> Result<(), String> {
    update_overlay_visible(&app, visible)
}

fn get_overlay_settings_inner(state: &AppState) -> OverlaySettings {
    state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}
