use tauri::{Manager, WebviewWindowBuilder};

#[cfg(not(target_os = "macos"))]
use super::list_lyrics_position::restore_list_lyrics_position;
use super::platform::{apply_list_lyrics_window_space_behavior, refresh_overlay_mouse_tracking};
use crate::{sync_list_unlock_handle, AppState};
#[cfg(not(target_os = "macos"))]
use tauri_plugin_window_state::{StateFlags, WindowExt};

const LIST_LYRICS_DEFAULT_WIDTH: f64 = 520.0;
const LIST_LYRICS_DEFAULT_HEIGHT: f64 = 720.0;
const LIST_LYRICS_MINIMUM_WIDTH: f64 = 160.0;
const LIST_LYRICS_MINIMUM_HEIGHT: f64 = 96.0;

#[cfg(target_os = "macos")]
fn hide_list_lyrics_window_controls_on_main(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSTitlebarSeparatorStyle, NSWindow, NSWindowButton};

    if MainThreadMarker::new().is_none() {
        return Err(std::io::Error::other(
            "macOS window controls must be updated on the main thread",
        )
        .into());
    }

    let ns_window = window.ns_window()?;
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };
    for button in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(control) = ns_window.standardWindowButton(button) {
            control.setHidden(true);
        }
    }
    ns_window.setTitlebarSeparatorStyle(NSTitlebarSeparatorStyle::None);
    Ok(())
}

#[cfg(target_os = "macos")]
fn hide_list_lyrics_window_controls(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2::MainThreadMarker;

    if MainThreadMarker::new().is_some() {
        return hide_list_lyrics_window_controls_on_main(window);
    }

    let target = window.clone();
    let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(1);
    window.run_on_main_thread(move || {
        let result =
            hide_list_lyrics_window_controls_on_main(&target).map_err(|error| error.to_string());
        let _ = result_sender.send(result);
    })?;
    match result_receiver.recv() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(std::io::Error::other(error).into()),
        Err(error) => Err(std::io::Error::other(format!(
            "macOS window control update was interrupted: {error}"
        ))
        .into()),
    }
}

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
    let title = app
        .state::<AppState>()
        .config
        .snapshot()
        .app
        .language
        .native_language()
        .native_labels()
        .list_title;
    let window_builder = WebviewWindowBuilder::new(
        app,
        "lyrics-list",
        crate::webview_url(app, "index.html?view=lyrics-list"),
    )
    .title(title)
    .inner_size(LIST_LYRICS_DEFAULT_WIDTH, LIST_LYRICS_DEFAULT_HEIGHT)
    .min_inner_size(LIST_LYRICS_MINIMUM_WIDTH, LIST_LYRICS_MINIMUM_HEIGHT)
    .transparent(true)
    .accept_first_mouse(true)
    .shadow(false)
    .resizable(true)
    .maximizable(false)
    .minimizable(true)
    .always_on_top(always_on_top)
    .visible(false);
    #[cfg(target_os = "macos")]
    let window_builder = window_builder
        .decorations(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    #[cfg(not(target_os = "macos"))]
    let window_builder = window_builder.decorations(false).center();
    let window = window_builder.build()?;
    #[cfg(not(target_os = "macos"))]
    if let Err(error) = window.restore_state(StateFlags::SIZE) {
        log::warn!("恢复歌词窗口尺寸失败：{error}");
    }
    #[cfg(target_os = "macos")]
    let restored = super::platform::enable_native_frame_autosave(
        &window,
        super::platform::LIST_FRAME_AUTOSAVE_NAME,
        true,
    )?;
    #[cfg(target_os = "macos")]
    super::restore_screen_affinity(app, &window, restored);
    #[cfg(not(target_os = "macos"))]
    restore_list_lyrics_position(app, &window);
    #[cfg(target_os = "macos")]
    hide_list_lyrics_window_controls(&window)?;
    let enabled = app
        .state::<AppState>()
        .config
        .snapshot()
        .app
        .lyrics_windows_show_on_all_spaces;
    apply_list_lyrics_window_space_behavior(&window, enabled)?;
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
        #[cfg(target_os = "macos")]
        hide_list_lyrics_window_controls(&window).map_err(|error| error.to_string())?;
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
