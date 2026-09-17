#[cfg(target_os = "macos")]
use std::cell::{Cell, RefCell};

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::NSObject;
#[cfg(target_os = "macos")]
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSFont, NSFontChanging, NSFontManager, NSFontPanel, NSFontTraitMask};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSNumber, NSObjectProtocol, NSString};

#[cfg(target_os = "macos")]
use crate::font_weight::{css_weight_from_font_manager, font_manager_weight, system_font_weight};

#[cfg(target_os = "macos")]
fn is_system_font_family(family: &str) -> bool {
    matches!(
        family.trim().to_ascii_lowercase().as_str(),
        "-apple-system" | "system-ui" | "blinkmacsystemfont" | "sans-serif"
    )
}

#[cfg(target_os = "macos")]
fn available_font_weights(manager: &NSFontManager, family: &str) -> Option<Vec<u16>> {
    if is_system_font_family(family) {
        return Some((1..=9).map(|weight| weight * 100).collect());
    }
    let members = manager.availableMembersOfFontFamily(&NSString::from_str(family))?;
    let mut weights = Vec::new();
    for index in 0..members.count() {
        let member = members.objectAtIndex(index);
        if member.count() < 4 {
            continue;
        }
        let weight_member = member.objectAtIndex(2);
        let Some(weight) = weight_member.downcast_ref::<NSNumber>() else {
            continue;
        };
        let traits_member = member.objectAtIndex(3);
        let Some(traits) = traits_member.downcast_ref::<NSNumber>() else {
            continue;
        };
        let excluded = NSFontTraitMask::ItalicFontMask
            | NSFontTraitMask::NarrowFontMask
            | NSFontTraitMask::ExpandedFontMask
            | NSFontTraitMask::CondensedFontMask
            | NSFontTraitMask::CompressedFontMask;
        if (traits.doubleValue() as usize) & excluded.bits() != 0 {
            continue;
        }
        let css_weight = css_weight_from_font_manager(weight.doubleValue() as isize);
        if !weights.contains(&css_weight) {
            weights.push(css_weight);
        }
    }
    if weights.is_empty() {
        return None;
    }
    weights.sort_unstable();
    Some(weights)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FontPanelSelection {
    name: String,
    family: String,
    font_weight: Option<u16>,
}

#[cfg(target_os = "macos")]
thread_local! {
    static FONT_PANEL_TARGET: RefCell<Option<Retained<FontPanelTarget>>> = const { RefCell::new(None) };
}

#[cfg(target_os = "macos")]
struct FontPanelTargetIvars {
    app: tauri::AppHandle,
    selected_font: RefCell<Retained<NSFont>>,
    requested_weight: Cell<u16>,
}

#[cfg(target_os = "macos")]
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = FontPanelTargetIvars]
    struct FontPanelTarget;

    unsafe impl NSObjectProtocol for FontPanelTarget {}

    unsafe impl NSFontChanging for FontPanelTarget {
        #[unsafe(method(changeFont:))]
        fn change_font(&self, _sender: Option<&NSFontManager>) {
            let manager = NSFontManager::sharedFontManager(self.mtm());
            // A family change keeps the requested weight. A face change within
            // the same family updates the lyric weight, but size does not.
            let selected_font = self.ivars().selected_font.borrow();
            let converted_font = manager.convertFont(&selected_font);
            let family = converted_font
                .familyName()
                .or_else(|| Some(converted_font.fontName()))
                .map(|value| value.to_string())
                .filter(|value| !value.trim().is_empty());
            let Some(family) = family else {
                return;
            };
            let selected_family = selected_font
                .familyName()
                .or_else(|| Some(selected_font.fontName()))
                .map(|value| value.to_string());
            let family_changed = !selected_family
                .as_deref()
                .is_some_and(|previous| previous.eq_ignore_ascii_case(&family));
            let font = if family_changed {
                let weight = self.ivars().requested_weight.get();
                if is_system_font_family(&family) {
                    NSFont::systemFontOfSize_weight(converted_font.pointSize(), system_font_weight(weight))
                } else {
                    manager.fontWithFamily_traits_weight_size(
                        &NSString::from_str(&family),
                        NSFontTraitMask::empty(),
                        font_manager_weight(weight),
                        converted_font.pointSize(),
                    ).unwrap_or(converted_font)
                }
            } else {
                converted_font
            };
            let size_changed = selected_font.pointSize() != font.pointSize();
            let was_italic = manager.traitsOfFont(&selected_font).contains(NSFontTraitMask::ItalicFontMask);
            let is_italic = manager.traitsOfFont(&font).contains(NSFontTraitMask::ItalicFontMask);
            let previous_weight = css_weight_from_font_manager(manager.weightOfFont(&selected_font));
            let next_weight = css_weight_from_font_manager(manager.weightOfFont(&font));
            drop(selected_font);
            *self.ivars().selected_font.borrow_mut() = font;
            if family_changed {
                let selected = Retained::clone(&*self.ivars().selected_font.borrow());
                manager.setSelectedFont_isMultiple(&selected, false);
            }
            let weight_changed = !size_changed && was_italic == is_italic && previous_weight != next_weight;
            if !family_changed && weight_changed {
                self.ivars().requested_weight.set(next_weight);
            }
            if !family_changed && !weight_changed {
                return;
            }
            let name = family.clone();
            let _ = self.ivars().app.emit(
                "font://selected",
                FontPanelSelection {
                    name,
                    family,
                    font_weight: (!family_changed && weight_changed).then_some(next_weight),
                },
            );
        }
    }
);

#[cfg(target_os = "macos")]
impl FontPanelTarget {
    fn new(mtm: MainThreadMarker, app: tauri::AppHandle, selected_font: Retained<NSFont>, requested_weight: u16) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(FontPanelTargetIvars {
            app,
            selected_font: RefCell::new(selected_font),
            requested_weight: Cell::new(requested_weight),
        });
        // SAFETY: NSObject's init method has the standard initializer signature.
        unsafe { msg_send![super(this), init] }
    }
}

#[tauri::command]
pub fn get_app_config(state: State<'_, AppState>) -> AppConfig {
    state.config.snapshot()
}

/// Open the native macOS font panel with the currently requested lyric weight.
#[tauri::command]
pub fn open_font_panel(app: tauri::AppHandle, family: Option<String>, font_weight: Option<u16>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let panel_app = app.clone();
        app.run_on_main_thread(move || {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let manager = NSFontManager::sharedFontManager(mtm);
            let weight = font_weight.unwrap_or(400);
            let selected = family
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .and_then(|value| {
                    if is_system_font_family(value) {
                        Some(NSFont::systemFontOfSize_weight(14.0, system_font_weight(weight)))
                    } else {
                        manager.fontWithFamily_traits_weight_size(
                            &NSString::from_str(value),
                            NSFontTraitMask::empty(),
                            font_manager_weight(weight),
                            14.0,
                        )
                    }
                })
                .unwrap_or_else(|| NSFont::systemFontOfSize_weight(14.0, system_font_weight(weight)));
            manager.setSelectedFont_isMultiple(&selected, false);
            let target = FontPanelTarget::new(mtm, panel_app, selected, weight);
            // NSFontManager keeps its target weakly; retain the target in the
            // main-thread slot for as long as the shared panel can send actions.
            unsafe {
                manager.setTarget(Some(&*target));
                manager.setAction(sel!(changeFont:));
            }
            FONT_PANEL_TARGET.with(|slot| *slot.borrow_mut() = Some(target));
            unsafe {
                manager.orderFrontFontPanel(None);
            }
        })
        .map_err(|error| format!("打开系统字体面板失败：{error}"))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, family, font_weight);
        Err("当前平台没有可用的系统字体面板".into())
    }
}

#[tauri::command]
pub async fn get_font_available_weights(app: tauri::AppHandle, family: String) -> Result<Option<Vec<u16>>, String> {
    #[cfg(target_os = "macos")]
    {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        app.run_on_main_thread(move || {
            let result = MainThreadMarker::new().and_then(|mtm| {
                available_font_weights(&NSFontManager::sharedFontManager(mtm), &family)
            });
            let _ = sender.send(result);
        })
        .map_err(|error| format!("读取字体字重失败：{error}"))?;
        receiver.await.map_err(|_| "读取字体字重失败：主线程未返回结果".to_owned())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, family);
        Ok(None)
    }
}

#[tauri::command]
pub fn close_font_panel(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        app.run_on_main_thread(move || {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            NSFontPanel::sharedFontPanel(mtm).orderOut(None);
            FONT_PANEL_TARGET.with(|slot| *slot.borrow_mut() = None);
        })
        .map_err(|error| format!("关闭系统字体面板失败：{error}"))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySettings {
    pub enabled: bool,
}

#[tauri::command]
pub fn get_telemetry_settings(state: State<'_, AppState>) -> TelemetrySettings {
    TelemetrySettings {
        enabled: state.telemetry.is_enabled(),
    }
}

#[tauri::command]
pub fn set_telemetry_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<TelemetrySettings, String> {
    state.telemetry.set_enabled(enabled)?;
    Ok(TelemetrySettings { enabled })
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
pub fn force_reregister_player_follower_service(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::player_lifecycle::PlayerFollowerServiceState, String> {
    let config = state.config.snapshot();
    crate::player_lifecycle::force_reregister_service(&app, &config.app)
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
    let comment_language = match language {
        UiLanguage::ZhCn => UiLanguage::ZhCn,
        _ => UiLanguage::EnUs,
    };
    if state.config.set_comment_language(comment_language)? {
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
pub fn reset_global_shortcut(app: tauri::AppHandle, action: String) -> Result<AppConfig, String> {
    let path = match action.as_str() {
        "toggleOverlay" => "/app/shortcuts/toggleOverlay",
        "unlockOverlay" => "/app/shortcuts/unlockOverlay",
        "resetOverlay" => "/app/shortcuts/resetOverlay",
        "toggleStatusBarLyrics" => "/app/shortcuts/toggleStatusBarLyrics",
        "toggleListLyrics" => "/app/shortcuts/toggleListLyrics",
        "toggleNotchLyrics" => "/app/shortcuts/toggleNotchLyrics",
        "switchLyrics" => "/app/shortcuts/switchLyrics",
        _ => return Err("不支持的全局快捷键配置项".into()),
    };
    let state = app.state::<AppState>();
    let previous = state.config.snapshot();
    let mut next_shortcuts = previous.app.shortcuts.clone();
    let defaults = GlobalShortcutSettings::default();
    match action.as_str() {
        "toggleOverlay" => next_shortcuts.toggle_overlay = defaults.toggle_overlay,
        "unlockOverlay" => next_shortcuts.unlock_overlay = defaults.unlock_overlay,
        "resetOverlay" => next_shortcuts.reset_overlay = defaults.reset_overlay,
        "toggleStatusBarLyrics" => {
            next_shortcuts.toggle_status_bar_lyrics = defaults.toggle_status_bar_lyrics
        }
        "toggleListLyrics" => next_shortcuts.toggle_list_lyrics = defaults.toggle_list_lyrics,
        "toggleNotchLyrics" => next_shortcuts.toggle_notch_lyrics = defaults.toggle_notch_lyrics,
        "switchLyrics" => next_shortcuts.switch_lyrics = defaults.switch_lyrics,
        _ => unreachable!("快捷键路径已在上方校验"),
    }
    crate::apply_global_shortcuts(&app, &previous.app.shortcuts, &next_shortcuts)?;
    let config = match state.config.reset_overrides(&[path]) {
        Ok(config) => config,
        Err(error) => {
            let _ = crate::apply_global_shortcuts(&app, &next_shortcuts, &previous.app.shortcuts);
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
pub fn get_ui_update_state(state: State<'_, AppState>) -> UiUpdateStateView {
    state.ui_update.state_view()
}

#[tauri::command]
pub async fn check_and_prepare_ui_update(
    state: State<'_, AppState>,
) -> Result<UiUpdateStateView, String> {
    state.ui_update.check_and_prepare().await
}

#[tauri::command]
pub fn apply_prepared_ui_update(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.ui_update.apply_prepared(&app)
}

#[tauri::command]
pub fn report_ui_ready(
    window: tauri::WebviewWindow,
    ui_version: String,
    state: State<'_, AppState>,
) -> Result<UiUpdateStateView, String> {
    state.ui_update.report_ready(window.label(), &ui_version)
}

#[tauri::command]
pub fn set_overlay_hide_when_not_playing(
    app: tauri::AppHandle,
    hidden: bool,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let config = state
        .config
        .update(|config| config.lyrics.displays.desktop.hide_when_not_playing = hidden)?;
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
