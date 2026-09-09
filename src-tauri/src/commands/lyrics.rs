#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetachPlatformTrackInput {
    pub track_key: String,
    pub platform: String,
    pub lyrics_mode: DetachLyricsMode,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DetachLyricsMode {
    InheritCurrent,
    Unbound,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssociateSongCandidateInput {
    pub track_key: String,
    pub platform: String,
    pub candidate_recording_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetArtistAliasConfirmationInput {
    pub track_key: String,
    pub artist_id: i64,
    pub alias: String,
    pub confirmed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindLibraryLyricInput {
    pub recording_id: i64,
    pub asset_id: i64,
    pub replace_default: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLyricRelationInput {
    pub recording_id: i64,
    pub asset_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryArtistNameInput {
    pub artist_id: i64,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryArtistAliasInput {
    pub artist_id: i64,
    pub alias: String,
    pub confirmed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeLibraryLyricsInput {
    pub keeper_asset_id: i64,
    pub redundant_asset_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeLibrarySongInput {
    pub recording_id: i64,
    pub candidate_recording_id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitLibrarySongInput {
    pub recording_id: i64,
    pub observation_id: i64,
    pub inherit_current_lyrics: bool,
}

#[cfg(test)]
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
) -> Result<LyricsLoadResponse, String> {
    let config = state.config.snapshot();
    match state.storage.load_with_status(&track_key)? {
        crate::storage::LyricsLoadResult::Ready(document) => Ok(LyricsLoadResponse {
            status: LyricsLoadStatus::Ready,
            document: Some(document.converted_for_output(
                config.lyrics.chinese_conversion,
                config.lyrics.repair_simplified_japanese,
            )),
            error: None,
        }),
        crate::storage::LyricsLoadResult::Missing => Ok(LyricsLoadResponse {
            status: LyricsLoadStatus::Missing,
            document: None,
            error: None,
        }),
        crate::storage::LyricsLoadResult::Invalid(error) => {
            log::warn!("歌词关联内容无效，准备解除关联：{error}");
            state.storage.remove(&track_key)?;
            Ok(LyricsLoadResponse {
                status: LyricsLoadStatus::Missing,
                document: None,
                error: None,
            })
        }
    }
}

/// 仅解析候选歌词供快速切换窗口预览，不写入歌词文件或关联记录。
#[tauri::command]
pub fn parse_lyrics_preview(
    source: String,
    lyrics: String,
    state: State<'_, AppState>,
) -> Result<LyricsDocument, String> {
    let config = state.config.snapshot();
    let document = crate::lyrics::parse_lrc_with_options(&lyrics, source, false)?;
    Ok(document.converted_for_output(
        config.lyrics.chinese_conversion,
        config.lyrics.repair_simplified_japanese,
    ))
}

#[tauri::command]
pub fn get_completed_lyrics_search(
    track_key: String,
    state: State<'_, AppState>,
) -> Option<SearchResponse> {
    completed_lyrics_search(&state, &track_key)
}

#[tauri::command]
pub fn get_lyrics_search_trace(
    track_key: String,
    state: State<'_, AppState>,
) -> Result<Option<LyricsSearchTrace>, String> {
    state.storage.latest_lyrics_search_trace(&track_key)
}

#[tauri::command]
pub fn get_current_lyrics_context(
    track_key: String,
    state: State<'_, AppState>,
) -> Result<Option<crate::storage::LyricsContext>, String> {
    state.storage.current_lyrics_context(&track_key)
}

#[tauri::command]
pub fn list_library_songs(
    query: Option<String>,
    page: Option<u64>,
    page_size: Option<u64>,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibraryPage<crate::storage::LibrarySongSummary>, String> {
    let started = std::time::Instant::now();
    let result = state.storage.list_library_songs(
        query.as_deref().unwrap_or_default(),
        page.unwrap_or(1),
        page_size.unwrap_or(20),
    );
    if let Ok(page) = &result {
        log::debug!(
            "资料库歌曲列表：rows={} total={} elapsed_ms={}",
            page.items.len(),
            page.total,
            started.elapsed().as_millis()
        );
    }
    result
}

#[tauri::command]
pub fn get_library_index_status(
    state: State<'_, AppState>,
) -> Result<crate::storage::LibraryIndexStatus, String> {
    state.storage.library_index_status()
}

#[tauri::command]
pub fn get_library_song(
    recording_id: i64,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibrarySongDetail, String> {
    state.storage.library_song_detail(recording_id)
}

#[tauri::command]
pub fn get_library_song_candidates(
    recording_id: i64,
    state: State<'_, AppState>,
) -> Result<Vec<SongAssociationCandidate>, String> {
    let detail = state.storage.library_song_detail(recording_id)?;
    let observation = detail
        .recording
        .observations
        .first()
        .ok_or_else(|| "歌曲没有可用于匹配的平台观察".to_string())?;
    let settings = state.providers.settings_view().settings;
    state.storage.song_association_candidates(
        &observation.platform,
        &observation.track_key,
        &settings,
    )
}

#[tauri::command]
pub fn analyze_library_song_similarity(
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::SongSimilarityPair>, String> {
    let settings = state.providers.settings_view().settings;
    state.storage.analyze_library_song_similarity(&settings)
}

#[tauri::command]
pub fn list_library_song_similarity(
    app: tauri::AppHandle,
    page: Option<u64>,
    page_size: Option<u64>,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibraryPage<crate::storage::SongSimilarityPair>, String> {
    let settings = state.providers.settings_view().settings;
    let result = state.storage.list_library_song_similarity(
        page.unwrap_or(1),
        page_size.unwrap_or(20),
        &settings,
    );
    if let Ok(status) = state.storage.library_index_status() {
        let _ = app.emit("lyrics://library-index-changed", status);
    }
    result
}

#[tauri::command]
pub fn dismiss_library_song_similarity(
    left_recording_id: i64,
    right_recording_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .storage
        .dismiss_library_song_similarity(left_recording_id, right_recording_id)
}

#[tauri::command]
pub fn merge_library_song(
    app: tauri::AppHandle,
    input: MergeLibrarySongInput,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibrarySongDetail, String> {
    let detail = state.storage.library_song_detail(input.recording_id)?;
    let observation = detail
        .recording
        .observations
        .first()
        .ok_or_else(|| "歌曲没有可用于合并的平台观察".to_string())?;
    let settings = state.providers.settings_view().settings;
    state.storage.associate_song_candidate(
        &observation.platform,
        &observation.track_key,
        input.candidate_recording_id,
        &settings,
    )?;
    publish_song_management_change(&app, &state, &observation.track_key);
    state.storage.library_song_detail(input.recording_id)
}

#[tauri::command]
pub fn split_library_song(
    app: tauri::AppHandle,
    input: SplitLibrarySongInput,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibrarySongDetail, String> {
    let detail = state.storage.library_song_detail(input.recording_id)?;
    let observation = detail
        .recording
        .observations
        .iter()
        .find(|item| item.observation_id == input.observation_id)
        .ok_or_else(|| "平台曲目不属于指定歌曲".to_string())?;
    state.storage.detach_platform_track(
        &observation.platform,
        &observation.track_key,
        input.inherit_current_lyrics,
    )?;
    publish_song_management_change(&app, &state, &observation.track_key);
    state.storage.library_song_detail(input.recording_id)
}

#[tauri::command]
pub fn list_library_lyrics(
    query: Option<String>,
    status: Option<String>,
    source_kind: Option<String>,
    page: Option<u64>,
    page_size: Option<u64>,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibraryLyricPage, String> {
    let started = std::time::Instant::now();
    let result = state.storage.list_library_lyrics(
        query.as_deref().unwrap_or_default(),
        status.as_deref(),
        source_kind.as_deref(),
        page.unwrap_or(1),
        page_size.unwrap_or(20),
    );
    if let Ok(page) = &result {
        log::debug!(
            "资料库歌词列表：rows={} total={} elapsed_ms={}",
            page.items.len(),
            page.total,
            started.elapsed().as_millis()
        );
    }
    result
}

#[tauri::command]
pub fn get_library_lyric(
    asset_id: i64,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibraryLyricDetail, String> {
    state.storage.library_lyric_detail(asset_id)
}

#[tauri::command]
pub fn bind_library_lyric(
    input: BindLibraryLyricInput,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibrarySongDetail, String> {
    state
        .storage
        .bind_library_lyric(input.recording_id, input.asset_id, input.replace_default)?;
    state.storage.library_song_detail(input.recording_id)
}

#[tauri::command]
pub fn unbind_library_lyric(
    input: LibraryLyricRelationInput,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibrarySongDetail, String> {
    state
        .storage
        .unbind_library_lyric(input.recording_id, input.asset_id)?;
    state.storage.library_song_detail(input.recording_id)
}

#[tauri::command]
pub fn list_library_artists(
    query: Option<String>,
    page: Option<u64>,
    page_size: Option<u64>,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibraryPage<crate::storage::LibraryArtistSummary>, String> {
    let started = std::time::Instant::now();
    let result = state.storage.list_library_artists(
        query.as_deref().unwrap_or_default(),
        page.unwrap_or(1),
        page_size.unwrap_or(20),
    );
    if let Ok(page) = &result {
        log::debug!(
            "资料库歌手列表：rows={} total={} elapsed_ms={}",
            page.items.len(),
            page.total,
            started.elapsed().as_millis()
        );
    }
    result
}

#[tauri::command]
pub fn get_library_artist(
    artist_id: i64,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibraryArtistDetail, String> {
    state.storage.library_artist_detail(artist_id)
}

#[tauri::command]
pub fn update_library_artist_name(
    input: LibraryArtistNameInput,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibraryArtistDetail, String> {
    state
        .storage
        .update_library_artist_name(input.artist_id, &input.name)
}

#[tauri::command]
pub fn set_library_artist_alias(
    input: LibraryArtistAliasInput,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibraryArtistDetail, String> {
    state
        .storage
        .set_library_artist_alias(input.artist_id, &input.alias, input.confirmed)
}

#[tauri::command]
pub fn analyze_library_lyric_similarity(
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::LyricSimilarityGroup>, String> {
    state.storage.analyze_library_lyric_similarity()
}

#[tauri::command]
pub fn list_library_lyric_similarity(
    app: tauri::AppHandle,
    page: Option<u64>,
    page_size: Option<u64>,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibraryLyricSimilarityPage, String> {
    let result = state
        .storage
        .list_library_lyric_similarity(page.unwrap_or(1), page_size.unwrap_or(20));
    if let Ok(status) = state.storage.library_index_status() {
        let _ = app.emit("lyrics://library-index-changed", status);
    }
    result
}

#[tauri::command]
pub fn dismiss_library_lyric_similarity(
    asset_ids: Vec<i64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.storage.dismiss_library_lyric_similarity(&asset_ids)
}

#[tauri::command]
pub fn merge_library_lyrics(
    input: MergeLibraryLyricsInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .storage
        .merge_library_lyrics(input.keeper_asset_id, &input.redundant_asset_ids)
}

#[tauri::command]
pub fn preview_unbound_lyrics_cleanup(
    page: u64,
    page_size: u64,
    state: State<'_, AppState>,
) -> Result<crate::storage::UnboundCleanupPreview, String> {
    state
        .storage
        .preview_unbound_lyrics_cleanup(page, page_size)
}

#[tauri::command]
pub fn cleanup_unbound_lyrics(
    selection_mode: String,
    asset_ids: Vec<i64>,
    excluded_asset_ids: Vec<i64>,
    revision: String,
    state: State<'_, AppState>,
) -> Result<crate::storage::UnboundCleanupResult, String> {
    state.storage.cleanup_unbound_lyrics(
        &selection_mode,
        &asset_ids,
        &excluded_asset_ids,
        &revision,
    )
}

#[tauri::command]
pub fn delete_library_lyric_source(
    asset_id: i64,
    source_id: i64,
    state: State<'_, AppState>,
) -> Result<crate::storage::LibraryLyricDetail, String> {
    state
        .storage
        .delete_library_lyric_source(asset_id, source_id)?;
    state.storage.library_lyric_detail(asset_id)
}

#[tauri::command]
pub fn set_artist_alias_confirmation(
    input: SetArtistAliasConfirmationInput,
    state: State<'_, AppState>,
) -> Result<crate::storage::LyricsContext, String> {
    state.storage.set_artist_alias_confirmation(
        &input.track_key,
        input.artist_id,
        &input.alias,
        input.confirmed,
    )?;
    state
        .storage
        .current_lyrics_context(&input.track_key)?
        .ok_or_else(|| "更新歌手别名后无法读取当前歌曲身份".to_string())
}

#[tauri::command]
pub async fn search_lyrics_v2(
    app: tauri::AppHandle,
    track_key: String,
    input: LyricsSearchInput,
    intent: LyricsSearchIntent,
    state: State<'_, AppState>,
) -> Result<SearchResponse, String> {
    search_lyrics_for_session(app, &state, &track_key, input, intent).await
}

#[tauri::command]
pub fn get_lyrics_runtime_snapshot(state: State<'_, AppState>) -> LyricsRuntimeSnapshot {
    let config = state.config.snapshot();
    let mut snapshot = state
        .lyrics_runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    snapshot.document = snapshot.document.map(|document| {
        document.converted_for_output(
            config.lyrics.chinese_conversion,
            config.lyrics.repair_simplified_japanese,
        )
    });
    snapshot
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
        album: input.album.as_deref(),
        duration_ms: input.duration_ms,
        source: &input.source,
        raw: &input.lyrics,
        provider_id: input.provider_id.as_deref(),
        provider_item_id: input.provider_item_id.as_deref(),
        confidence: None,
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
    let config = state.config.snapshot();
    Ok(document.converted_for_output(
        config.lyrics.chinese_conversion,
        config.lyrics.repair_simplified_japanese,
    ))
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
pub fn import_local_lyrics(
    app: tauri::AppHandle,
    input: SaveLyricsInput,
    state: State<'_, AppState>,
) -> Result<LyricsDocument, String> {
    save_and_emit(&app, &state, input, SaveKind::Import)
}

#[tauri::command]
pub fn select_lyrics_candidate(
    app: tauri::AppHandle,
    mut input: SaveLyricsInput,
    state: State<'_, AppState>,
) -> Result<LyricsDocument, String> {
    input.manual_selected = true;
    let track_key = input.track_key.clone();
    let provider_id = input.provider_id.clone();
    let provider_item_id = input.provider_item_id.clone();
    let document = save_and_emit(&app, &state, input, SaveKind::ManualSelection)?;
    if let (Some(provider_id), Some(provider_item_id)) =
        (provider_id.as_deref(), provider_item_id.as_deref())
    {
        state
            .storage
            .mark_latest_lyrics_search_selection(
                &track_key,
                provider_id,
                provider_item_id,
                "user_selected_default",
            )
            .ok();
    }
    Ok(document)
}

#[tauri::command]
pub fn clear_lyrics_binding(
    app: tauri::AppHandle,
    track_key: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.storage.clear_lyrics_binding(&track_key)?;
    app.emit("lyrics://changed", &track_key)
        .map_err(|error| error.to_string())?;
    set_runtime_document_if_active(&app, &track_key, None);
    Ok(())
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
pub fn set_lyrics_offset_v2(
    app: tauri::AppHandle,
    track_key: String,
    offset_ms: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.storage.set_v2_lyrics_offset(&track_key, offset_ms)?;
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

#[tauri::command]
pub fn get_song_association_candidates(
    track_key: String,
    platform: String,
    state: State<'_, AppState>,
) -> Result<Vec<SongAssociationCandidate>, String> {
    let settings = state.providers.settings_view().settings;
    state
        .storage
        .song_association_candidates(&platform, &track_key, &settings)
}

#[tauri::command]
pub fn associate_song_candidate(
    app: tauri::AppHandle,
    input: AssociateSongCandidateInput,
    state: State<'_, AppState>,
) -> Result<RecordingView, String> {
    let settings = state.providers.settings_view().settings;
    let view = state.storage.associate_song_candidate(
        &input.platform,
        &input.track_key,
        input.candidate_recording_id,
        &settings,
    )?;
    publish_song_management_change(&app, &state, &input.track_key);
    Ok(view)
}

#[tauri::command]
pub fn detach_platform_track(
    app: tauri::AppHandle,
    input: DetachPlatformTrackInput,
    state: State<'_, AppState>,
) -> Result<RecordingView, String> {
    let inherit_current_lyrics = matches!(input.lyrics_mode, DetachLyricsMode::InheritCurrent);
    let view = state.storage.detach_platform_track(
        &input.platform,
        &input.track_key,
        inherit_current_lyrics,
    )?;
    publish_song_management_change(&app, &state, &input.track_key);
    Ok(view)
}

/// 事务已提交后，通知或文件读取失败不能被报告成数据库回滚。
fn publish_song_management_change(app: &tauri::AppHandle, state: &AppState, track_key: &str) {
    match state.storage.load(track_key) {
        Ok(document) => set_runtime_document_if_active(app, track_key, document),
        Err(error) => log::warn!("歌曲关系已保存，刷新歌词失败：{error}"),
    }
    if let Err(error) = app.emit("lyrics://changed", track_key) {
        log::warn!("歌曲关系已保存，发送刷新通知失败：{error}");
    }
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

pub(crate) fn start_library_root_scan(
    app: &tauri::AppHandle,
    root_id: &str,
) -> Result<LibraryScanStatus, String> {
    let storage = app.state::<AppState>().storage.clone();
    let status = storage.begin_library_root_scan(root_id)?;
    let scan_id = status.scan_id;
    let root_id = root_id.to_owned();
    let worker_app = app.clone();
    let _ = app.emit("lyrics://library-scan-progress", &status);
    tauri::async_runtime::spawn_blocking(move || {
        let result = storage.run_library_root_scan(scan_id, &root_id, |status| {
            let _ = worker_app.emit("lyrics://library-scan-progress", status);
        });
        match result {
            Ok(true) => {
                reload_active_lyrics_runtime(&worker_app);
                let _ = worker_app.emit("lyrics://library-changed", ());
            }
            Ok(false) => {}
            Err(error) => {
                log::warn!("扫描歌词根目录失败：{error}");
                if let Some(status) = storage.fail_library_scan(scan_id, error) {
                    let _ = worker_app.emit("lyrics://library-scan-progress", status);
                }
            }
        }
    });
    Ok(status)
}

#[tauri::command]
pub fn get_library_roots(state: State<'_, AppState>) -> Result<Vec<LibraryRootView>, String> {
    state.storage.list_library_roots()
}

#[tauri::command]
pub fn list_library_roots(state: State<'_, AppState>) -> Result<Vec<LibraryRootView>, String> {
    state.storage.list_library_roots()
}

#[tauri::command]
pub fn get_provider_catalog(state: State<'_, AppState>) -> Vec<ProviderDescriptor> {
    state.providers.catalog_view()
}

#[tauri::command]
pub fn update_provider_policy(
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
pub fn add_library_root(
    app: tauri::AppHandle,
    path: String,
    display_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<LibraryRootView, String> {
    let root = state
        .storage
        .add_library_root(&path, display_name.as_deref())?;
    let _ = start_library_root_scan(&app, &root.root_id)?;
    Ok(root)
}

#[tauri::command]
pub fn set_library_root_enabled(
    app: tauri::AppHandle,
    root_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<LibraryRootView, String> {
    let root = state.storage.set_library_root_enabled(&root_id, enabled)?;
    if enabled {
        let _ = start_library_root_scan(&app, &root_id)?;
    }
    Ok(root)
}

#[tauri::command]
pub fn remove_library_root(root_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.storage.remove_library_root(&root_id)
}

#[tauri::command]
pub fn rescan_library_root(
    app: tauri::AppHandle,
    root_id: String,
) -> Result<LibraryScanStatus, String> {
    start_library_root_scan(&app, &root_id)
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
        .update(|config| config.lyrics.displays.desktop.enabled = visible)?;
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
