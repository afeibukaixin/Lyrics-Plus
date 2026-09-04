use tauri::{Emitter, Manager};

use crate::config::{AppConfig, GlobalShortcutSettings};
use crate::runtime_model::OverlaySettings;
use crate::AppState;

pub(super) fn update_global_shortcuts(
    app: &tauri::AppHandle,
    shortcuts: GlobalShortcutSettings,
) -> Result<AppConfig, String> {
    let state = app.state::<AppState>();
    let previous = state.config.snapshot().app.shortcuts;
    crate::apply_global_shortcuts(app, &previous, &shortcuts)?;
    let registered = shortcuts.clone();
    let config = match state
        .config
        .update(|config| config.app.shortcuts = shortcuts)
    {
        Ok(config) => config,
        Err(error) => {
            let _ = crate::apply_global_shortcuts(app, &registered, &previous);
            return Err(error);
        }
    };
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

pub(super) fn update_dock_icon_hidden(
    app: &tauri::AppHandle,
    hidden: bool,
) -> Result<AppConfig, String> {
    let state = app.state::<AppState>();
    let previous = state.config.snapshot().app.hide_dock_icon;
    crate::apply_dock_icon_hidden(app, hidden)?;
    let config = match state
        .config
        .update(|config| config.app.hide_dock_icon = hidden)
    {
        Ok(config) => config,
        Err(error) => {
            let _ = crate::apply_dock_icon_hidden(app, previous);
            return Err(error);
        }
    };
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

pub(super) fn update_menu_bar_icon_hidden(
    app: &tauri::AppHandle,
    hidden: bool,
) -> Result<AppConfig, String> {
    let state = app.state::<AppState>();
    let previous = state.config.snapshot().app.hide_menu_bar_icon;
    crate::apply_menu_bar_icon_hidden(app, hidden)?;
    let config = match state
        .config
        .update(|config| config.app.hide_menu_bar_icon = hidden)
    {
        Ok(config) => config,
        Err(error) => {
            let _ = crate::apply_menu_bar_icon_hidden(app, previous);
            return Err(error);
        }
    };
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

pub(super) fn finish_display_config_update(
    app: &tauri::AppHandle,
    config: AppConfig,
) -> Result<AppConfig, String> {
    crate::sync_lyrics_surfaces(app);
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

pub(super) fn apply_app_config(
    app: &tauri::AppHandle,
    state: &AppState,
    next: AppConfig,
    expected_revision: u64,
) -> Result<AppConfig, String> {
    let previous_config = state.config.snapshot();
    let previous_dock_icon_hidden = previous_config.app.hide_dock_icon;
    let previous_menu_bar_icon_hidden = previous_config.app.hide_menu_bar_icon;
    let previous_shortcuts = previous_config.app.shortcuts.clone();
    let dock_visibility_changed = previous_dock_icon_hidden != next.app.hide_dock_icon;
    let menu_bar_icon_visibility_changed =
        previous_menu_bar_icon_hidden != next.app.hide_menu_bar_icon;
    let chinese_conversion_changed =
        previous_config.lyrics.chinese_conversion != next.lyrics.chinese_conversion;
    let japanese_repair_changed =
        previous_config.lyrics.repair_simplified_japanese != next.lyrics.repair_simplified_japanese;
    if dock_visibility_changed {
        crate::apply_dock_icon_hidden(app, next.app.hide_dock_icon)?;
    }
    if menu_bar_icon_visibility_changed {
        if let Err(error) = crate::apply_menu_bar_icon_hidden(app, next.app.hide_menu_bar_icon) {
            if dock_visibility_changed {
                let _ = crate::apply_dock_icon_hidden(app, previous_dock_icon_hidden);
            }
            return Err(error);
        }
    }
    let shortcuts_changed = previous_shortcuts != next.app.shortcuts;
    if shortcuts_changed {
        if let Err(error) =
            crate::apply_global_shortcuts(app, &previous_shortcuts, &next.app.shortcuts)
        {
            if dock_visibility_changed {
                let _ = crate::apply_dock_icon_hidden(app, previous_dock_icon_hidden);
            }
            if menu_bar_icon_visibility_changed {
                let _ = crate::apply_menu_bar_icon_hidden(app, previous_menu_bar_icon_hidden);
            }
            return Err(error);
        }
    }
    let save_result = state
        .config
        .replace_at_revision(next.clone(), expected_revision);
    let saved = match save_result {
        Ok(saved) => saved,
        Err(error) => {
            if dock_visibility_changed {
                let _ = crate::apply_dock_icon_hidden(app, previous_dock_icon_hidden);
            }
            if menu_bar_icon_visibility_changed {
                let _ = crate::apply_menu_bar_icon_hidden(app, previous_menu_bar_icon_hidden);
            }
            if shortcuts_changed {
                let _ =
                    crate::apply_global_shortcuts(app, &next.app.shortcuts, &previous_shortcuts);
            }
            return Err(error);
        }
    };

    let geometry = {
        let style = state
            .overlay_style
            .read()
            .unwrap_or_else(|error| error.into_inner());
        (style.horizontal_max_width, style.vertical_max_height)
    };
    let mut style = saved.overlay.appearance.clone().into_style();
    style.horizontal_max_width = geometry.0;
    style.vertical_max_height = geometry.1;
    *state
        .overlay_style
        .write()
        .unwrap_or_else(|error| error.into_inner()) = style.clone();
    if let Some(window) = app.get_webview_window("lyrics-overlay") {
        crate::sync_overlay_vibrancy(&window, &style);
    }

    state
        .providers
        .set_settings(saved.lyrics.providers.clone())?;
    super::invalidate_lyrics_search_session(state);
    if chinese_conversion_changed || japanese_repair_changed {
        super::republish_lyrics_runtime(app);
    }
    *state
        .selection
        .write()
        .unwrap_or_else(|error| error.into_inner()) = saved.app.player_selection;
    *state
        .auto_player
        .write()
        .unwrap_or_else(|error| error.into_inner()) = None;
    *state
        .overlay_settings
        .write()
        .unwrap_or_else(|error| error.into_inner()) = OverlaySettings {
        visible: saved.overlay.visible,
        locked: saved.overlay.locked,
    };
    if let Some(window) = app.get_webview_window("lyrics-overlay") {
        let _ = window.set_ignore_cursor_events(saved.overlay.locked);
        let _ = window.set_focusable(!saved.overlay.locked);
        if !saved.overlay.locked {
            crate::refresh_overlay_mouse_tracking(&window);
        }
    }
    crate::reconcile_overlay_visibility(app)?;
    crate::sync_tray_overlay_checked(app, saved.overlay.visible);
    crate::sync_lyrics_surfaces(app);
    let _ = app.emit("player://selection", saved.app.player_selection);
    let _ = app.emit(
        "overlay://settings",
        super::get_overlay_settings_inner(state),
    );
    let _ = app.emit("overlay://style", &style);
    app.emit("config://changed", &saved)
        .map_err(|error| error.to_string())?;
    crate::player_lifecycle::sync_service(app, &saved.app)?;
    Ok(saved)
}
