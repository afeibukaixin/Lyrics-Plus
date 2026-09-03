use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

use super::platform::{
    apply_joining_other_apps_fullscreen, apply_lyrics_window_space_behavior,
    refresh_overlay_mouse_tracking,
};
use crate::{
    sync_overlay_vibrancy, AppState, OverlayOrientation, OverlayStyleSettings, UiLanguage,
};

pub(crate) fn initial_overlay_dimensions(style: &OverlayStyleSettings) -> (f64, f64) {
    match style.orientation {
        OverlayOrientation::Horizontal => (style.horizontal_max_width.unwrap_or(760.0), 156.0),
        OverlayOrientation::Vertical => (190.0, style.vertical_max_height.unwrap_or(620.0)),
    }
}

const MIN_VERTICAL_HOST_WIDTH: f64 = 49.0;

pub(crate) fn create_overlay(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("lyrics-overlay").is_some() {
        return Ok(());
    }

    let style = app
        .try_state::<AppState>()
        .map(|state| {
            state
                .overlay_style
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
        })
        .unwrap_or_default();
    let (initial_width, initial_height) = initial_overlay_dimensions(&style);
    let minimum_width = match style.orientation {
        OverlayOrientation::Vertical => MIN_VERTICAL_HOST_WIDTH,
        OverlayOrientation::Horizontal => 190.0,
    };

    let window = WebviewWindowBuilder::new(
        app,
        "lyrics-overlay",
        WebviewUrl::App("index.html?view=overlay".into()),
    )
    .title(UiLanguage::ZhCn.native_labels().overlay_title)
    .inner_size(initial_width, initial_height)
    .min_inner_size(minimum_width, 76.0)
    .transparent(true)
    .accept_first_mouse(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .build()?;

    apply_joining_other_apps_fullscreen(&window)?;
    let enabled = app
        .state::<AppState>()
        .config
        .snapshot()
        .app
        .lyrics_windows_show_on_all_spaces;
    apply_lyrics_window_space_behavior(&window, enabled)?;
    refresh_overlay_mouse_tracking(&window);
    sync_overlay_vibrancy(&window, &style);

    Ok(())
}
