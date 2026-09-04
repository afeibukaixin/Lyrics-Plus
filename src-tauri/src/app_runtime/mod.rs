mod icons;
mod language;
mod shortcuts;
mod tray;

pub(crate) fn legal_notice_accepted(storage: &crate::storage::Storage) -> Result<bool, String> {
    Ok(storage
        .get_preference(crate::LEGAL_NOTICE_PREFERENCE)?
        .as_deref()
        .and_then(|value| value.parse::<u16>().ok())
        == Some(crate::LEGAL_NOTICE_VERSION))
}

pub(crate) fn apply_native_language(
    app: &tauri::AppHandle,
    ui_language: crate::language::UiLanguage,
) -> Result<(), String> {
    language::apply_native_language(app, ui_language)
}

pub(crate) fn sync_tray_overlay_checked(app: &tauri::AppHandle, visible: bool) {
    tray::sync_tray_overlay_checked(app, visible);
}

pub(crate) fn sync_tray_lyrics_display_checked(app: &tauri::AppHandle) {
    tray::sync_tray_lyrics_display_checked(app);
}

fn sync_tray_toggle_accelerator(
    app: &tauri::AppHandle,
    shortcuts: &crate::config::GlobalShortcutSettings,
) -> Result<(), String> {
    tray::sync_tray_toggle_accelerator(app, shortcuts)
}

pub(crate) fn apply_global_shortcuts(
    app: &tauri::AppHandle,
    previous: &crate::config::GlobalShortcutSettings,
    next: &crate::config::GlobalShortcutSettings,
) -> Result<(), String> {
    shortcuts::apply_global_shortcuts(app, previous, next)
}

pub(crate) fn register_global_shortcuts(
    app: &tauri::AppHandle,
    shortcuts: &crate::config::GlobalShortcutSettings,
) -> Result<(), String> {
    shortcuts::register_global_shortcuts(app, shortcuts)
}

pub(crate) fn apply_dock_icon_hidden(app: &tauri::AppHandle, hidden: bool) -> Result<(), String> {
    icons::apply_dock_icon_hidden(app, hidden)
}

pub(crate) fn apply_menu_bar_icon_hidden(
    app: &tauri::AppHandle,
    hidden: bool,
) -> Result<(), String> {
    icons::apply_menu_bar_icon_hidden(app, hidden)
}

pub(crate) fn sync_app_menu_bar_icon_visibility(app: &tauri::AppHandle) -> Result<(), String> {
    icons::sync_app_menu_bar_icon_visibility(app)
}
