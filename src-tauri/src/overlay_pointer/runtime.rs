use crate::overlay_pointer::{
    list::start_list_unlock_handle_monitor, overlay::start_overlay_pointer_monitor,
};
use crate::{
    apply_dock_icon_hidden, register_global_shortcuts, setup_tray,
    sync_app_menu_bar_icon_visibility, AppState,
};
use tauri::Manager;

pub(crate) fn activate_runtime(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut started = state
        .runtime_started
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if *started {
        return Ok(());
    }

    let configured = state.config.snapshot();
    // 创建浮窗时需要 Accessory 资格；创建后恢复用户的 Dock 设置。
    #[cfg(target_os = "macos")]
    apply_dock_icon_hidden(app, true)?;
    setup_tray(app).map_err(|error| error.to_string())?;
    if let Err(error) = register_global_shortcuts(app, &configured.app.shortcuts) {
        log::warn!(
            "Failed to register global shortcuts at startup; runtime will continue: {error}"
        );
    }

    *started = true;
    crate::commands::start_library_scan(app);
    if let Err(error) = crate::reconcile_overlay_visibility(app) {
        log::warn!("Failed to reconcile overlay visibility at activation: {error}");
    }
    if let Err(error) = crate::reconcile_auxiliary_lyrics_windows(app) {
        log::warn!("Failed to restore auxiliary lyrics windows: {error}");
    }
    #[cfg(target_os = "macos")]
    apply_dock_icon_hidden(app, configured.app.hide_dock_icon)?;
    sync_app_menu_bar_icon_visibility(app)?;
    start_overlay_pointer_monitor(app.clone());
    start_list_unlock_handle_monitor(app.clone());
    crate::start_player_monitor(app.clone());
    crate::player_lifecycle::start_exit_monitor(app.clone());
    Ok(())
}
