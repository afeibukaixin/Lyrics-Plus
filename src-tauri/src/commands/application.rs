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

fn plist_string(path: &Path, key: &str) -> Option<String> {
    let mut command = Command::new("/usr/bin/plutil");
    command.args(["-extract", key, "raw", "-o", "-"]).arg(path);
    let output = run_with_timeout(command, Duration::from_secs(3)).ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(target_os = "macos")]
fn localized_application_name(path: &Path) -> Option<String> {
    use objc2_foundation::{NSBundle, NSString};

    let bundle_path = NSString::from_str(path.to_string_lossy().as_ref());
    let bundle = NSBundle::bundleWithPath(&bundle_path)?;
    ["CFBundleDisplayName", "CFBundleName"]
        .into_iter()
        .find_map(|key| {
            let value = bundle.objectForInfoDictionaryKey(&NSString::from_str(key))?;
            let value = value.downcast_ref::<NSString>()?.to_string();
            (!value.trim().is_empty()).then_some(value)
        })
}

#[cfg(not(target_os = "macos"))]
fn localized_application_name(_path: &Path) -> Option<String> {
    None
}

fn application_display_name(name: String) -> String {
    name.strip_suffix(".app").unwrap_or(&name).to_owned()
}

fn resolve_registered_application(path: &Path) -> Result<RegisteredApplication, String> {
    if !path.is_dir() || path.extension().and_then(|value| value.to_str()) != Some("app") {
        return Err(format!("不是有效的 .app：{}", path.display()));
    }
    let plist = ["Contents/Info.plist", "WrappedBundle/Info.plist"]
        .into_iter()
        .map(|relative_path| path.join(relative_path))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("应用缺少 Info.plist：{}", path.display()))?;
    let bundle_id = plist_string(&plist, "CFBundleIdentifier")
        .ok_or_else(|| format!("应用缺少 Bundle ID：{}", path.display()))?;
    let name = localized_application_name(path)
        .or_else(|| plist_string(&plist, "CFBundleDisplayName"))
        .or_else(|| plist_string(&plist, "CFBundleName"))
        .or_else(|| {
            path.file_stem()
                .map(|value| value.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| bundle_id.clone());
    Ok(RegisteredApplication {
        name: application_display_name(name),
        bundle_id,
    })
}

#[tauri::command]
pub fn resolve_system_media_applications(
    paths: Vec<PathBuf>,
) -> Result<Vec<RegisteredApplication>, String> {
    normalize_system_media_applications(
        paths
            .iter()
            .map(|path| resolve_registered_application(path))
            .collect::<Result<Vec<_>, _>>()?,
    )
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
    normalize_player_follower_application(Some(resolve_registered_application(&path)?))?
        .ok_or_else(|| "未选择播放器".into())
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

fn collect_application_icons(bundle_ids: Vec<String>) -> HashMap<String, String> {
    bundle_ids
        .into_iter()
        .filter_map(|bundle_id| application_icon(&bundle_id).map(|icon| (bundle_id, icon)))
        .collect()
}

#[cfg(target_os = "macos")]
fn application_icon(bundle_id: &str) -> Option<String> {
    use objc2::rc::autoreleasepool;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let url =
            workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))?;
        let path = url.path()?;
        application_icon_at_path(&path.to_string())
    })
}

#[cfg(target_os = "macos")]
fn application_icon_at_path(path: &str) -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use objc2::{rc::autoreleasepool, AnyThread};
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let icon = workspace.iconForFile(&NSString::from_str(path));
        let mut bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(64.0, 64.0));
        let image = unsafe { icon.CGImageForProposedRect_context_hints(&mut bounds, None, None)? };
        let bitmap = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &image);
        let properties = NSDictionary::new();
        let png = unsafe {
            bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)?
        };
        Some(format!(
            "data:image/png;base64,{}",
            STANDARD.encode(png.to_vec())
        ))
    })
}

#[cfg(not(target_os = "macos"))]
fn application_icon(_bundle_id: &str) -> Option<String> {
    None
}

#[tauri::command]
pub async fn resolve_application_by_bundle_id(
    bundle_id: String,
) -> Result<RegisteredApplication, String> {
    tauri::async_runtime::spawn_blocking(move || resolve_application_bundle_id(&bundle_id))
        .await
        .map_err(|error| format!("读取应用信息失败：{error}"))?
}

#[cfg(target_os = "macos")]
fn resolve_application_bundle_id(bundle_id: &str) -> Result<RegisteredApplication, String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    let workspace = NSWorkspace::sharedWorkspace();
    let url = workspace
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))
        .ok_or_else(|| format!("找不到应用：{bundle_id}"))?;
    let path = url
        .path()
        .ok_or_else(|| format!("无法读取应用路径：{bundle_id}"))?;
    resolve_registered_application(Path::new(&path.to_string()))
}

#[cfg(not(target_os = "macos"))]
fn resolve_application_bundle_id(_bundle_id: &str) -> Result<RegisteredApplication, String> {
    Err("应用解析仅支持 macOS".into())
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
    let ([toggle, unlock, reset], [toggle_status_bar, toggle_list, toggle_notch]) =
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
    })
}

pub fn update_global_shortcuts(
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

#[tauri::command]
pub fn set_global_shortcuts(
    app: tauri::AppHandle,
    shortcuts: GlobalShortcutSettings,
) -> Result<AppConfig, String> {
    update_global_shortcuts(&app, shortcuts)
}

pub fn update_dock_icon_hidden(app: &tauri::AppHandle, hidden: bool) -> Result<AppConfig, String> {
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

#[tauri::command]
pub fn set_dock_icon_hidden(app: tauri::AppHandle, hidden: bool) -> Result<AppConfig, String> {
    update_dock_icon_hidden(&app, hidden)
}

pub fn update_menu_bar_icon_hidden(
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
    crate::apply_lyrics_windows_space_behavior(&app, enabled)
        .map_err(|error| error.to_string())?;
    crate::sync_lyrics_surfaces(&app);
    app.emit("config://changed", &config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

fn finish_display_config_update(
    app: &tauri::AppHandle,
    config: AppConfig,
) -> Result<AppConfig, String> {
    crate::sync_lyrics_surfaces(app);
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
