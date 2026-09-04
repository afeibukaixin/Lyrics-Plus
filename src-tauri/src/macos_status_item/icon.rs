use objc2_foundation::NSString;
use tauri::Manager;

use crate::TrayMenuState;

pub(super) fn sync_app_icon_visibility_inner(
    app: &tauri::AppHandle,
    visible: bool,
) -> tauri::Result<()> {
    let Some(tray_state) = app.try_state::<TrayMenuState>() else {
        return Ok(());
    };

    // Tauri 在 macOS 上通过移除 NSStatusItem 实现 set_visible(false)。固定应用图标只使用
    // AppKit 的可见性开关，确保最后一个 WebView 销毁后仍保留状态项及其菜单栏位置。
    if visible {
        tray_state.icon.set_visible(true)?;
    }
    let autosave_name = format!("{}.app-menu", app.config().identifier);
    tray_state.icon.with_inner_tray_icon(move |inner| {
        if let Some(status_item) = inner.ns_status_item() {
            status_item.setAutosaveName(Some(&NSString::from_str(&autosave_name)));
            status_item.setVisible(visible);
        }
    })?;
    Ok(())
}

pub(super) fn configure_lyrics_icon_identity_inner(app: &tauri::AppHandle) -> tauri::Result<()> {
    let Some(tray_state) = app.try_state::<TrayMenuState>() else {
        return Ok(());
    };
    let autosave_name = format!("{}.lyrics-status-item", app.config().identifier);
    tray_state.lyrics_icon.with_inner_tray_icon(move |inner| {
        if let Some(status_item) = inner.ns_status_item() {
            status_item.setAutosaveName(Some(&NSString::from_str(&autosave_name)));
        }
    })?;
    Ok(())
}
