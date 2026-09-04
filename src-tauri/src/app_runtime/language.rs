use tauri::Manager;

pub(super) fn apply_native_language(
    app: &tauri::AppHandle,
    language: crate::language::UiLanguage,
) -> Result<(), String> {
    let labels = language.native_labels();
    if let Some(tray) = app.try_state::<crate::TrayMenuState>() {
        tray.toggle_overlay
            .set_text(labels.toggle_overlay)
            .map_err(|error| error.to_string())?;
        tray.toggle_status_bar_lyrics
            .set_text(labels.toggle_status_bar_lyrics)
            .map_err(|error| error.to_string())?;
        tray.toggle_list_lyrics
            .set_text(labels.toggle_list_lyrics)
            .map_err(|error| error.to_string())?;
        tray.toggle_notch_lyrics
            .set_text(labels.toggle_notch_lyrics)
            .map_err(|error| error.to_string())?;
        tray.switch_lyrics
            .set_text(labels.switch_lyrics)
            .map_err(|error| error.to_string())?;
        tray.settings
            .set_text(labels.settings)
            .map_err(|error| error.to_string())?;
        tray.quit
            .set_text(labels.quit)
            .map_err(|error| error.to_string())?;
    }
    for (label, title) in [
        ("quick-lyrics", labels.quick_title),
        ("lyrics-unlock-handle", labels.unlock_title),
        ("lyrics-list-unlock-handle", labels.unlock_title),
        ("lyrics-overlay", labels.overlay_title),
        ("lyrics-list", labels.list_title),
        ("lyrics-notch", labels.notch_title),
    ] {
        if let Some(window) = app.get_webview_window(label) {
            window.set_title(title).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}
