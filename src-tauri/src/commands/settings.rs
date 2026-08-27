#[tauri::command]
pub fn reset_lyrics_base_appearance(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state.config.update(|config| {
        config.lyrics.base_appearance = LyricsBaseAppearance::default();
    })?;
    sync_desktop_style_from_config(&app, &state, &config)?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_status_bar_lyrics_enabled(
    app: tauri::AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.lyrics.displays.status_bar.enabled = enabled)?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_list_lyrics_visible(
    app: tauri::AppHandle,
    visible: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.lyrics.displays.list_window.enabled = visible)?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_list_lyrics_options(
    app: tauri::AppHandle,
    show_translation: bool,
    show_romanization: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state.config.update(|config| {
        config.lyrics.displays.list_window.show_translation = show_translation;
        config.lyrics.displays.list_window.show_romanization = show_romanization;
    })?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_notch_lyrics_visible(
    app: tauri::AppHandle,
    visible: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.lyrics.displays.notch.enabled = visible)?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_lyrics_display_preferences(
    app: tauri::AppHandle,
    mode: LyricsStyleMode,
    preferences: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = match mode {
        LyricsStyleMode::Desktop => return Err("桌面歌词样式请使用桌面样式接口".into()),
        LyricsStyleMode::StatusBar => {
            let value = serde_json::from_value::<StatusBarLyricsPreferences>(preferences)
                .map_err(|error| format!("菜单栏歌词配置无效：{error}"))?;
            state
                .config
                .update(|config| config.lyrics.displays.status_bar = value.clone())?
        }
        LyricsStyleMode::ListWindow => {
            let value = serde_json::from_value::<ListLyricsPreferences>(preferences)
                .map_err(|error| format!("歌词窗口配置无效：{error}"))?;
            state
                .config
                .update(|config| config.lyrics.displays.list_window = value.clone())?
        }
        LyricsStyleMode::Notch => {
            let value = serde_json::from_value::<NotchLyricsPreferences>(preferences)
                .map_err(|error| format!("灵动岛歌词配置无效：{error}"))?;
            state
                .config
                .update(|config| config.lyrics.displays.notch = value.clone())?
        }
    };
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn reset_lyrics_style_mode(
    app: tauri::AppHandle,
    mode: LyricsStyleMode,
    state: State<'_, AppState>,
) -> Result<SettingsResetResponse, String> {
    if matches!(mode, LyricsStyleMode::Desktop) {
        return reset_settings_section(app, SettingsSection::Style, state);
    }
    state.config.update(|config| match mode {
        LyricsStyleMode::StatusBar => {
            config.lyrics.displays.status_bar = Default::default();
            config.lyrics.style_inheritance.status_bar = Default::default();
        }
        LyricsStyleMode::ListWindow => {
            config.lyrics.displays.list_window = Default::default();
            config.lyrics.style_inheritance.list_window = Default::default();
        }
        LyricsStyleMode::Notch => {
            config.lyrics.displays.notch = Default::default();
            config.lyrics.style_inheritance.notch = Default::default();
        }
        LyricsStyleMode::Desktop => {}
    })?;
    let configured = state.config.snapshot();
    crate::sync_lyrics_surfaces(&app);
    app.emit("config://changed", &configured)
        .map_err(|error| error.to_string())?;
    Ok(SettingsResetResponse {
        overlay_settings: get_overlay_settings_inner(&state),
        overlay_style: state
            .overlay_style
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone(),
        provider_view: state.providers.settings_view(),
        player_selection: *state
            .selection
            .read()
            .unwrap_or_else(|error| error.into_inner()),
    })
}

#[tauri::command]
pub fn reset_lyrics_display_position(
    app: tauri::AppHandle,
    mode: LyricsStyleMode,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let label = match mode {
        LyricsStyleMode::StatusBar => "lyrics-status-bar",
        LyricsStyleMode::ListWindow => "lyrics-list",
        LyricsStyleMode::Notch => "lyrics-notch",
        LyricsStyleMode::Desktop => return Err("桌面歌词请使用桌面位置复位命令".into()),
    };
    if label == "lyrics-status-bar" {
        state
            .storage
            .remove_preference("lyrics-status-bar.position")?;
        state
            .storage
            .remove_preference("lyrics-status-bar.last-monitor")?;
        state
            .storage
            .remove_preferences_with_prefix("lyrics-status-bar.position.")?;
    } else {
        state
            .storage
            .remove_preferences_with_prefix(&format!("{label}.position."))?;
    }
    if let Some(window) = app.get_webview_window(label) {
        crate::position_auxiliary_lyrics_window_default(&app, &window, label)?;
    }
    Ok(())
}

#[tauri::command]
pub fn reset_list_lyrics_window_size(app: tauri::AppHandle) -> Result<(), String> {
    crate::reset_list_lyrics_window_size(&app)
}

#[tauri::command]
pub fn export_app_config(state: State<'_, AppState>) -> Result<ConfigExport, String> {
    Ok(ConfigExport {
        file_name: "lyrics-plus-config.jsonc".into(),
        raw: state.config.export_json()?,
    })
}

#[tauri::command]
pub fn reveal_config_directory(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let directory = state
        .config
        .path()
        .parent()
        .ok_or_else(|| "配置目录无效".to_string())?;
    app.opener()
        .open_path(directory.to_string_lossy(), None::<&str>)
        .map_err(|error| format!("打开配置目录失败：{error}"))
}

#[tauri::command]
pub fn get_config_editor_data(state: State<'_, AppState>) -> ConfigEditorData {
    state.config.editor_data()
}

#[tauri::command]
pub fn validate_app_config_draft(raw: String) -> ConfigDraftValidation {
    validate_config_draft(&raw)
}

#[tauri::command]
pub fn save_app_config_draft(
    app: tauri::AppHandle,
    raw: String,
    expected_revision: u64,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let validation = validate_config_draft(&raw);
    if let Some(error) = validation.error {
        return Err(format!(
            "第 {} 行第 {} 列：{}",
            error.line, error.column, error.message
        ));
    }
    apply_app_config(&app, &state, validation.effective_config, expected_revision)
}

fn apply_app_config(
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
    invalidate_lyrics_search_session(&state);
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
    let _ = app.emit("overlay://settings", get_overlay_settings_inner(&state));
    let _ = app.emit("overlay://style", &style);
    app.emit("config://changed", &saved)
        .map_err(|error| error.to_string())?;
    crate::player_lifecycle::sync_service(app, &saved.app)?;
    Ok(saved)
}

#[tauri::command]
pub fn reset_settings_section(
    app: tauri::AppHandle,
    section: SettingsSection,
    state: State<'_, AppState>,
) -> Result<SettingsResetResponse, String> {
    let mut player_follower_error = None;
    match section {
        SettingsSection::Style => {
            state
                .storage
                .remove_preferences_with_prefix("overlay.style.")?;

            let geometry = {
                let current = state
                    .overlay_style
                    .read()
                    .unwrap_or_else(|error| error.into_inner());
                (current.horizontal_max_width, current.vertical_max_height)
            };
            *state
                .overlay_settings
                .write()
                .unwrap_or_else(|error| error.into_inner()) = OverlaySettings::default();
            let configured = state.config.update(|config| {
                config.overlay.appearance = OverlayAppearance::default();
                config.overlay.visible = true;
                config.overlay.locked = false;
                config.overlay.hide_when_not_playing = false;
                config.lyrics.style_inheritance.desktop = Default::default();
            })?;
            let mut style = configured.overlay.appearance.into_style();
            style.horizontal_max_width = geometry.0;
            style.vertical_max_height = geometry.1;
            *state
                .overlay_style
                .write()
                .unwrap_or_else(|error| error.into_inner()) = style.clone();

            if let Some(window) = app.get_webview_window("lyrics-overlay") {
                crate::sync_overlay_vibrancy(&window, &style);
                crate::reset_overlay_toolbar_placement(&app, style.orientation);
            }
            app.emit("overlay://style", &style)
                .map_err(|error| error.to_string())?;
            app.emit("overlay://settings", get_overlay_settings_inner(&state))
                .map_err(|error| error.to_string())?;
            crate::reconcile_overlay_visibility(&app)?;
            crate::sync_tray_overlay_checked(&app, true);
        }
        SettingsSection::Display => {
            state
                .storage
                .remove_preferences_with_prefix("overlay.position.")?;
            state
                .storage
                .remove_preferences_with_prefix("overlay.geometry.")?;
            state.storage.remove_preference("overlay.last_monitor")?;
            state.storage.remove_preference("overlay.visible")?;
            state.storage.remove_preference("overlay.locked")?;
            state.storage.remove_preference("overlay.passthrough")?;

            let style = {
                let mut current = state
                    .overlay_style
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone();
                current.horizontal_max_width = None;
                current.vertical_max_height = None;
                current
            };
            *state
                .overlay_style
                .write()
                .unwrap_or_else(|error| error.into_inner()) = style.clone();
            *state
                .overlay_monitor
                .write()
                .unwrap_or_else(|error| error.into_inner()) = None;
            state
                .overlay_placement
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .preferred_monitor = None;
            *state
                .overlay_settings
                .write()
                .unwrap_or_else(|error| error.into_inner()) = OverlaySettings::default();
            state.config.update(|config| {
                config.overlay.visible = true;
                config.overlay.locked = false;
                config.overlay.hide_when_not_playing = false;
                config.lyrics.displays = Default::default();
            })?;

            let window = app
                .get_webview_window("lyrics-overlay")
                .ok_or_else(|| "歌词浮窗不存在".to_string())?;
            crate::sync_overlay_vibrancy(&window, &style);
            window
                .set_ignore_cursor_events(false)
                .map_err(|error| error.to_string())?;
            let _ = window.set_focusable(true);
            crate::refresh_overlay_mouse_tracking(&window);
            let _ = window.set_resizable(false);
            crate::restore_overlay_position(&app, &window);
            crate::reconcile_overlay_visibility(&app)?;
            crate::sync_tray_overlay_checked(&app, true);
            app.emit("overlay://settings", get_overlay_settings_inner(&state))
                .map_err(|error| error.to_string())?;
            app.emit("overlay://style", &style)
                .map_err(|error| error.to_string())?;
        }
        SettingsSection::Lyrics => {
            let view = state.providers.set_settings(ProviderSettings::default())?;
            state
                .config
                .update(|config| config.lyrics.providers = view.settings)?;
            invalidate_lyrics_search_session(&state);
        }
        SettingsSection::Player => {
            update_player_selection(&app, PlayerSelection::Auto)?;
            let config = state.config.update(|config| {
                config.app.system_media_filter_mode = SystemMediaFilterMode::Allowlist;
                config.app.system_media_applications.clear();
                config.app.player_follower_application = None;
            })?;
            player_follower_error = crate::player_lifecycle::sync_service(&app, &config.app).err();
        }
        SettingsSection::Application => {
            update_dock_icon_hidden(&app, false)?;
            update_menu_bar_icon_hidden(&app, false)?;
            update_global_shortcuts(&app, GlobalShortcutSettings::default())?;
            state.config.update(|config| {
                config.app.theme = ThemePreference::Dark;
                config.app.language = LanguagePreference::default();
                config.app.silent_startup = false;
                config.app.lyrics_windows_show_on_all_spaces = false;
            })?;
            crate::apply_lyrics_windows_space_behavior(&app, false)
                .map_err(|error| error.to_string())?;
        }
        SettingsSection::About => {
            state
                .config
                .update(|config| config.app.auto_check_updates = true)?;
        }
    }

    let configured = state.config.snapshot();
    crate::sync_lyrics_surfaces(&app);
    let _ = app.emit("config://changed", &configured);
    if let Some(error) = player_follower_error {
        return Err(error);
    }
    Ok(SettingsResetResponse {
        overlay_settings: get_overlay_settings_inner(&state),
        overlay_style: state
            .overlay_style
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone(),
        provider_view: state.providers.settings_view(),
        player_selection: *state
            .selection
            .read()
            .unwrap_or_else(|error| error.into_inner()),
    })
}
