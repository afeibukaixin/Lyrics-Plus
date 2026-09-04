use tauri::Manager;

use crate::AppState;
#[cfg(not(target_os = "macos"))]
use crate::TrayMenuState;

#[cfg(target_os = "macos")]
pub(super) fn apply_dock_icon_hidden(app: &tauri::AppHandle, hidden: bool) -> Result<(), String> {
    let main_window = app.get_webview_window("main");
    let main_was_visible = main_window
        .as_ref()
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    let main_was_focused = main_window
        .as_ref()
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false);

    app.set_activation_policy(if hidden {
        tauri::ActivationPolicy::Accessory
    } else {
        tauri::ActivationPolicy::Regular
    })
    .map_err(|error| format!("更新应用激活策略失败：{error}"))?;
    app.set_dock_visibility(!hidden)
        .map_err(|error| format!("更新 Dock 显示状态失败：{error}"))?;

    if let Some(window) = main_window {
        if main_was_visible {
            if let Err(error) = window.show() {
                log::warn!(
                    "Failed to restore main window visibility after updating Dock icon visibility: {error}"
                );
            }
        }
        if main_was_focused {
            if let Err(error) = window.set_focus() {
                log::warn!(
                    "Failed to restore main window focus after updating Dock icon visibility: {error}"
                );
            }
        }
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn apply_dock_icon_hidden(_app: &tauri::AppHandle, _hidden: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn apply_menu_bar_icon_hidden(
    app: &tauri::AppHandle,
    hidden: bool,
) -> Result<(), String> {
    crate::macos_status_item::sync_app_icon_visibility(app, !hidden)
        .map_err(|error| format!("更新菜单栏图标显示状态失败：{error}"))
}

#[cfg(not(target_os = "macos"))]
pub(super) fn apply_menu_bar_icon_hidden(
    app: &tauri::AppHandle,
    hidden: bool,
) -> Result<(), String> {
    if let Some(tray) = app.try_state::<TrayMenuState>() {
        tray.icon
            .set_visible(!hidden)
            .map_err(|error| format!("更新菜单栏图标显示状态失败：{error}"))?;
    }
    Ok(())
}

pub(super) fn sync_app_menu_bar_icon_visibility(app: &tauri::AppHandle) -> Result<(), String> {
    let hidden = app
        .try_state::<AppState>()
        .is_some_and(|state| state.config.snapshot().app.hide_menu_bar_icon);
    apply_menu_bar_icon_hidden(app, hidden)
}
