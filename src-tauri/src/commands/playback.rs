#[tauri::command]
pub fn get_playback_artwork(
    artwork_id: String,
    state: State<'_, AppState>,
) -> Result<Option<PlaybackArtwork>, String> {
    state.system_media.artwork(&artwork_id)
}

#[tauri::command]
pub fn start_playback_spectrum(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) -> PlaybackSpectrumState {
    let snapshot = state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    state.spectrum.subscribe(&app, window.label(), &snapshot)
}

#[tauri::command]
pub fn stop_playback_spectrum(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
) {
    state.spectrum.unsubscribe(&app, window.label());
}

#[tauri::command]
pub fn get_playback_spectrum_state(state: State<'_, AppState>) -> PlaybackSpectrumState {
    state.spectrum.state()
}

#[tauri::command]
pub fn get_player_selection(state: State<'_, AppState>) -> PlayerSelection {
    *state
        .selection
        .read()
        .unwrap_or_else(|error| error.into_inner())
}

pub fn update_player_selection(
    app: &tauri::AppHandle,
    selection: PlayerSelection,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let saved = state
        .config
        .update(|config| config.app.player_selection = selection)?;
    *state
        .selection
        .write()
        .unwrap_or_else(|error| error.into_inner()) = selection;
    *state
        .auto_player
        .write()
        .unwrap_or_else(|error| error.into_inner()) = None;
    app.emit("player://selection", selection)
        .map_err(|error| error.to_string())?;
    app.emit("config://changed", &saved)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_player_selection(
    app: tauri::AppHandle,
    selection: PlayerSelection,
) -> Result<(), String> {
    update_player_selection(&app, selection)
}

#[tauri::command]
pub async fn search_lyrics(
    track_key: String,
    input: LyricsSearchInput,
    intent: LyricsSearchIntent,
    state: State<'_, AppState>,
) -> Result<SearchResponse, String> {
    search_lyrics_for_session(&state, &track_key, input, intent).await
}

fn candidate_capability_rank(
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
