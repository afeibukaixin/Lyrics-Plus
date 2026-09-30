use tauri::Manager;

use super::platform::LIST_FRAME_AUTOSAVE_NAME;
use crate::AppState;

/// 位置复位是用户主动操作；日常移动与重开由 AppKit 管理。
pub(crate) fn reset_list_lyrics_position(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state
        .storage
        .remove_preference(crate::window_placement::LIST_POSITION_KEY)?;
    state
        .storage
        .remove_preferences_with_prefix("lyrics-list.position.")?;
    state.storage.remove_preference("lyrics-list.last-monitor")?;
    state
        .storage
        .set_preference("lyrics-list.position-migrated", "1")?;
    super::clear_native_frame_autosave(app, LIST_FRAME_AUTOSAVE_NAME)
        .map_err(|error| error.to_string())?;
    if let Some(window) = app.get_webview_window("lyrics-list") {
        window.center().map_err(|error| error.to_string())?;
        super::reset_screen_affinity(app, &window);
    }
    Ok(())
}
