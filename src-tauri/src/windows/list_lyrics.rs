use tauri::{Manager, WebviewWindowBuilder};

use super::platform::{apply_lyrics_window_space_behavior, refresh_overlay_mouse_tracking};
use crate::{sync_list_unlock_handle, AppState, UiLanguage};

const LIST_LYRICS_DEFAULT_WIDTH: f64 = 520.0;
const LIST_LYRICS_DEFAULT_HEIGHT: f64 = 720.0;

pub(super) fn create_list_lyrics_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("lyrics-list").is_some() {
        return Ok(());
    }
    let always_on_top = app
        .state::<AppState>()
        .config
        .snapshot()
        .lyrics
        .displays
        .list_window
        .always_on_top;
    let locked = app
        .state::<AppState>()
        .config
        .snapshot()
        .lyrics
        .displays
        .list_window
        .locked;
    let window = WebviewWindowBuilder::new(
        app,
        "lyrics-list",
        crate::webview_url(app, "index.html?view=lyrics-list"),
    )
    .title(UiLanguage::ZhCn.native_labels().list_title)
    .inner_size(LIST_LYRICS_DEFAULT_WIDTH, LIST_LYRICS_DEFAULT_HEIGHT)
    .min_inner_size(360.0, 480.0)
    .transparent(true)
    .accept_first_mouse(true)
    .decorations(false)
    .shadow(false)
    .resizable(true)
    .maximizable(false)
    .minimizable(true)
    .always_on_top(always_on_top)
    .visible(false)
    .center()
    .build()?;
    let enabled = app
        .state::<AppState>()
        .config
        .snapshot()
        .app
        .lyrics_windows_show_on_all_spaces;
    apply_lyrics_window_space_behavior(&window, enabled)?;
    apply_list_lyrics_window_lock(app, locked).map_err(std::io::Error::other)?;
    Ok(())
}

pub(crate) fn apply_list_lyrics_window_lock(
    app: &tauri::AppHandle,
    locked: bool,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("lyrics-list") {
        window
            .set_ignore_cursor_events(locked)
            .map_err(|error| error.to_string())?;
        window
            .set_focusable(!locked)
            .map_err(|error| error.to_string())?;
        window
            .set_resizable(!locked)
            .map_err(|error| error.to_string())?;
        if !locked {
            refresh_overlay_mouse_tracking(&window);
        }
    }
    sync_list_unlock_handle(app);
    Ok(())
}

pub(crate) fn reset_list_lyrics_window_size(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("lyrics-list")
        .ok_or_else(|| "歌词窗口不存在".to_string())?;
    window
        .set_size(tauri::LogicalSize::new(
            LIST_LYRICS_DEFAULT_WIDTH,
            LIST_LYRICS_DEFAULT_HEIGHT,
        ))
        .map_err(|error| error.to_string())
}
