use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::config::GlobalShortcutSettings;
use crate::{commands, AppState};

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

pub(super) fn register_global_shortcuts(
    app: &tauri::AppHandle,
    shortcuts: &GlobalShortcutSettings,
) -> Result<(), String> {
    let (
        [toggle, toggle_lock, reset],
        [toggle_status_bar, toggle_list, toggle_notch, switch_lyrics],
    ) = shortcuts.parsed()?;
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
            app.global_shortcut()
                .on_shortcut(toggle_notch, |app, _, event| {
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
                })
                .map_err(|error| {
                    format!(
                        "注册显示 / 隐藏灵动岛歌词快捷键 {} 失败：{error}",
                        shortcuts.toggle_notch_lyrics
                    )
                })?;
            registered.push(toggle_notch);
        }

        if let Some(switch_lyrics) = switch_lyrics {
            app.global_shortcut()
                .on_shortcut(switch_lyrics, |app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Err(error) = crate::toggle_quick_lyrics_window(app) {
                            log::warn!(
                                "Failed to toggle quick lyrics from global shortcut: {error}"
                            );
                        }
                    }
                })
                .map_err(|error| {
                    format!(
                        "注册切换歌词快捷键 {} 失败：{error}",
                        shortcuts.switch_lyrics
                    )
                })?;
            registered.push(switch_lyrics);
        }
        Ok(())
    })();

    if result.is_err() && !registered.is_empty() {
        let _ = app.global_shortcut().unregister_multiple(registered);
    }
    result
}

pub(super) fn apply_global_shortcuts(
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
    if let Err(error) = super::sync_tray_toggle_accelerator(app, next) {
        let _ = unregister_global_shortcuts(app, next);
        let rollback = register_global_shortcuts(app, previous);
        let _ = super::sync_tray_toggle_accelerator(app, previous);
        return Err(match rollback {
            Ok(()) => error,
            Err(rollback_error) => format!("{error}；恢复旧快捷键失败：{rollback_error}"),
        });
    }
    Ok(())
}
