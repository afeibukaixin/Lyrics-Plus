use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use super::lifecycle::{
    hide_surface, prepare_surface_show, schedule_surface_destroy, set_surface_runtime_state,
    toggle_quick_lyrics_reopen_while_destroying, SurfaceRuntimeState,
};
use crate::{SurfaceReopenRequest, UiLanguage};

const QUICK_LYRICS_REFRESH_EVENT: &str = "quick-lyrics://refresh";

pub(crate) fn show_quick_lyrics_window(app: &tauri::AppHandle) -> Result<(), String> {
    if prepare_surface_show(app, "quick-lyrics", SurfaceReopenRequest::QuickLyrics) {
        return Ok(());
    }
    if let Some(window) = app.get_webview_window("quick-lyrics") {
        if let Err(error) = window.set_size(tauri::LogicalSize::new(900.0, 620.0)) {
            log::warn!("Failed to restore the quick lyrics window size: {error}");
        }
        if let Err(error) = window.set_resizable(false) {
            log::warn!("Failed to disable resizing for the quick lyrics window: {error}");
        }
        if let Err(error) = window.unminimize() {
            log::warn!("Failed to unminimize the quick lyrics window: {error}");
        }
        window.show().map_err(|error| error.to_string())?;
        set_surface_runtime_state(app, &window, SurfaceRuntimeState::Active);
        window.set_focus().map_err(|error| error.to_string())?;
        if let Err(error) = window.emit(QUICK_LYRICS_REFRESH_EVENT, ()) {
            log::debug!("Failed to request quick lyrics refresh: {error}");
        }
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        app,
        "quick-lyrics",
        WebviewUrl::App("index.html?view=quick-lyrics".into()),
    )
    .title(UiLanguage::ZhCn.native_labels().quick_title)
    .inner_size(900.0, 620.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(true)
    .center()
    .visible(false)
    .build()
    .map_err(|error| error.to_string())?;

    window.show().map_err(|error| error.to_string())?;
    set_surface_runtime_state(app, &window, SurfaceRuntimeState::Active);
    window.set_focus().map_err(|error| error.to_string())
}

pub(crate) fn toggle_quick_lyrics_window(app: &tauri::AppHandle) -> Result<(), String> {
    if toggle_quick_lyrics_reopen_while_destroying(app) {
        return Ok(());
    }
    if let Some(window) = app.get_webview_window("quick-lyrics") {
        if window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false) {
            hide_surface(app, "quick-lyrics")?;
            schedule_surface_destroy(app, "quick-lyrics");
            return Ok(());
        }
    }
    show_quick_lyrics_window(app)
}
