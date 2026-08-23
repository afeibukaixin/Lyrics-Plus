#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsResetResponse {
    pub overlay_settings: OverlaySettings,
    pub overlay_style: OverlayStyleSettings,
    pub provider_view: ProviderSettingsView,
    pub player_selection: PlayerSelection,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigExport {
    pub file_name: String,
    pub raw: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCredentialUpdate {
    pub credentials: ProviderCredentialView,
    pub provider_view: ProviderSettingsView,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverlayResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayResizeBounds {
    pub width: f64,
    pub height: f64,
}

#[tauri::command]
pub fn get_playback_snapshot(state: State<'_, AppState>) -> PlaybackSnapshot {
    state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

#[tauri::command]
pub fn control_playback(action: PlaybackAction, state: State<'_, AppState>) -> Result<(), String> {
    let snapshot = state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let selection = *state
        .selection
        .read()
        .unwrap_or_else(|error| error.into_inner());
    control_player(action, selection, &snapshot, &state.system_media)
}

#[tauri::command]
pub fn seek_playback(position_ms: u64, state: State<'_, AppState>) -> Result<(), String> {
    let snapshot = state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let selection = *state
        .selection
        .read()
        .unwrap_or_else(|error| error.into_inner());
    seek_player(position_ms, selection, &snapshot, &state.system_media)
}
