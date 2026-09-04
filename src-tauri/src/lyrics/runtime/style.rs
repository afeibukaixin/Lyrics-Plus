use serde::Deserialize;
use tauri::{Emitter, Manager, State};

use crate::config::AppConfig;
use crate::overlay_model::OverlayStyleSettings;
use crate::state::AppState;

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

pub(crate) fn sync_desktop_style_from_config(
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
