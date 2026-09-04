#[tauri::command]
pub fn get_app_config(state: State<'_, AppState>) -> AppConfig {
    state.config.snapshot()
}

#[tauri::command]
pub fn get_legal_notice_status(state: State<'_, AppState>) -> Result<LegalNoticeStatus, String> {
    Ok(LegalNoticeStatus {
        current_version: crate::LEGAL_NOTICE_VERSION,
        accepted: crate::legal_notice_accepted(&state.storage)?,
    })
}

#[tauri::command]
pub fn accept_legal_notice(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.storage.set_preference(
        crate::LEGAL_NOTICE_PREFERENCE,
        &crate::LEGAL_NOTICE_VERSION.to_string(),
    )?;
    crate::activate_runtime(&app)
}

#[tauri::command]
pub fn quit_application(app: tauri::AppHandle) {
    log::info!("Application exit requested: reason=frontend_quit_command");
    app.exit(0);
}

#[tauri::command]
pub fn set_theme(
    app: tauri::AppHandle,
    theme: ThemePreference,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state.config.update(|config| config.app.theme = theme)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn resolve_system_media_applications(
    paths: Vec<PathBuf>,
) -> Result<Vec<RegisteredApplication>, String> {
    application_discovery::discover_system_media_applications(paths)
}

#[tauri::command]
pub fn set_system_media_applications(
    app: tauri::AppHandle,
    applications: Vec<RegisteredApplication>,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let applications = normalize_system_media_applications(applications)?;
    let config = state
        .config
        .update(|config| config.app.system_media_applications = applications)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_system_media_filter_mode(
    app: tauri::AppHandle,
    mode: SystemMediaFilterMode,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.app.system_media_filter_mode = mode)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn resolve_player_follower_application(path: PathBuf) -> Result<RegisteredApplication, String> {
    application_discovery::discover_player_follower_application(&path)
}

#[tauri::command]
pub fn set_player_follower_application(
    app: tauri::AppHandle,
    application: Option<RegisteredApplication>,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let application = normalize_player_follower_application(application)?;
    let config = state
        .config
        .update(|config| config.app.player_follower_application = application)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    crate::player_lifecycle::sync_service(&app, &config.app)?;
    Ok(config)
}

#[tauri::command]
pub fn get_player_follower_service_status() -> crate::player_lifecycle::PlayerFollowerServiceState {
    crate::player_lifecycle::service_state()
}

#[tauri::command]
pub fn open_player_follower_system_settings() -> Result<(), String> {
    crate::player_lifecycle::open_system_settings()
}

#[tauri::command]
pub fn open_automation_system_settings(app: tauri::AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation",
            None::<&str>,
        )
        .map_err(|error| format!("打开自动化系统设置失败：{error}"))
}

#[tauri::command]
pub async fn get_application_icons(
    bundle_ids: Vec<String>,
) -> Result<HashMap<String, String>, String> {
    tauri::async_runtime::spawn_blocking(move || collect_application_icons(bundle_ids))
        .await
        .map_err(|error| format!("读取应用图标失败：{error}"))
}

#[tauri::command]
pub async fn resolve_application_by_bundle_id(
    bundle_id: String,
) -> Result<RegisteredApplication, String> {
    tauri::async_runtime::spawn_blocking(move || resolve_application_bundle_id(&bundle_id))
        .await
        .map_err(|error| format!("读取应用信息失败：{error}"))?
}

#[tauri::command]
pub fn set_language(
    app: tauri::AppHandle,
    language: LanguagePreference,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    if !language.is_valid() {
        return Err("language must be system or a valid BCP 47 language tag".into());
    }
    let config = state
        .config
        .update(|config| config.app.language = language)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_native_language(
    app: tauri::AppHandle,
    language: UiLanguage,
    state: State<'_, AppState>,
) -> Result<(), String> {
    crate::apply_native_language(&app, language)?;
    if state.config.set_comment_language(language)? {
        app.emit("config://changed", state.config.snapshot())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_global_shortcut_status(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<GlobalShortcutStatus, String> {
    let ([toggle, unlock, reset], [toggle_status_bar, toggle_list, toggle_notch, switch_lyrics]) =
        state.config.snapshot().app.shortcuts.parsed()?;
    let shortcuts = app.global_shortcut();
    Ok(GlobalShortcutStatus {
        toggle_overlay: shortcuts.is_registered(toggle),
        unlock_overlay: shortcuts.is_registered(unlock),
        reset_overlay: shortcuts.is_registered(reset),
        toggle_status_bar_lyrics: toggle_status_bar
            .is_some_and(|shortcut| shortcuts.is_registered(shortcut)),
        toggle_list_lyrics: toggle_list.is_some_and(|shortcut| shortcuts.is_registered(shortcut)),
        toggle_notch_lyrics: toggle_notch.is_some_and(|shortcut| shortcuts.is_registered(shortcut)),
        switch_lyrics: switch_lyrics.is_some_and(|shortcut| shortcuts.is_registered(shortcut)),
    })
}

#[tauri::command]
pub fn set_global_shortcuts(
    app: tauri::AppHandle,
    shortcuts: GlobalShortcutSettings,
) -> Result<AppConfig, String> {
    update_global_shortcuts(&app, shortcuts)
}

#[tauri::command]
pub fn set_dock_icon_hidden(app: tauri::AppHandle, hidden: bool) -> Result<AppConfig, String> {
    update_dock_icon_hidden(&app, hidden)
}

#[tauri::command]
pub fn set_menu_bar_icon_hidden(app: tauri::AppHandle, hidden: bool) -> Result<AppConfig, String> {
    update_menu_bar_icon_hidden(&app, hidden)
}

#[tauri::command]
pub fn set_silent_startup(
    app: tauri::AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.app.silent_startup = enabled)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_auto_check_updates(
    app: tauri::AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.app.auto_check_updates = enabled)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_overlay_hide_when_not_playing(
    app: tauri::AppHandle,
    hidden: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.overlay.hide_when_not_playing = hidden)?;
    crate::reconcile_overlay_visibility(&app)?;
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_lyrics_windows_show_on_all_spaces(
    app: tauri::AppHandle,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.app.lyrics_windows_show_on_all_spaces = enabled)?;
    crate::apply_lyrics_windows_space_behavior(&app, enabled).map_err(|error| error.to_string())?;
    crate::sync_lyrics_surfaces(&app);
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn set_lyrics_base_appearance(
    app: tauri::AppHandle,
    appearance: LyricsBaseAppearance,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.lyrics.base_appearance = appearance.clone())?;
    sync_desktop_style_from_config(&app, &state, &config)?;
    finish_display_config_update(&app, config)
}

#[tauri::command]
pub fn set_lyrics_style_inheritance(
    app: tauri::AppHandle,
    mode: LyricsStyleMode,
    inheritance: LyricsModeStyleInheritance,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state.config.update(|config| match mode {
        LyricsStyleMode::Desktop => config.lyrics.style_inheritance.desktop = inheritance,
        LyricsStyleMode::StatusBar => config.lyrics.style_inheritance.status_bar = inheritance,
        LyricsStyleMode::ListWindow => config.lyrics.style_inheritance.list_window = inheritance,
        LyricsStyleMode::Notch => config.lyrics.style_inheritance.notch = inheritance,
    })?;
    if matches!(mode, LyricsStyleMode::Desktop) {
        sync_desktop_style_from_config(&app, &state, &config)?;
    }
    finish_display_config_update(&app, config)
}
