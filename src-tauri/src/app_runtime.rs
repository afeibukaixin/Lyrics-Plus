pub(crate) fn legal_notice_accepted(storage: &storage::Storage) -> Result<bool, String> {
    Ok(storage
        .get_preference(LEGAL_NOTICE_PREFERENCE)?
        .as_deref()
        .and_then(|value| value.parse::<u16>().ok())
        == Some(LEGAL_NOTICE_VERSION))
}

pub(crate) fn apply_native_language(
    app: &tauri::AppHandle,
    language: UiLanguage,
) -> Result<(), String> {
    let labels = language.native_labels();
    if let Some(tray) = app.try_state::<TrayMenuState>() {
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

pub(crate) fn sync_tray_overlay_checked(app: &tauri::AppHandle, visible: bool) {
    if let Some(tray) = app.try_state::<TrayMenuState>() {
        if let Err(error) = tray.toggle_overlay.set_checked(visible) {
            log::warn!("Failed to sync the tray overlay toggle state: {error}");
        }
    }
}

fn sync_tray_lyrics_display_checked(app: &tauri::AppHandle) {
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

fn sync_tray_toggle_accelerator(
    app: &tauri::AppHandle,
    shortcuts: &GlobalShortcutSettings,
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
    }
    Ok(())
}

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

fn unregister_global_shortcuts(
    app: &tauri::AppHandle,
    shortcuts: &GlobalShortcutSettings,
) -> Result<(), String> {
    let (required, optional) = shortcuts.parsed()?;
    let parsed = required
        .into_iter()
        .chain(optional.into_iter().flatten())
        .collect::<Vec<_>>();
    app.global_shortcut()
        .unregister_multiple(parsed)
        .map_err(|error| format!("注销旧快捷键失败：{error}"))
}

fn register_global_shortcuts(
    app: &tauri::AppHandle,
    shortcuts: &GlobalShortcutSettings,
) -> Result<(), String> {
    let ([toggle, toggle_lock, reset], [toggle_status_bar, toggle_list, toggle_notch]) =
        shortcuts.parsed()?;
    let mut registered = Vec::<Shortcut>::new();

    let result = (|| {
        app.global_shortcut()
            .on_shortcut(toggle, |app, _, event| {
                log::debug!(
                    "Global shortcut event: action=toggle-overlay state={:?}",
                    event.state
                );
                if event.state == ShortcutState::Pressed {
                    let visible = app
                        .state::<AppState>()
                        .overlay_settings
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                        .visible;
                    if let Err(error) = commands::update_overlay_visible(app, !visible) {
                        log::warn!("Failed to toggle desktop lyrics from global shortcut: {error}");
                    }
                }
            })
            .map_err(|error| {
                format!(
                    "注册显示 / 隐藏桌面歌词快捷键 {} 失败：{error}",
                    shortcuts.toggle_overlay
                )
            })?;
        registered.push(toggle);

        app.global_shortcut()
            .on_shortcut(toggle_lock, |app, _, event| {
                log::debug!(
                    "Global shortcut event: action=toggle-overlay-lock state={:?}",
                    event.state
                );
                if event.state == ShortcutState::Pressed {
                    let locked = app
                        .state::<AppState>()
                        .overlay_settings
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                        .locked;
                    let next_locked = !locked;
                    match commands::update_overlay_locked(app, next_locked) {
                        Ok(()) => {
                            if !next_locked {
                                let _ = app.emit("overlay://unlock-feedback", ());
                            }
                        }
                        Err(error) => {
                            log::warn!(
                                "Failed to toggle desktop lyrics lock from global shortcut: {error}"
                            );
                        }
                    }
                }
            })
            .map_err(|error| {
                format!(
                    "注册锁定 / 解锁桌面歌词快捷键 {} 失败：{error}",
                    shortcuts.unlock_overlay
                )
            })?;
        registered.push(toggle_lock);

        app.global_shortcut()
            .on_shortcut(reset, |app, _, event| {
                log::debug!(
                    "Global shortcut event: action=reset-overlay state={:?}",
                    event.state
                );
                if event.state == ShortcutState::Pressed {
                    if let Err(error) = commands::reset_overlay_bounds(app.clone()) {
                        log::warn!("Failed to reset desktop lyrics from global shortcut: {error}");
                    }
                }
            })
            .map_err(|error| {
                format!(
                    "注册复位桌面歌词快捷键 {} 失败：{error}",
                    shortcuts.reset_overlay
                )
            })?;
        registered.push(reset);

        if let Some(toggle_status_bar) = toggle_status_bar {
            app.global_shortcut()
                .on_shortcut(toggle_status_bar, |app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        let enabled = app
                            .state::<AppState>()
                            .config
                            .snapshot()
                            .lyrics
                            .displays
                            .status_bar
                            .enabled;
                        if let Err(error) = commands::set_status_bar_lyrics_enabled(
                            app.clone(),
                            !enabled,
                            app.state::<AppState>(),
                        ) {
                            log::warn!(
                                "Failed to toggle menu bar lyrics from global shortcut: {error}"
                            );
                        }
                    }
                })
                .map_err(|error| {
                    format!(
                        "注册显示 / 隐藏菜单栏歌词快捷键 {} 失败：{error}",
                        shortcuts.toggle_status_bar_lyrics
                    )
                })?;
            registered.push(toggle_status_bar);
        }

        if let Some(toggle_list) = toggle_list {
            app.global_shortcut()
                .on_shortcut(toggle_list, |app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        let enabled = app
                            .state::<AppState>()
                            .config
                            .snapshot()
                            .lyrics
                            .displays
                            .list_window
                            .enabled;
                        if let Err(error) = commands::set_list_lyrics_visible(
                            app.clone(),
                            !enabled,
                            app.state::<AppState>(),
                        ) {
                            log::warn!(
                                "Failed to toggle the lyrics window from global shortcut: {error}"
                            );
                        }
                    }
                })
                .map_err(|error| {
                    format!(
                        "注册显示 / 隐藏歌词窗口快捷键 {} 失败：{error}",
                        shortcuts.toggle_list_lyrics
                    )
                })?;
            registered.push(toggle_list);
        }

        if let Some(toggle_notch) = toggle_notch {
            app.global_shortcut().on_shortcut(toggle_notch, |app, _, event| {
                if event.state == ShortcutState::Pressed {
                    let enabled = app
                        .state::<AppState>()
                        .config
                        .snapshot()
                        .lyrics
                        .displays
                        .notch
                        .enabled;
                    if let Err(error) = commands::set_notch_lyrics_visible(
                        app.clone(),
                        !enabled,
                        app.state::<AppState>(),
                    ) {
                        log::warn!("Failed to toggle Dynamic Island lyrics from global shortcut: {error}");
                    }
                }
            }).map_err(|error| {
                format!(
                    "注册显示 / 隐藏灵动岛歌词快捷键 {} 失败：{error}",
                    shortcuts.toggle_notch_lyrics
                )
            })?;
            registered.push(toggle_notch);
        }
        Ok(())
    })();

    if result.is_err() && !registered.is_empty() {
        let _ = app.global_shortcut().unregister_multiple(registered);
    }
    result
}

pub(crate) fn apply_global_shortcuts(
    app: &tauri::AppHandle,
    previous: &GlobalShortcutSettings,
    next: &GlobalShortcutSettings,
) -> Result<(), String> {
    next.parsed()?;
    if previous == next {
        return Ok(());
    }
    unregister_global_shortcuts(app, previous)?;
    if let Err(error) = register_global_shortcuts(app, next) {
        let rollback = register_global_shortcuts(app, previous);
        return Err(match rollback {
            Ok(()) => error,
            Err(rollback_error) => format!("{error}；恢复旧快捷键失败：{rollback_error}"),
        });
    }
    if let Err(error) = sync_tray_toggle_accelerator(app, next) {
        let _ = unregister_global_shortcuts(app, next);
        let rollback = register_global_shortcuts(app, previous);
        let _ = sync_tray_toggle_accelerator(app, previous);
        return Err(match rollback {
            Ok(()) => error,
            Err(rollback_error) => format!("{error}；恢复旧快捷键失败：{rollback_error}"),
        });
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn apply_dock_icon_hidden(app: &tauri::AppHandle, hidden: bool) -> Result<(), String> {
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
pub(crate) fn apply_dock_icon_hidden(_app: &tauri::AppHandle, _hidden: bool) -> Result<(), String> {
    Ok(())
}
