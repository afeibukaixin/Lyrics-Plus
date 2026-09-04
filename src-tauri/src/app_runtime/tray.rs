use tauri::Manager;

use crate::{AppState, TrayMenuState};

pub(super) fn sync_tray_overlay_checked(app: &tauri::AppHandle, visible: bool) {
    if let Some(tray) = app.try_state::<TrayMenuState>() {
        if let Err(error) = tray.toggle_overlay.set_checked(visible) {
            log::warn!("Failed to sync the tray overlay toggle state: {error}");
        }
    }
}

pub(super) fn sync_tray_lyrics_display_checked(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let displays = state.config.snapshot().lyrics.displays;
    if let Some(tray) = app.try_state::<TrayMenuState>() {
        for result in [
            tray.toggle_status_bar_lyrics
                .set_checked(displays.status_bar.enabled),
            tray.toggle_list_lyrics
                .set_checked(displays.list_window.enabled),
            tray.toggle_notch_lyrics.set_checked(displays.notch.enabled),
        ] {
            if let Err(error) = result {
                log::warn!("Failed to sync a lyrics display tray item: {error}");
            }
        }
    }
}

pub(super) fn sync_tray_toggle_accelerator(
    app: &tauri::AppHandle,
    shortcuts: &crate::config::GlobalShortcutSettings,
) -> Result<(), String> {
    if let Some(tray) = app.try_state::<TrayMenuState>() {
        for (index, (item, value)) in [
            (&tray.toggle_overlay, shortcuts.toggle_overlay.as_str()),
            (
                &tray.toggle_status_bar_lyrics,
                shortcuts.toggle_status_bar_lyrics.as_str(),
            ),
            (
                &tray.toggle_list_lyrics,
                shortcuts.toggle_list_lyrics.as_str(),
            ),
            (
                &tray.toggle_notch_lyrics,
                shortcuts.toggle_notch_lyrics.as_str(),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let accelerator =
                (!value.trim().is_empty()).then(|| value.replace("CommandOrControl", "CmdOrCtrl"));
            item.set_accelerator(accelerator.as_deref())
                .map_err(|error| format!("更新菜单栏快捷键失败：{error}"))?;
            #[cfg(target_os = "macos")]
            if accelerator.is_none() {
                clear_macos_tray_accelerator(&tray, index)?;
            }
        }
        let value = shortcuts.switch_lyrics.as_str();
        let accelerator =
            (!value.trim().is_empty()).then(|| value.replace("CommandOrControl", "CmdOrCtrl"));
        tray.switch_lyrics
            .set_accelerator(accelerator.as_deref())
            .map_err(|error| format!("更新菜单栏快捷键失败：{error}"))?;
        #[cfg(target_os = "macos")]
        if accelerator.is_none() {
            clear_macos_tray_accelerator(&tray, 4)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn clear_macos_tray_accelerator(tray: &TrayMenuState, item_index: usize) -> Result<(), String> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSEventModifierFlags;
    use objc2_foundation::NSString;

    tray.icon
        .with_inner_tray_icon(move |icon| {
            let mtm = MainThreadMarker::new().ok_or("菜单栏快捷键只能在主线程清除")?;
            let status_item = icon.ns_status_item().ok_or("无法访问菜单栏状态项")?;
            let menu = status_item.menu(mtm).ok_or("无法访问菜单栏菜单")?;
            let item = menu
                .itemArray()
                .to_vec()
                .into_iter()
                .nth(item_index)
                .ok_or("无法定位菜单栏快捷键项")?;

            // muda 0.19.3 在 accelerator 为 None 时未清空原生 NSMenuItem。
            item.setKeyEquivalent(&NSString::new());
            item.setKeyEquivalentModifierMask(NSEventModifierFlags::empty());
            Ok::<(), String>(())
        })
        .map_err(|error| error.to_string())?
}
