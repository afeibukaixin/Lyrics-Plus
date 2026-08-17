mod commands;
mod config;
mod language;
mod lyrics;
#[cfg(target_os = "macos")]
mod macos_status_item;
mod overlay_effect;
mod player;
mod player_lifecycle;
mod storage;

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use commands::{
    AppState, NotchLayoutMetrics, OverlayOrientation, OverlaySettings, OverlayStyleSettings,
};
use config::{ConfigStore, GlobalShortcutSettings};
use language::UiLanguage;
pub(crate) use overlay_effect::sync_overlay_vibrancy;
use overlay_effect::{HORIZONTAL_OVERLAY_SURFACE_INSET, VERTICAL_OVERLAY_SURFACE_INSET};
use player::{query_selected_player, PlayerSelection, SystemMediaService};
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

struct TrayMenuState {
    icon: TrayIcon<tauri::Wry>,
    #[cfg(target_os = "macos")]
    lyrics_icon: TrayIcon<tauri::Wry>,
    toggle_overlay: CheckMenuItem<tauri::Wry>,
    toggle_status_bar_lyrics: CheckMenuItem<tauri::Wry>,
    toggle_list_lyrics: CheckMenuItem<tauri::Wry>,
    toggle_notch_lyrics: CheckMenuItem<tauri::Wry>,
    switch_lyrics: MenuItem<tauri::Wry>,
    settings: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

pub(crate) const LEGAL_NOTICE_VERSION: u16 = 1;
pub(crate) const LEGAL_NOTICE_PREFERENCE: &str = "legal.notice.acceptedVersion";
const LIST_LYRICS_DEFAULT_WIDTH: f64 = 520.0;
const LIST_LYRICS_DEFAULT_HEIGHT: f64 = 720.0;
const NOTCH_VISIBILITY_TRANSITION_EVENT: &str = "notch://visibility-transition";
const NOTCH_EXIT_ANIMATION_DURATION: Duration = Duration::from_millis(400);

#[derive(Default)]
pub(crate) struct NotchVisibilityState {
    target_visible: bool,
    generation: u64,
}

#[derive(Clone, serde::Serialize)]
struct NotchVisibilityTransitionPayload {
    visible: bool,
}

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
                        "注册显示 / 隐藏状态栏歌词快捷键 {} 失败：{error}",
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
                                "Failed to toggle list lyrics from global shortcut: {error}"
                            );
                        }
                    }
                })
                .map_err(|error| {
                    format!(
                        "注册显示 / 隐藏列表歌词快捷键 {} 失败：{error}",
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

#[cfg(target_os = "macos")]
fn enable_joining_other_apps_fullscreen(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

    if MainThreadMarker::new().is_none() {
        return Err(std::io::Error::other(
            "macOS window collection behavior must be updated on the main thread",
        )
        .into());
    }
    let ns_window = window.ns_window()?;
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };
    let mut behavior = ns_window.collectionBehavior();
    behavior.remove(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Primary
            | NSWindowCollectionBehavior::Auxiliary
            | NSWindowCollectionBehavior::FullScreenPrimary
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::FullScreenNone,
    );
    behavior.insert(NSWindowCollectionBehavior::CanJoinAllApplications);
    ns_window.setCollectionBehavior(behavior);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn enable_joining_other_apps_fullscreen(_window: &tauri::WebviewWindow) -> tauri::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn enable_notch_window_behavior(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2_app_kit::{NSStatusWindowLevel, NSWindow};

    enable_joining_other_apps_fullscreen(window)?;
    let ns_window = window.ns_window()?;
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };
    ns_window.setLevel(NSStatusWindowLevel);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn enable_notch_window_behavior(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    enable_joining_other_apps_fullscreen(window)
}

#[cfg(target_os = "macos")]
fn refresh_macos_mouse_tracking(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2_app_kit::{NSView, NSWindow};

    fn update_tracking_areas(view: &NSView) {
        view.updateTrackingAreas();
        for child in view.subviews().iter() {
            update_tracking_areas(&child);
        }
    }

    let ns_window = window.ns_window()?;
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };
    ns_window.setAcceptsMouseMovedEvents(true);
    ns_window.resetCursorRects();

    let ns_view = window.ns_view()?;
    let ns_view = unsafe { &*ns_view.cast::<NSView>() };
    update_tracking_areas(ns_view);
    Ok(())
}

pub(crate) fn refresh_overlay_mouse_tracking(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        let target = window.clone();
        if let Err(error) = window.run_on_main_thread(move || {
            if let Err(error) = refresh_macos_mouse_tracking(&target) {
                log::warn!("Failed to refresh overlay mouse tracking: {error}");
            }
        }) {
            log::warn!("Failed to schedule the overlay mouse tracking refresh: {error}");
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub(crate) fn create_overlay(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("lyrics-overlay").is_some() {
        return Ok(());
    }

    let style = app
        .try_state::<AppState>()
        .map(|state| {
            state
                .overlay_style
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
        })
        .unwrap_or_default();
    let (initial_width, initial_height) = initial_overlay_dimensions(&style);

    let window = WebviewWindowBuilder::new(
        app,
        "lyrics-overlay",
        WebviewUrl::App("index.html?view=overlay".into()),
    )
    .title(UiLanguage::ZhCn.native_labels().overlay_title)
    .inner_size(initial_width, initial_height)
    .min_inner_size(190.0, 76.0)
    .transparent(true)
    .accept_first_mouse(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .build()?;

    enable_joining_other_apps_fullscreen(&window)?;
    refresh_overlay_mouse_tracking(&window);
    sync_overlay_vibrancy(&window, &style);

    Ok(())
}

pub(crate) fn show_quick_lyrics_window(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("quick-lyrics") {
        if let Err(error) = window.set_size(tauri::LogicalSize::new(900.0, 620.0)) {
            log::warn!("Failed to restore the quick lyrics window size: {error}");
        }
        if let Err(error) = window.set_resizable(false) {
            log::warn!("Failed to disable resizing for the quick lyrics window: {error}");
        }
        if let Err(error) = window.unminimize() {
            log::warn!("Failed to unminimize the quick lyrics window: {error}");
        }
        window.show().map_err(|error| error.to_string())?;
        return window.set_focus().map_err(|error| error.to_string());
    }

    let window = WebviewWindowBuilder::new(
        app,
        "quick-lyrics",
        WebviewUrl::App("index.html?view=quick-lyrics".into()),
    )
    .title(UiLanguage::ZhCn.native_labels().quick_title)
    .inner_size(900.0, 620.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(true)
    .center()
    .build()
    .map_err(|error| error.to_string())?;

    window.set_focus().map_err(|error| error.to_string())
}

fn create_list_lyrics_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("lyrics-list").is_some() {
        return Ok(());
    }
    let always_on_top = app
        .state::<AppState>()
        .config
        .snapshot()
        .lyrics
        .displays
        .list_window
        .always_on_top;
    WebviewWindowBuilder::new(
        app,
        "lyrics-list",
        WebviewUrl::App("index.html?view=lyrics-list".into()),
    )
    .title(UiLanguage::ZhCn.native_labels().list_title)
    .inner_size(LIST_LYRICS_DEFAULT_WIDTH, LIST_LYRICS_DEFAULT_HEIGHT)
    .min_inner_size(360.0, 480.0)
    .transparent(true)
    .accept_first_mouse(true)
    .decorations(false)
    .shadow(false)
    .resizable(true)
    .maximizable(false)
    .minimizable(true)
    .always_on_top(always_on_top)
    .visible(false)
    .center()
    .build()?;
    Ok(())
}

pub(crate) fn reset_list_lyrics_window_size(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("lyrics-list")
        .ok_or_else(|| "列表歌词窗口不存在".to_string())?;
    window
        .set_size(tauri::LogicalSize::new(
            LIST_LYRICS_DEFAULT_WIDTH,
            LIST_LYRICS_DEFAULT_HEIGHT,
        ))
        .map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn restore_status_bar_position(app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };
    let monitor_id = state
        .storage
        .get_preference("lyrics-status-bar.last-monitor")
        .ok()
        .flatten()
        .or_else(|| {
            app.primary_monitor()
                .ok()
                .flatten()
                .map(|monitor| notch_monitor_id(&monitor))
        });
    let raw = monitor_id
        .as_deref()
        .and_then(|id| {
            state
                .storage
                .get_preference(&format!("lyrics-status-bar.position.{id}"))
                .ok()
                .flatten()
        })
        .or_else(|| {
            state
                .storage
                .get_preference("lyrics-status-bar.position")
                .ok()
                .flatten()
        });
    let Some(raw) = raw else {
        return false;
    };
    let Some((x, y)) = raw.split_once(',') else {
        return false;
    };
    let (Ok(x), Ok(y)) = (x.parse::<i32>(), y.parse::<i32>()) else {
        return false;
    };
    let position = tauri::PhysicalPosition::new(x, y);
    let visible = app.available_monitors().ok().is_some_and(|monitors| {
        monitors.iter().any(|monitor| {
            let origin = monitor.position();
            let size = monitor.size();
            position.x >= origin.x
                && position.y >= origin.y
                && position.x < origin.x.saturating_add(size.width as i32)
                && position.y < origin.y.saturating_add(size.height as i32)
        })
    });
    visible && window.set_position(position).is_ok()
}

#[cfg(not(target_os = "macos"))]
fn position_status_bar_window_default(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let Some(monitor) = app.primary_monitor().ok().flatten() else {
        return;
    };
    let scale = monitor.scale_factor().max(1.0);
    let size = window
        .outer_size()
        .unwrap_or(tauri::PhysicalSize::new(360, 36));
    let right_gap = (96.0 * scale).round() as i32;
    let top_gap = (3.0 * scale).round() as i32;
    let x = monitor.position().x + monitor.size().width as i32 - size.width as i32 - right_gap;
    let y = monitor.position().y + top_gap;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

#[cfg(not(target_os = "macos"))]
fn create_status_bar_lyrics_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("lyrics-status-bar").is_some() {
        return Ok(());
    }
    let config = app.state::<AppState>().config.snapshot();
    let appearance = &config.lyrics.displays.status_bar.appearance;
    let height = appearance.font_size as f64 + 12.0;
    let window = WebviewWindowBuilder::new(
        app,
        "lyrics-status-bar",
        WebviewUrl::App("index.html?view=lyrics-status-bar".into()),
    )
    .title("Lyrics Plus 状态栏歌词")
    .inner_size(appearance.width as f64, height.max(26.0))
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .focusable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .build()?;
    enable_joining_other_apps_fullscreen(&window)?;
    window.set_ignore_cursor_events(true)?;
    if !restore_status_bar_position(app, &window) {
        position_status_bar_window_default(app, &window);
    }
    Ok(())
}

pub(crate) fn position_auxiliary_lyrics_window_default(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    label: &str,
) -> Result<(), String> {
    match label {
        "lyrics-status-bar" => {
            #[cfg(not(target_os = "macos"))]
            position_status_bar_window_default(app, window);
        }
        "lyrics-notch" => schedule_notch_position(app, window),
        "lyrics-list" => {
            let _ = window.center();
        }
        _ => return Err("未知歌词窗口".into()),
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn screen_notch_layout(monitor: &tauri::Monitor) -> NotchLayoutMetrics {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;

    let Some(marker) = MainThreadMarker::new() else {
        return NotchLayoutMetrics::default();
    };
    let monitor_name = monitor.name().map(String::as_str);
    let screens = NSScreen::screens(marker);
    let metrics_for = |screen: &NSScreen| {
        let top_inset = screen.safeAreaInsets().top.max(0.0);
        let left_area = screen.auxiliaryTopLeftArea();
        let right_area = screen.auxiliaryTopRightArea();
        let left_edge = left_area.origin.x + left_area.size.width;
        let center_gap_width = (right_area.origin.x - left_edge).max(0.0);
        let has_notch = top_inset > 0.0 && center_gap_width > 0.0;

        NotchLayoutMetrics {
            has_notch,
            top_inset: if has_notch { top_inset } else { 0.0 },
            center_gap_width: if has_notch { center_gap_width } else { 0.0 },
        }
    };

    if let Some(screen) = screens
        .iter()
        .find(|screen| monitor_name.is_some_and(|name| screen.localizedName().to_string() == name))
    {
        return metrics_for(&screen);
    }
    NSScreen::mainScreen(marker)
        .as_deref()
        .map(metrics_for)
        .unwrap_or_default()
}

#[cfg(not(target_os = "macos"))]
fn screen_notch_layout(_monitor: &tauri::Monitor) -> NotchLayoutMetrics {
    NotchLayoutMetrics::default()
}

fn preferred_notch_monitor(app: &tauri::AppHandle) -> Option<tauri::Monitor> {
    let preferred = app
        .try_state::<AppState>()
        .and_then(|state| state.config.snapshot().lyrics.displays.notch.monitor_id);
    let monitors = app.available_monitors().ok()?;
    preferred
        .as_deref()
        .and_then(|id| {
            monitors
                .iter()
                .find(|monitor| notch_monitor_id(monitor) == id)
                .cloned()
        })
        .or_else(|| app.primary_monitor().ok().flatten())
        .or_else(|| monitors.into_iter().next())
}

pub(crate) fn notch_monitor_id(monitor: &tauri::Monitor) -> String {
    let position = monitor.position();
    let size = monitor.size();
    format!(
        "{}@{},{}:{}x{}",
        monitor.name().map(String::as_str).unwrap_or("display"),
        position.x,
        position.y,
        size.width,
        size.height
    )
}

fn position_notch_window(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let Some(monitor) = preferred_notch_monitor(app) else {
        return;
    };
    let resolved_monitor_id = notch_monitor_id(&monitor);
    if let Some(state) = app.try_state::<AppState>() {
        let current = state.config.snapshot().lyrics.displays.notch.monitor_id;
        if current.as_deref() != Some(resolved_monitor_id.as_str()) {
            if let Ok(config) = state.config.update(|config| {
                config.lyrics.displays.notch.monitor_id = Some(resolved_monitor_id.clone());
            }) {
                let _ = app.emit("config://changed", &config);
            }
        }
    }
    let width = window.outer_size().map(|size| size.width).unwrap_or(420);
    let monitor_width = monitor.size().width;
    let x = monitor.position().x + monitor_width.saturating_sub(width) as i32 / 2;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, monitor.position().y));
    let metrics = screen_notch_layout(&monitor);
    if let Some(state) = app.try_state::<AppState>() {
        *state
            .notch_layout_metrics
            .write()
            .unwrap_or_else(|error| error.into_inner()) = metrics.clone();
    }
    let _ = window.emit("notch://layout", &metrics);
}

fn schedule_notch_position(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let target = window.clone();
    let handle = app.clone();
    if let Err(error) = window.run_on_main_thread(move || position_notch_window(&handle, &target)) {
        log::warn!("Failed to schedule Dynamic Island lyrics positioning: {error}");
    }
}

fn create_notch_lyrics_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("lyrics-notch").is_some() {
        return Ok(());
    }
    let width = app
        .state::<AppState>()
        .config
        .snapshot()
        .lyrics
        .displays
        .notch
        .appearance
        .max_width as f64
        + 16.0;
    let window = WebviewWindowBuilder::new(
        app,
        "lyrics-notch",
        WebviewUrl::App("index.html?view=lyrics-notch".into()),
    )
    .title(UiLanguage::ZhCn.native_labels().notch_title)
    .inner_size(width, 124.0)
    .transparent(true)
    .accept_first_mouse(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .focusable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .build()?;
    enable_notch_window_behavior(&window)?;
    refresh_overlay_mouse_tracking(&window);
    schedule_notch_position(app, &window);
    Ok(())
}

// 该函数会创建窗口并调用 AppKit，只能由主线程入口调用。
fn reconcile_auxiliary_lyrics_windows(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let displays = state.config.snapshot().lyrics.displays;
    let playback = state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    #[cfg(not(target_os = "macos"))]
    {
        let show_status_bar = displays.status_bar.enabled
            && (!displays.status_bar.hide_when_not_playing || playback.is_playing);
        if show_status_bar {
            create_status_bar_lyrics_window(app).map_err(|error| error.to_string())?;
            if let Some(window) = app.get_webview_window("lyrics-status-bar") {
                let appearance = &displays.status_bar.appearance;
                let height = appearance.font_size as f64 + 12.0;
                let _ = window.set_size(tauri::LogicalSize::new(
                    appearance.width as f64,
                    height.max(26.0),
                ));
                window.show().map_err(|error| error.to_string())?;
            }
        } else if let Some(window) = app.get_webview_window("lyrics-status-bar") {
            window.hide().map_err(|error| error.to_string())?;
        }
    }
    if displays.list_window.enabled {
        create_list_lyrics_window(app).map_err(|error| error.to_string())?;
        if let Some(window) = app.get_webview_window("lyrics-list") {
            window
                .set_always_on_top(displays.list_window.always_on_top)
                .map_err(|error| error.to_string())?;
            if !window.is_visible().unwrap_or(false) {
                window.show().map_err(|error| error.to_string())?;
            }
        }
    } else if let Some(window) = app.get_webview_window("lyrics-list") {
        window.hide().map_err(|error| error.to_string())?;
    }

    let has_track = playback
        .title
        .as_deref()
        .is_some_and(|title| !title.trim().is_empty());
    let show_notch = displays.notch.enabled
        && has_track
        && (!displays.notch.hide_when_not_playing || playback.is_playing);
    let (visibility_changed, visibility_generation) = {
        let mut visibility = state
            .notch_visibility
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if visibility.target_visible != show_notch {
            visibility.target_visible = show_notch;
            visibility.generation = visibility.generation.wrapping_add(1);
            (true, visibility.generation)
        } else {
            (false, visibility.generation)
        }
    };
    if show_notch {
        create_notch_lyrics_window(app).map_err(|error| error.to_string())?;
        if let Some(window) = app.get_webview_window("lyrics-notch") {
            let was_visible = window.is_visible().unwrap_or(false);
            if !was_visible {
                window.show().map_err(|error| error.to_string())?;
            }
            if visibility_changed || !was_visible {
                let _ = window.emit(
                    NOTCH_VISIBILITY_TRANSITION_EVENT,
                    NotchVisibilityTransitionPayload { visible: true },
                );
            }
            schedule_notch_position(app, &window);
        }
    } else if visibility_changed {
        if let Some(window) = app.get_webview_window("lyrics-notch") {
            if window.is_visible().unwrap_or(false) {
                let _ = window.emit(
                    NOTCH_VISIBILITY_TRANSITION_EVENT,
                    NotchVisibilityTransitionPayload { visible: false },
                );
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(NOTCH_EXIT_ANIMATION_DURATION).await;
                    let state = handle.state::<AppState>();
                    let transition_is_current = {
                        let visibility = state
                            .notch_visibility
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        !visibility.target_visible
                            && visibility.generation == visibility_generation
                    };
                    if !transition_is_current {
                        return;
                    }

                    let displays = state.config.snapshot().lyrics.displays;
                    let playback = state
                        .last_snapshot
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                        .clone();
                    let has_track = playback
                        .title
                        .as_deref()
                        .is_some_and(|title| !title.trim().is_empty());
                    let should_still_hide = !displays.notch.enabled
                        || !has_track
                        || (displays.notch.hide_when_not_playing && !playback.is_playing);
                    if should_still_hide {
                        if let Some(window) = handle.get_webview_window("lyrics-notch") {
                            let _ = window.hide();
                        }
                    }
                });
            }
        }
    }
    sync_tray_lyrics_display_checked(app);
    Ok(())
}

fn sync_lyrics_surfaces_on_main(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    macos_status_item::sync(app);
    #[cfg(not(target_os = "macos"))]
    if let Some(tray) = app.try_state::<TrayMenuState>() {
        if let Err(error) = tray.icon.set_title(None::<&str>) {
            log::warn!("Failed to update menu bar lyrics: {error}");
        }
    }
    if let Err(error) = reconcile_auxiliary_lyrics_windows(app) {
        log::warn!("Failed to reconcile auxiliary lyrics windows: {error}");
    }
}

pub(crate) fn sync_lyrics_surfaces(app: &tauri::AppHandle) {
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || sync_lyrics_surfaces_on_main(&handle)) {
        log::warn!("Failed to schedule lyrics surface synchronization: {error}");
    }
}

fn initial_overlay_dimensions(style: &OverlayStyleSettings) -> (f64, f64) {
    match style.orientation {
        OverlayOrientation::Horizontal => (style.horizontal_max_width.unwrap_or(760.0), 156.0),
        OverlayOrientation::Vertical => (190.0, style.vertical_max_height.unwrap_or(620.0)),
    }
}

fn create_unlock_handle(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.get_webview_window("lyrics-unlock-handle").is_some() {
        return Ok(());
    }
    let builder = WebviewWindowBuilder::new(
        app,
        "lyrics-unlock-handle",
        WebviewUrl::App("index.html?view=unlock-handle".into()),
    )
    .title(UiLanguage::ZhCn.native_labels().unlock_title)
    .inner_size(28.0, 28.0)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    // 这是覆盖在歌词浮窗上的点击入口，不应成为 macOS 的键盘焦点窗口。
    // 否则歌词换行触发位置同步时，可能抢走当前应用的焦点。
    .focusable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false);

    #[cfg(target_os = "macos")]
    let builder = {
        let overlay = app
            .get_webview_window("lyrics-overlay")
            .ok_or(tauri::Error::WindowNotFound)?;
        builder.parent(&overlay)?
    };

    builder.build()?;

    Ok(())
}

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    if app.try_state::<TrayMenuState>().is_some() {
        return Ok(());
    }
    let labels = UiLanguage::ZhCn.native_labels();
    let overlay_visible = app
        .state::<AppState>()
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .visible;
    let display_config = app.state::<AppState>().config.snapshot().lyrics.displays;
    let shortcuts = app.state::<AppState>().config.snapshot().app.shortcuts;
    let accelerator = |value: &str| {
        (!value.trim().is_empty()).then(|| value.replace("CommandOrControl", "CmdOrCtrl"))
    };
    let toggle_accelerator = accelerator(&shortcuts.toggle_overlay);
    let status_bar_accelerator = accelerator(&shortcuts.toggle_status_bar_lyrics);
    let list_accelerator = accelerator(&shortcuts.toggle_list_lyrics);
    let notch_accelerator = accelerator(&shortcuts.toggle_notch_lyrics);
    let toggle_overlay = CheckMenuItem::with_id(
        app,
        "toggle-overlay",
        labels.toggle_overlay,
        true,
        overlay_visible,
        toggle_accelerator.as_deref(),
    )?;
    let toggle_status_bar_lyrics = CheckMenuItem::with_id(
        app,
        "toggle-status-bar-lyrics",
        labels.toggle_status_bar_lyrics,
        true,
        display_config.status_bar.enabled,
        status_bar_accelerator.as_deref(),
    )?;
    let toggle_list_lyrics = CheckMenuItem::with_id(
        app,
        "toggle-list-lyrics",
        labels.toggle_list_lyrics,
        true,
        display_config.list_window.enabled,
        list_accelerator.as_deref(),
    )?;
    let toggle_notch_lyrics = CheckMenuItem::with_id(
        app,
        "toggle-notch-lyrics",
        labels.toggle_notch_lyrics,
        true,
        display_config.notch.enabled,
        notch_accelerator.as_deref(),
    )?;
    let switch_lyrics = MenuItem::with_id(
        app,
        "switch-lyrics",
        labels.switch_lyrics,
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, "settings", labels.settings, true, Some("CmdOrCtrl+,"))?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle_overlay,
            &toggle_status_bar_lyrics,
            &toggle_list_lyrics,
            &toggle_notch_lyrics,
            &switch_lyrics,
            &settings,
            &quit,
        ],
    )?;

    #[cfg(target_os = "macos")]
    let tray_icon = {
        let rgba = image::load_from_memory(include_bytes!("../icons/tray-icon.png"))
            .expect("invalid embedded tray icon")
            .into_rgba8();
        let (width, height) = rgba.dimensions();
        tauri::image::Image::new_owned(rgba.into_raw(), width, height)
    };
    #[cfg(not(target_os = "macos"))]
    let tray_icon = app
        .default_window_icon()
        .cloned()
        .expect("missing application icon");

    let tray_builder = TrayIconBuilder::new().icon(tray_icon);
    #[cfg(target_os = "macos")]
    let tray_builder = tray_builder.icon_as_template(true);

    let icon = tray_builder
        .tooltip("Lyrics Plus")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle-overlay" => {
                let visible = app
                    .state::<AppState>()
                    .overlay_settings
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .visible;
                let _ = commands::update_overlay_visible(app, !visible);
            }
            "toggle-status-bar-lyrics" => {
                let enabled = app
                    .state::<AppState>()
                    .config
                    .snapshot()
                    .lyrics
                    .displays
                    .status_bar
                    .enabled;
                let _ = commands::set_status_bar_lyrics_enabled(
                    app.clone(),
                    !enabled,
                    app.state::<AppState>(),
                );
            }
            "toggle-list-lyrics" => {
                let enabled = app
                    .state::<AppState>()
                    .config
                    .snapshot()
                    .lyrics
                    .displays
                    .list_window
                    .enabled;
                let _ = commands::set_list_lyrics_visible(
                    app.clone(),
                    !enabled,
                    app.state::<AppState>(),
                );
            }
            "toggle-notch-lyrics" => {
                let enabled = app
                    .state::<AppState>()
                    .config
                    .snapshot()
                    .lyrics
                    .displays
                    .notch
                    .enabled;
                let _ = commands::set_notch_lyrics_visible(
                    app.clone(),
                    !enabled,
                    app.state::<AppState>(),
                );
            }
            "switch-lyrics" => {
                if let Err(error) = show_quick_lyrics_window(app) {
                    log::warn!("Failed to open quick lyrics from the tray: {error}");
                }
            }
            "settings" => {
                if let Err(error) = show_main_window_centered(app) {
                    log::warn!("Failed to open settings from the tray: {error}");
                }
            }
            "quit" => {
                log::info!("Application exit requested: reason=tray_quit");
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    #[cfg(target_os = "macos")]
    let lyrics_icon = TrayIconBuilder::with_id("lyrics-status-item")
        .title("Lyrics Plus")
        .tooltip("Lyrics Plus")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .build(app)?;
    #[cfg(target_os = "macos")]
    lyrics_icon.with_inner_tray_icon(|inner| {
        if let Some(status_item) = inner.ns_status_item() {
            status_item.setVisible(false);
        }
    })?;

    app.manage(TrayMenuState {
        icon,
        #[cfg(target_os = "macos")]
        lyrics_icon,
        toggle_overlay: toggle_overlay.clone(),
        toggle_status_bar_lyrics: toggle_status_bar_lyrics.clone(),
        toggle_list_lyrics: toggle_list_lyrics.clone(),
        toggle_notch_lyrics: toggle_notch_lyrics.clone(),
        switch_lyrics: switch_lyrics.clone(),
        settings: settings.clone(),
        quit: quit.clone(),
    });
    sync_lyrics_surfaces(app);
    #[cfg(target_os = "macos")]
    macos_status_item::start(app.clone());

    Ok(())
}

fn should_show_overlay(visible: bool, hide_when_not_playing: bool, is_playing: bool) -> bool {
    visible && (!hide_when_not_playing || is_playing)
}

pub(crate) fn reconcile_overlay_visibility(app: &tauri::AppHandle) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let configured = state.config.snapshot();
    let is_playing = state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .is_playing;
    let should_show = should_show_overlay(
        configured.overlay.visible,
        configured.overlay.hide_when_not_playing,
        is_playing,
    );
    let window = app
        .get_webview_window("lyrics-overlay")
        .ok_or_else(|| "歌词浮窗不存在".to_string())?;
    let is_visible = window.is_visible().unwrap_or(false);
    if should_show != is_visible {
        if should_show {
            restore_overlay_position(app, &window);
            window.show()
        } else {
            window.hide()
        }
        .map_err(|error| error.to_string())?;
    }
    sync_unlock_handle(app);
    Ok(should_show)
}

fn start_player_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let selection = app
                .try_state::<AppState>()
                .map(|state| *state.selection.read().unwrap_or_else(|e| e.into_inner()))
                .unwrap_or(PlayerSelection::Auto);
            let previous_auto_player = app
                .try_state::<AppState>()
                .map(|state| {
                    *state
                        .auto_player
                        .read()
                        .unwrap_or_else(|error| error.into_inner())
                })
                .unwrap_or(None);
            let system_media = app
                .try_state::<AppState>()
                .map(|state| state.system_media.clone())
                .unwrap_or_else(|| Arc::new(SystemMediaService::default()));
            let (system_media_filter_mode, system_media_applications) = app
                .try_state::<AppState>()
                .map(|state| {
                    let config = state.config.snapshot();
                    (
                        config.app.system_media_filter_mode,
                        config.app.system_media_applications,
                    )
                })
                .unwrap_or_default();

            let (snapshot, next_auto_player) = tauri::async_runtime::spawn_blocking(move || {
                query_selected_player(
                    selection,
                    previous_auto_player,
                    &system_media,
                    system_media_filter_mode,
                    &system_media_applications,
                )
            })
            .await
            .unwrap_or_else(|error| {
                (
                    player::PlaybackSnapshot::unavailable(
                        selection.preferred_kind(),
                        format!("播放器读取任务失败：{error}"),
                    ),
                    previous_auto_player,
                )
            });

            if let Some(state) = app.try_state::<AppState>() {
                *state
                    .last_snapshot
                    .write()
                    .unwrap_or_else(|e| e.into_inner()) = snapshot.clone();
                *state.auto_player.write().unwrap_or_else(|e| e.into_inner()) = next_auto_player;
            }
            let _ = app.emit("playback://snapshot", &snapshot);
            commands::sync_lyrics_runtime(&app, &snapshot);
            if let Err(error) = reconcile_overlay_visibility(&app) {
                log::warn!("Failed to reconcile overlay visibility with playback state: {error}");
            }
            if let Some(window) = app.get_webview_window("lyrics-overlay") {
                if window.is_visible().unwrap_or(false) {
                    reconcile_overlay_placement(&app, &window);
                }
            }
            let any_window_visible = app
                .try_state::<AppState>()
                .is_some_and(|state| state.config.snapshot().lyrics.displays.status_bar.enabled)
                || [
                    "main",
                    "lyrics-overlay",
                    "lyrics-list",
                    "lyrics-notch",
                ]
                .iter()
                .any(|label| {
                    app.get_webview_window(label)
                        .and_then(|window| window.is_visible().ok())
                        .unwrap_or(false)
                });
            tokio::time::sleep(Duration::from_millis(if any_window_visible {
                750
            } else {
                2_000
            }))
            .await;
        }
    });
}

pub(crate) fn monitor_id(monitor: &tauri::Monitor) -> String {
    monitor.name().cloned().unwrap_or_else(|| {
        let position = monitor.position();
        let size = monitor.size();
        format!(
            "{}x{}-{}x{}",
            position.x, position.y, size.width, size.height
        )
    })
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarPlacement {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl ToolbarPlacement {
    fn for_orientation(orientation: OverlayOrientation) -> Self {
        match orientation {
            OverlayOrientation::Horizontal => Self::Top,
            OverlayOrientation::Vertical => Self::Right,
        }
    }

    fn normalized(self, orientation: OverlayOrientation) -> Self {
        match (orientation, self) {
            (OverlayOrientation::Horizontal, Self::Top | Self::Bottom)
            | (OverlayOrientation::Vertical, Self::Left | Self::Right) => self,
            _ => Self::for_orientation(orientation),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredBounds {
    x: i32,
    y: i32,
    #[serde(default)]
    work_x: Option<i32>,
    #[serde(default)]
    work_y: Option<i32>,
    #[serde(default)]
    work_width: Option<u32>,
    #[serde(default)]
    work_height: Option<u32>,
    #[serde(default)]
    scale_factor: Option<f64>,
    #[serde(default)]
    relative_x: Option<f64>,
    #[serde(default)]
    relative_y: Option<f64>,
    #[serde(default)]
    toolbar_placement: Option<ToolbarPlacement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MonitorTopologyEntry {
    id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    work_x: i32,
    work_y: i32,
    work_width: u32,
    work_height: u32,
    scale_factor_bits: u64,
}

const PROGRAMMATIC_MOVE_SUPPRESSION: Duration = Duration::from_secs(2);

#[derive(Default)]
pub(crate) struct OverlayPlacementState {
    preferred_monitor: Option<String>,
    topology: Vec<MonitorTopologyEntry>,
    pub(crate) toolbar_placement: ToolbarPlacement,
    expected_programmatic_position: Option<tauri::PhysicalPosition<i32>>,
    programmatic_move_started_at: Option<Instant>,
}

impl OverlayPlacementState {
    fn update_topology(&mut self, next: Vec<MonitorTopologyEntry>) -> bool {
        if self.topology.is_empty() {
            self.topology = next;
            return false;
        }
        if self.topology == next {
            return false;
        }
        self.topology = next;
        self.expected_programmatic_position = None;
        self.programmatic_move_started_at = None;
        true
    }

    fn consume_programmatic_move(&mut self, position: tauri::PhysicalPosition<i32>) -> bool {
        let expected = self.expected_programmatic_position.take();
        self.programmatic_move_started_at = None;
        let Some(expected) = expected else {
            return false;
        };
        expected.x.abs_diff(position.x) <= 2 && expected.y.abs_diff(position.y) <= 2
    }

    fn suppress_persistence(&mut self, now: Instant) -> bool {
        let active = self.programmatic_move_started_at.is_some_and(|started| {
            now.saturating_duration_since(started) <= PROGRAMMATIC_MOVE_SUPPRESSION
        });
        if !active {
            self.expected_programmatic_position = None;
            self.programmatic_move_started_at = None;
        }
        active
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct StoredOverlayGeometry {
    pub horizontal_max_width: Option<f64>,
    pub vertical_max_height: Option<f64>,
}

fn overlay_geometry(storage: &storage::Storage, monitor_id: Option<&str>) -> StoredOverlayGeometry {
    let geometry_key = monitor_id
        .map(|id| format!("overlay.geometry.{id}"))
        .unwrap_or_else(|| "overlay.geometry.default".into());
    if let Ok(Some(raw)) = storage.get_preference(&geometry_key) {
        if let Ok(geometry) = serde_json::from_str(&raw) {
            return geometry;
        }
    }
    let legacy_key = monitor_id
        .map(|id| format!("overlay.style.{id}"))
        .unwrap_or_else(|| "overlay.style.default".into());
    storage
        .get_preference(&legacy_key)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<OverlayStyleSettings>(&raw).ok())
        .map(|style| StoredOverlayGeometry {
            horizontal_max_width: style.horizontal_max_width,
            vertical_max_height: style.vertical_max_height,
        })
        .unwrap_or_default()
}

fn monitor_topology(monitors: &[tauri::Monitor]) -> Vec<MonitorTopologyEntry> {
    let mut topology = monitors
        .iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            let work_area = monitor.work_area();
            MonitorTopologyEntry {
                id: monitor_id(monitor),
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
                work_x: work_area.position.x,
                work_y: work_area.position.y,
                work_width: work_area.size.width,
                work_height: work_area.size.height,
                scale_factor_bits: monitor.scale_factor().to_bits(),
            }
        })
        .collect::<Vec<_>>();
    topology.sort_by(|left, right| {
        (&left.id, left.x, left.y, left.width, left.height).cmp(&(
            &right.id,
            right.x,
            right.y,
            right.width,
            right.height,
        ))
    });
    topology
}

fn centered_position(
    work_position: tauri::PhysicalPosition<i32>,
    work_size: tauri::PhysicalSize<u32>,
    window_size: tauri::PhysicalSize<u32>,
) -> tauri::PhysicalPosition<i32> {
    tauri::PhysicalPosition::new(
        work_position.x + work_size.width.saturating_sub(window_size.width) as i32 / 2,
        work_position.y + work_size.height.saturating_sub(window_size.height) as i32 / 2,
    )
}

fn monitor_contains_point(monitor: &tauri::Monitor, point: tauri::PhysicalPosition<f64>) -> bool {
    let position = monitor.position();
    let size = monitor.size();
    point.x >= position.x as f64
        && point.x < position.x as f64 + size.width as f64
        && point.y >= position.y as f64
        && point.y < position.y as f64 + size.height as f64
}

fn center_main_window_on_cursor(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    let monitors = window
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let cursor = app.cursor_position().ok();
    let monitor = cursor
        .and_then(|point| {
            monitors
                .iter()
                .find(|monitor| monitor_contains_point(monitor, point))
        })
        .cloned()
        .or_else(|| window.primary_monitor().ok().flatten())
        .or_else(|| monitors.first().cloned())
        .ok_or_else(|| "没有可用的显示器".to_string())?;
    let work_area = monitor.work_area();
    let window_size = window.outer_size().map_err(|error| error.to_string())?;
    window
        .set_position(centered_position(
            work_area.position,
            work_area.size,
            window_size,
        ))
        .map_err(|error| error.to_string())
}

pub(crate) fn show_main_window_centered(app: &tauri::AppHandle) -> Result<(), String> {
    show_main_window_at(app, Some("#/settings"))
}

fn should_show_main_window(notice_accepted: bool, silent_startup: bool) -> bool {
    !notice_accepted || !silent_startup
}

pub(crate) fn show_main_window_at(
    app: &tauri::AppHandle,
    route: Option<&str>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    if let Some(route) = route {
        window
            .eval(format!("window.location.hash = {route:?}"))
            .map_err(|error| error.to_string())?;
    }
    if !window.is_visible().unwrap_or(false) {
        if let Err(error) = center_main_window_on_cursor(app, &window) {
            log::warn!("Failed to center the main window; using its current position: {error}");
        }
    }
    if let Err(error) = window.unminimize() {
        log::warn!("Failed to unminimize the main window: {error}");
    }
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

fn set_overlay_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) {
    if let Some(state) = app.try_state::<AppState>() {
        let mut placement = state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        placement.expected_programmatic_position = Some(position);
        placement.programmatic_move_started_at = Some(Instant::now());
    }
    let _ = window.set_position(position);
}

pub(crate) fn move_overlay_to_primary(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.primary_monitor() {
        let work_area = monitor.work_area();
        let window_width = window.outer_size().map(|size| size.width).unwrap_or(760);
        let x =
            work_area.position.x + (work_area.size.width.saturating_sub(window_width) / 2) as i32;
        let y = work_area.position.y + 72;
        set_overlay_position(app, window, tauri::PhysicalPosition::new(x, y));
    }
}

const UNLOCK_HANDLE_BACKGROUND_GAP: f64 = 6.0;
const OVERLAY_EDGE_SNAP_DISTANCE: i32 = 12;
const OVERLAY_POINTER_MONITOR_INTERVAL: Duration = Duration::from_millis(50);
const UNLOCK_HANDLE_HIDE_DELAY: Duration = Duration::from_millis(200);
const UNLOCK_HANDLE_HOVER_EVENT: &str = "unlock-handle://hover";
const OVERLAY_HOVER_EVENT: &str = "overlay://hover";
const NOTCH_HOVER_EVENT: &str = "notch://hover";
const NOTCH_HORIZONTAL_WINDOW_PADDING: f64 = 8.0;
const OVERLAY_TOOLBAR_PLACEMENT_EVENT: &str = "overlay://toolbar-placement";

fn toolbar_placement_after_move(
    orientation: OverlayOrientation,
    placement: ToolbarPlacement,
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    scale: f64,
    work_position: tauri::PhysicalPosition<i32>,
    work_size: tauri::PhysicalSize<u32>,
) -> (ToolbarPlacement, tauri::PhysicalPosition<i32>) {
    let placement = placement.normalized(orientation);
    let inset = (match orientation {
        OverlayOrientation::Horizontal => HORIZONTAL_OVERLAY_SURFACE_INSET,
        OverlayOrientation::Vertical => VERTICAL_OVERLAY_SURFACE_INSET,
    } * scale)
        .round() as i32;
    match (orientation, placement) {
        (OverlayOrientation::Horizontal, ToolbarPlacement::Top)
            if position.y <= work_position.y.saturating_add(OVERLAY_EDGE_SNAP_DISTANCE) =>
        {
            (
                ToolbarPlacement::Bottom,
                tauri::PhysicalPosition::new(position.x, position.y.saturating_add(inset)),
            )
        }
        (OverlayOrientation::Horizontal, ToolbarPlacement::Bottom) => {
            let window_bottom = position.y as i64 + size.height as i64;
            let work_bottom = work_position.y as i64 + work_size.height as i64;
            if window_bottom >= work_bottom - OVERLAY_EDGE_SNAP_DISTANCE as i64 {
                (
                    ToolbarPlacement::Top,
                    tauri::PhysicalPosition::new(position.x, position.y.saturating_sub(inset)),
                )
            } else {
                (placement, position)
            }
        }
        (OverlayOrientation::Vertical, ToolbarPlacement::Right) => {
            let window_right = position.x as i64 + size.width as i64;
            let work_right = work_position.x as i64 + work_size.width as i64;
            if window_right >= work_right - OVERLAY_EDGE_SNAP_DISTANCE as i64 {
                (
                    ToolbarPlacement::Left,
                    tauri::PhysicalPosition::new(position.x.saturating_sub(inset), position.y),
                )
            } else {
                (placement, position)
            }
        }
        (OverlayOrientation::Vertical, ToolbarPlacement::Left) => {
            if position.x as i64 <= work_position.x as i64 + OVERLAY_EDGE_SNAP_DISTANCE as i64 {
                (
                    ToolbarPlacement::Right,
                    tauri::PhysicalPosition::new(position.x.saturating_add(inset), position.y),
                )
            } else {
                (placement, position)
            }
        }
        _ => (placement, position),
    }
}

fn set_overlay_toolbar_placement(app: &tauri::AppHandle, placement: ToolbarPlacement) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let changed = {
        let mut overlay_placement = state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if overlay_placement.toolbar_placement == placement {
            false
        } else {
            overlay_placement.toolbar_placement = placement;
            true
        }
    };
    if changed {
        let _ = app.emit(OVERLAY_TOOLBAR_PLACEMENT_EVENT, placement);
        if let Some(window) = app.get_webview_window("lyrics-overlay") {
            let style = state
                .overlay_style
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            sync_overlay_vibrancy(&window, &style);
        }
    }
}

pub(crate) fn reset_overlay_toolbar_placement(
    app: &tauri::AppHandle,
    orientation: OverlayOrientation,
) {
    set_overlay_toolbar_placement(app, ToolbarPlacement::for_orientation(orientation));
}

fn adjust_overlay_toolbar_for_move(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) -> tauri::PhysicalPosition<i32> {
    let (Ok(Some(monitor)), Ok(size)) = (window.current_monitor(), window.outer_size()) else {
        return position;
    };
    let state = app.state::<AppState>();
    let orientation = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .orientation;
    let placement = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .toolbar_placement;
    let scale = monitor.scale_factor();
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let work_area = monitor.work_area();
    let (next_placement, next_position) = toolbar_placement_after_move(
        orientation,
        placement,
        position,
        size,
        scale,
        work_area.position,
        work_area.size,
    );
    set_overlay_toolbar_placement(app, next_placement);
    next_position
}

#[cfg(target_os = "macos")]
pub(crate) fn primary_mouse_button_pressed() -> bool {
    use objc2_app_kit::NSEvent;

    NSEvent::pressedMouseButtons() & 1 != 0
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn primary_mouse_button_pressed() -> bool {
    false
}

fn point_in_window_bounds(
    point: tauri::PhysicalPosition<f64>,
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
) -> bool {
    let right = position.x as f64 + size.width as f64;
    let bottom = position.y as f64 + size.height as f64;
    point.x >= position.x as f64
        && point.x < right
        && point.y >= position.y as f64
        && point.y < bottom
}

fn should_hover_overlay(
    settings: &OverlaySettings,
    cursor: tauri::PhysicalPosition<f64>,
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
) -> bool {
    settings.visible && !settings.locked && point_in_window_bounds(cursor, position, size)
}

fn stable_overlay_hover(previous: Option<bool>, sampled: bool, mouse_pressed: bool) -> bool {
    if mouse_pressed {
        previous.unwrap_or(sampled)
    } else {
        sampled
    }
}

fn unlock_handle_position(
    placement: ToolbarPlacement,
    overlay_position: tauri::PhysicalPosition<i32>,
    overlay_size: tauri::PhysicalSize<u32>,
    handle_size: tauri::PhysicalSize<u32>,
    surface_inset: u32,
    background_gap: u32,
) -> tauri::PhysicalPosition<i32> {
    let available_width = overlay_size.width.saturating_sub(handle_size.width);
    let available_height = overlay_size.height.saturating_sub(handle_size.height);
    match placement {
        ToolbarPlacement::Top => tauri::PhysicalPosition::new(
            overlay_position
                .x
                .saturating_add((available_width / 2) as i32),
            overlay_position.y.saturating_add(
                surface_inset
                    .saturating_sub(background_gap)
                    .saturating_sub(handle_size.height)
                    .min(available_height) as i32,
            ),
        ),
        ToolbarPlacement::Bottom => tauri::PhysicalPosition::new(
            overlay_position
                .x
                .saturating_add((available_width / 2) as i32),
            overlay_position.y.saturating_add(
                overlay_size
                    .height
                    .saturating_sub(surface_inset)
                    .saturating_add(background_gap)
                    .min(available_height) as i32,
            ),
        ),
        ToolbarPlacement::Left => tauri::PhysicalPosition::new(
            overlay_position.x.saturating_add(
                surface_inset
                    .saturating_sub(background_gap)
                    .saturating_sub(handle_size.width)
                    .min(available_width) as i32,
            ),
            overlay_position
                .y
                .saturating_add((available_height / 2) as i32),
        ),
        ToolbarPlacement::Right => tauri::PhysicalPosition::new(
            overlay_position.x.saturating_add(
                overlay_size
                    .width
                    .saturating_sub(surface_inset)
                    .saturating_add(background_gap)
                    .min(available_width) as i32,
            ),
            overlay_position
                .y
                .saturating_add((available_height / 2) as i32),
        ),
    }
}

fn position_unlock_handle(app: &tauri::AppHandle) {
    let (Some(overlay), Some(handle)) = (
        app.get_webview_window("lyrics-overlay"),
        app.get_webview_window("lyrics-unlock-handle"),
    ) else {
        return;
    };
    let (Ok(position), Ok(size), Ok(handle_size)) = (
        overlay.outer_position(),
        overlay.outer_size(),
        handle.outer_size(),
    ) else {
        return;
    };
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let orientation = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .orientation;
    let placement = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .toolbar_placement
        .normalized(orientation);
    let scale = overlay.scale_factor().unwrap_or(1.0);
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let surface_inset = (match orientation {
        OverlayOrientation::Horizontal => HORIZONTAL_OVERLAY_SURFACE_INSET,
        OverlayOrientation::Vertical => VERTICAL_OVERLAY_SURFACE_INSET,
    } * scale)
        .round() as u32;
    let background_gap = (UNLOCK_HANDLE_BACKGROUND_GAP * scale).round() as u32;
    let _ = handle.set_position(unlock_handle_position(
        placement,
        position,
        size,
        handle_size,
        surface_inset,
        background_gap,
    ));
}

pub(crate) fn sync_unlock_handle(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let settings = state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let (Some(overlay), Some(handle)) = (
        app.get_webview_window("lyrics-overlay"),
        app.get_webview_window("lyrics-unlock-handle"),
    ) else {
        return;
    };
    let should_show = settings.visible && settings.locked && overlay.is_visible().unwrap_or(false);
    let is_visible = handle.is_visible().unwrap_or(false);
    if should_show {
        position_unlock_handle(app);
    } else if is_visible {
        let _ = handle.hide();
        let _ = handle.emit(UNLOCK_HANDLE_HOVER_EVENT, false);
    }
}

fn start_overlay_pointer_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_inside_at: Option<Instant> = None;
        let mut last_handle_hovered: Option<bool> = None;
        let mut last_overlay_hovered: Option<bool> = None;
        let mut last_notch_hovered: Option<bool> = None;

        loop {
            tokio::time::sleep(OVERLAY_POINTER_MONITOR_INTERVAL).await;

            let Some(state) = app.try_state::<AppState>() else {
                continue;
            };
            let settings = state
                .overlay_settings
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone();

            if let Some(notch) = app.get_webview_window("lyrics-notch") {
                let notch_visible = notch.is_visible().unwrap_or(false);
                let sampled_notch_hover = notch_visible
                    && match (
                        app.cursor_position(),
                        notch.outer_position(),
                        notch.outer_size(),
                        notch.scale_factor(),
                    ) {
                        (Ok(cursor), Ok(position), Ok(size), Ok(scale_factor)) => {
                            let horizontal_padding =
                                NOTCH_HORIZONTAL_WINDOW_PADDING * scale_factor;
                            let left = position.x as f64 + horizontal_padding;
                            let right = position.x as f64 + size.width as f64
                                - horizontal_padding;
                            let bottom = position.y as f64 + size.height as f64;
                            cursor.x >= left
                                && cursor.x < right
                                && cursor.y >= position.y as f64
                                && cursor.y < bottom
                        }
                        _ => false,
                    };
                let notch_hovered = stable_overlay_hover(
                    last_notch_hovered,
                    sampled_notch_hover,
                    notch_visible && primary_mouse_button_pressed(),
                );
                if last_notch_hovered != Some(notch_hovered) {
                    let _ = notch.emit(NOTCH_HOVER_EVENT, notch_hovered);
                    last_notch_hovered = Some(notch_hovered);
                }
            } else {
                last_notch_hovered = None;
            }

            let (Some(overlay), Some(handle)) = (
                app.get_webview_window("lyrics-overlay"),
                app.get_webview_window("lyrics-unlock-handle"),
            ) else {
                continue;
            };
            let overlay_visible = overlay.is_visible().unwrap_or(false);

            let overlay_sample = match (
                app.cursor_position(),
                overlay.outer_position(),
                overlay.outer_size(),
            ) {
                (Ok(cursor), Ok(position), Ok(size)) => Some((cursor, position, size)),
                _ => None,
            };
            let sampled_overlay_hover = overlay_visible
                && overlay_sample
                    .as_ref()
                    .is_some_and(|(cursor, position, size)| {
                        should_hover_overlay(&settings, *cursor, *position, *size)
                    });
            let overlay_hovered = stable_overlay_hover(
                last_overlay_hovered,
                sampled_overlay_hover,
                overlay_visible
                    && settings.visible
                    && !settings.locked
                    && primary_mouse_button_pressed(),
            );
            if last_overlay_hovered != Some(overlay_hovered) {
                let _ = overlay.emit(OVERLAY_HOVER_EVENT, overlay_hovered);
                last_overlay_hovered = Some(overlay_hovered);
            }

            if !settings.visible || !settings.locked || !overlay_visible {
                last_inside_at = None;
                if handle.is_visible().unwrap_or(false) {
                    let _ = handle.hide();
                }
                if last_handle_hovered != Some(false) {
                    let _ = handle.emit(UNLOCK_HANDLE_HOVER_EVENT, false);
                    last_handle_hovered = Some(false);
                }
                continue;
            }

            let sample = (overlay_sample, handle.outer_position(), handle.outer_size());
            let (should_show, hovered) = match sample {
                (
                    Some((cursor, overlay_position, overlay_size)),
                    Ok(handle_position),
                    Ok(handle_size),
                ) => {
                    let now = Instant::now();
                    let inside_overlay =
                        point_in_window_bounds(cursor, overlay_position, overlay_size);
                    if inside_overlay {
                        last_inside_at = Some(now);
                    }
                    let within_hide_delay = last_inside_at.is_some_and(|last_inside| {
                        now.duration_since(last_inside) < UNLOCK_HANDLE_HIDE_DELAY
                    });
                    (
                        inside_overlay || within_hide_delay,
                        inside_overlay
                            && point_in_window_bounds(cursor, handle_position, handle_size),
                    )
                }
                _ => {
                    // 读取系统鼠标或窗口边界失败时优先保留解锁入口，下一轮继续重试。
                    last_inside_at = None;
                    (true, false)
                }
            };

            if should_show != handle.is_visible().unwrap_or(false) {
                if should_show {
                    position_unlock_handle(&app);
                    let _ = handle.show();
                } else {
                    let _ = handle.hide();
                }
            }
            let hovered = should_show && hovered;
            if last_handle_hovered != Some(hovered) {
                let _ = handle.emit(UNLOCK_HANDLE_HOVER_EVENT, hovered);
                last_handle_hovered = Some(hovered);
            }
        }
    });
}

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
    let overlay_settings = state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();

    let create_windows = (|| {
        create_overlay(app).map_err(|error| error.to_string())?;
        create_unlock_handle(app).map_err(|error| error.to_string())
    })();
    #[cfg(target_os = "macos")]
    let restore_dock = apply_dock_icon_hidden(app, configured.app.hide_dock_icon);
    create_windows?;
    #[cfg(target_os = "macos")]
    restore_dock?;
    if let Some(window) = app.get_webview_window("lyrics-overlay") {
        let _ = window.set_resizable(false);
        let _ = window.set_ignore_cursor_events(overlay_settings.locked);
        let _ = window.set_focusable(!overlay_settings.locked);
        if !overlay_settings.locked {
            refresh_overlay_mouse_tracking(&window);
        }
        restore_overlay_position(app, &window);
    }
    setup_tray(app).map_err(|error| error.to_string())?;
    if !configured.app.language.uses_native_chinese() {
        apply_native_language(app, UiLanguage::EnUs)?;
    }
    if let Err(error) = register_global_shortcuts(app, &configured.app.shortcuts) {
        log::warn!(
            "Failed to register global shortcuts at startup; runtime will continue: {error}"
        );
    }

    *started = true;
    commands::start_library_scan(app);
    if let Err(error) = reconcile_overlay_visibility(app) {
        log::warn!("Failed to reconcile overlay visibility at activation: {error}");
    }
    if let Err(error) = reconcile_auxiliary_lyrics_windows(app) {
        log::warn!("Failed to restore auxiliary lyrics windows: {error}");
    }
    start_overlay_pointer_monitor(app.clone());
    start_player_monitor(app.clone());
    player_lifecycle::start_exit_monitor(app.clone());
    Ok(())
}

fn snapped_position(
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) -> tauri::PhysicalPosition<i32> {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return position;
    };
    let Ok(size) = window.outer_size() else {
        return position;
    };
    let origin = monitor.position();
    let monitor_size = monitor.size();
    let right = origin.x + monitor_size.width as i32 - size.width as i32;
    let bottom = origin.y + monitor_size.height as i32 - size.height as i32;
    tauri::PhysicalPosition::new(
        snap_coordinate(position.x, origin.x, right),
        snap_coordinate(position.y, origin.y, bottom),
    )
}

fn snap_coordinate(value: i32, start: i32, end: i32) -> i32 {
    if value.abs_diff(start) <= 12 {
        start
    } else if value.abs_diff(end) <= 12 {
        end
    } else {
        value
    }
}

fn relative_axis(position: i32, start: i32, work_length: u32, window_length: u32) -> f64 {
    let available = work_length.saturating_sub(window_length);
    if available == 0 {
        0.0
    } else {
        ((position as i64 - start as i64) as f64 / available as f64).clamp(0.0, 1.0)
    }
}

fn clamp_axis(position: i32, start: i32, work_length: u32, window_length: u32) -> i32 {
    let maximum = start as i64 + work_length.saturating_sub(window_length) as i64;
    (position as i64).clamp(start as i64, maximum.max(start as i64)) as i32
}

fn restored_overlay_position(
    bounds: &StoredBounds,
    work_position: tauri::PhysicalPosition<i32>,
    work_size: tauri::PhysicalSize<u32>,
    window_size: tauri::PhysicalSize<u32>,
    scale_factor: f64,
) -> tauri::PhysicalPosition<i32> {
    let same_work_area = bounds.work_x == Some(work_position.x)
        && bounds.work_y == Some(work_position.y)
        && bounds.work_width == Some(work_size.width)
        && bounds.work_height == Some(work_size.height)
        && bounds
            .scale_factor
            .is_some_and(|saved| (saved - scale_factor).abs() < 0.001);

    let (x, y) = if same_work_area {
        (bounds.x, bounds.y)
    } else {
        let available_width = work_size.width.saturating_sub(window_size.width);
        let available_height = work_size.height.saturating_sub(window_size.height);
        (
            bounds.relative_x.map_or(bounds.x, |relative| {
                work_position.x + (relative.clamp(0.0, 1.0) * available_width as f64).round() as i32
            }),
            bounds.relative_y.map_or(bounds.y, |relative| {
                work_position.y
                    + (relative.clamp(0.0, 1.0) * available_height as f64).round() as i32
            }),
        )
    };

    tauri::PhysicalPosition::new(
        clamp_axis(x, work_position.x, work_size.width, window_size.width),
        clamp_axis(y, work_position.y, work_size.height, window_size.height),
    )
}

fn stored_bounds(
    position: tauri::PhysicalPosition<i32>,
    window_size: tauri::PhysicalSize<u32>,
    monitor: &tauri::Monitor,
    toolbar_placement: ToolbarPlacement,
) -> StoredBounds {
    let work_area = monitor.work_area();
    StoredBounds {
        x: position.x,
        y: position.y,
        work_x: Some(work_area.position.x),
        work_y: Some(work_area.position.y),
        work_width: Some(work_area.size.width),
        work_height: Some(work_area.size.height),
        scale_factor: Some(monitor.scale_factor()),
        relative_x: Some(relative_axis(
            position.x,
            work_area.position.x,
            work_area.size.width,
            window_size.width,
        )),
        relative_y: Some(relative_axis(
            position.y,
            work_area.position.y,
            work_area.size.height,
            window_size.height,
        )),
        toolbar_placement: Some(toolbar_placement),
    }
}

fn persist_overlay_state_at(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let id = monitor_id(&monitor);
    let state = app.state::<AppState>();
    if !state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .visible
    {
        return;
    }
    let mut active_monitor = state
        .overlay_monitor
        .write()
        .unwrap_or_else(|error| error.into_inner());
    if active_monitor.as_deref() != Some(&id) {
        *active_monitor = Some(id.clone());
    }
    drop(active_monitor);

    let Ok(window_size) = window.outer_size() else {
        return;
    };
    let toolbar_placement = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .toolbar_placement;
    let bounds = stored_bounds(position, window_size, &monitor, toolbar_placement);
    if let Ok(raw) = serde_json::to_string(&bounds) {
        let _ = state.storage.set_preference("overlay.last_monitor", &id);
        let _ = state
            .storage
            .set_preference(&format!("overlay.position.{id}"), &raw);
        state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .preferred_monitor = Some(id);
    }
    position_unlock_handle(app);
}

fn persist_overlay_state(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    if let Ok(position) = window.outer_position() {
        persist_overlay_state_at(app, window, position);
    }
}

fn overlay_intersects_any_monitor(
    window: &tauri::WebviewWindow,
    monitors: &[tauri::Monitor],
) -> bool {
    let (Ok(position), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return false;
    };
    let right = position.x as i64 + size.width as i64;
    let bottom = position.y as i64 + size.height as i64;
    monitors.iter().any(|monitor| {
        let monitor_position = monitor.position();
        let monitor_size = monitor.size();
        let monitor_right = monitor_position.x as i64 + monitor_size.width as i64;
        let monitor_bottom = monitor_position.y as i64 + monitor_size.height as i64;
        right > monitor_position.x as i64
            && (position.x as i64) < monitor_right
            && bottom > monitor_position.y as i64
            && (position.y as i64) < monitor_bottom
    })
}

fn apply_stored_overlay_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    monitor: &tauri::Monitor,
) -> bool {
    let state = app.state::<AppState>();
    let key = format!("overlay.position.{}", monitor_id(monitor));
    let Some(bounds) = state
        .storage
        .get_preference(&key)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<StoredBounds>(&raw).ok())
    else {
        return false;
    };
    let Ok(window_size) = window.outer_size() else {
        return false;
    };
    let work_area = monitor.work_area();
    let position = restored_overlay_position(
        &bounds,
        work_area.position,
        work_area.size,
        window_size,
        monitor.scale_factor(),
    );
    let orientation = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .orientation;
    set_overlay_toolbar_placement(
        app,
        bounds
            .toolbar_placement
            .unwrap_or_else(|| ToolbarPlacement::for_orientation(orientation))
            .normalized(orientation),
    );
    set_overlay_position(app, window, position);
    true
}

fn refresh_overlay_topology(app: &tauri::AppHandle, monitors: &[tauri::Monitor]) -> bool {
    let state = app.state::<AppState>();
    let next = monitor_topology(monitors);
    let mut placement = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    placement.update_topology(next)
}

fn restore_preferred_overlay_placement(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    monitors: &[tauri::Monitor],
) {
    let preferred_monitor = app
        .state::<AppState>()
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .preferred_monitor
        .clone();
    if let Some(preferred_monitor) = preferred_monitor {
        if let Some(monitor) = monitors
            .iter()
            .find(|monitor| monitor_id(monitor) == preferred_monitor)
        {
            if apply_stored_overlay_position(app, window, monitor) {
                return;
            }
        } else {
            move_overlay_to_primary(app, window);
            return;
        }
    }
    if !overlay_intersects_any_monitor(window, monitors) {
        move_overlay_to_primary(app, window);
    }
}

fn reconcile_overlay_placement(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let monitors = window.available_monitors().unwrap_or_default();
    if monitors.is_empty() || !refresh_overlay_topology(app, &monitors) {
        return;
    }
    restore_preferred_overlay_placement(app, window, &monitors);
}

fn ignore_overlay_move(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) -> bool {
    let monitors = window.available_monitors().unwrap_or_default();
    let topology_changed = !monitors.is_empty() && refresh_overlay_topology(app, &monitors);
    let state = app.state::<AppState>();
    let mut placement = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let programmatic = placement.consume_programmatic_move(position);
    drop(placement);

    if topology_changed {
        restore_preferred_overlay_placement(app, window, &monitors);
    }
    topology_changed || programmatic
}

fn suppress_overlay_persistence(app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> bool {
    let monitors = window.available_monitors().unwrap_or_default();
    let topology_changed = !monitors.is_empty() && refresh_overlay_topology(app, &monitors);
    if topology_changed {
        restore_preferred_overlay_placement(app, window, &monitors);
        return true;
    }
    app.state::<AppState>()
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .suppress_persistence(Instant::now())
}

pub(crate) fn restore_overlay_position(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let state = app.state::<AppState>();
    let monitors = window.available_monitors().unwrap_or_default();
    let last_monitor = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .preferred_monitor
        .clone();

    if let Some(monitor) = last_monitor
        .as_ref()
        .and_then(|id| monitors.iter().find(|monitor| monitor_id(monitor) == *id))
    {
        if apply_stored_overlay_position(app, window, monitor) {
            return;
        }
    }
    move_overlay_to_primary(app, window);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .max_file_size(1_000_000)
                .rotation_strategy(RotationStrategy::KeepSome(3))
                .target(Target::new(TargetKind::Webview))
                .build(),
        )
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .skip_initial_state("main")
                .skip_initial_state("lyrics-overlay")
                .skip_initial_state("quick-lyrics")
                .skip_initial_state("lyrics-notch")
                .build(),
        )
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let storage = storage::Storage::new(app.handle())?;
            let notice_accepted = legal_notice_accepted(&storage).unwrap_or(false);
            let app_dir = app.path().app_data_dir()?;
            let (config, migrated) = ConfigStore::load(&app_dir, &storage)
                .map_err(|error| std::io::Error::other(error))?;
            let config = Arc::new(config);
            let configured = config.snapshot();
            let provider_settings = configured.lyrics.providers.clone();
            let selection = configured.app.player_selection;
            let locked = configured.overlay.locked;
            let overlay_settings = OverlaySettings {
                visible: configured.overlay.visible,
                locked,
            };
            let last_overlay_monitor = storage
                .get_preference("overlay.last_monitor")
                .unwrap_or(None);
            let geometry = overlay_geometry(&storage, last_overlay_monitor.as_deref());
            if migrated {
                let geometry_key = last_overlay_monitor
                    .as_ref()
                    .map(|id| format!("overlay.geometry.{id}"))
                    .unwrap_or_else(|| "overlay.geometry.default".into());
                let raw = serde_json::to_string(&geometry)?;
                storage.set_preference(&geometry_key, &raw)?;
                for key in [
                    "player.selection",
                    "lyrics.providers",
                    "overlay.visible",
                    "overlay.locked",
                    "overlay.passthrough",
                ] {
                    storage.remove_preference(key)?;
                }
                storage.remove_preferences_with_prefix("overlay.style.")?;
            }
            let mut overlay_style = configured.overlay.appearance.clone().into_style();
            overlay_style.horizontal_max_width = geometry.horizontal_max_width;
            overlay_style.vertical_max_height = geometry.vertical_max_height;
            let initial_toolbar_placement =
                ToolbarPlacement::for_orientation(overlay_style.orientation);
            app.manage(AppState {
                runtime_started: Mutex::new(false),
                selection: Arc::new(RwLock::new(selection)),
                auto_player: Arc::new(RwLock::new(None)),
                overlay_settings: Arc::new(RwLock::new(overlay_settings.clone())),
                overlay_style: Arc::new(RwLock::new(overlay_style)),
                overlay_monitor: Arc::new(RwLock::new(last_overlay_monitor.clone())),
                overlay_placement: Arc::new(Mutex::new(OverlayPlacementState {
                    preferred_monitor: last_overlay_monitor,
                    toolbar_placement: initial_toolbar_placement,
                    ..OverlayPlacementState::default()
                })),
                last_snapshot: Arc::new(RwLock::new(player::PlaybackSnapshot::empty())),
                lyrics_runtime: Arc::new(RwLock::new(commands::LyricsRuntimeSnapshot::default())),
                lyrics_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                lyrics_auto_search_attempted: Arc::new(Mutex::new(std::collections::HashSet::new())),
                notch_layout_metrics: Arc::new(RwLock::new(NotchLayoutMetrics::default())),
                notch_visibility: Arc::new(Mutex::new(NotchVisibilityState::default())),
                storage: Arc::new(storage),
                config,
                providers: Arc::new(lyrics::provider::ProviderRegistry::new(provider_settings)),
                system_media: Arc::new(SystemMediaService::default()),
                http: reqwest::Client::builder()
                    .user_agent(concat!(
                        "Lyrics Plus/",
                        env!("CARGO_PKG_VERSION"),
                        " (https://github.com/afeibukaixin/Lyrics-Plus)"
                    ))
                    .timeout(Duration::from_secs(8))
                    .build()
                    .map_err(|error| error.to_string())?,
            });

            if let Err(error) = player_lifecycle::sync_service(app.handle(), &configured.app) {
                log::warn!("Failed to configure player follower: {error}");
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_size(tauri::LogicalSize::new(980.0, 720.0));
                let _ = window.set_resizable(false);
                let _ = window.set_maximizable(false);
                let _ = center_main_window_on_cursor(app.handle(), &window);
            }

            if notice_accepted {
                activate_runtime(app.handle()).map_err(std::io::Error::other)?;
            }
            if should_show_main_window(notice_accepted, configured.app.silent_startup) {
                show_main_window_centered(app.handle()).map_err(std::io::Error::other)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let runtime_started = window
                        .app_handle()
                        .try_state::<AppState>()
                        .map(|state| {
                            *state
                                .runtime_started
                                .lock()
                                .unwrap_or_else(|error| error.into_inner())
                        })
                        .unwrap_or(false);
                    if !runtime_started {
                        log::info!(
                            "Application exit requested: reason=main_window_closed_before_runtime_started"
                        );
                        window.app_handle().exit(0);
                        return;
                    }
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            if window.label() == "lyrics-list" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                    if let Some(state) = window.app_handle().try_state::<AppState>() {
                        if let Ok(config) = state.config.update(|config| {
                            config.lyrics.displays.list_window.enabled = false;
                        }) {
                            let _ = window.app_handle().emit("config://changed", &config);
                        }
                    }
                    sync_lyrics_surfaces(window.app_handle());
                }
            }
            if window.label() == "lyrics-overlay" {
                if let tauri::WindowEvent::Moved(position) = event {
                    if let Some(overlay) = window.app_handle().get_webview_window("lyrics-overlay")
                    {
                        if ignore_overlay_move(window.app_handle(), &overlay, *position) {
                            return;
                        }
                        let snapped = snapped_position(&overlay, *position);
                        let adjusted =
                            adjust_overlay_toolbar_for_move(window.app_handle(), &overlay, snapped);
                        if adjusted != *position {
                            set_overlay_position(window.app_handle(), &overlay, adjusted);
                            persist_overlay_state_at(window.app_handle(), &overlay, adjusted);
                            return;
                        }
                        persist_overlay_state_at(window.app_handle(), &overlay, adjusted);
                    }
                }
                if matches!(event, tauri::WindowEvent::Resized(_)) {
                    if let Some(overlay) = window.app_handle().get_webview_window("lyrics-overlay")
                    {
                        if let Some(state) = window.app_handle().try_state::<AppState>() {
                            let style = state
                                .overlay_style
                                .read()
                                .unwrap_or_else(|error| error.into_inner())
                                .clone();
                            sync_overlay_vibrancy(&overlay, &style);
                        }
                        if !suppress_overlay_persistence(window.app_handle(), &overlay) {
                            persist_overlay_state(window.app_handle(), &overlay);
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_legal_notice_status,
            commands::accept_legal_notice,
            commands::quit_application,
            commands::get_playback_snapshot,
            commands::get_player_selection,
            commands::set_player_selection,
            commands::search_lyrics,
            commands::get_provider_settings,
            commands::set_provider_settings,
            commands::test_provider,
            commands::get_cached_lyrics,
            commands::get_lyrics_runtime_snapshot,
            commands::get_notch_layout_metrics,
            commands::get_lyrics_monitors,
            commands::save_lyrics,
            commands::import_lyrics,
            commands::set_lyrics_offset,
            commands::remove_lyrics_association,
            commands::get_library_scan_status,
            commands::rescan_lyrics_library,
            commands::set_lyrics_directory,
            commands::open_lyrics_directory,
            commands::set_overlay_visible,
            commands::get_overlay_settings,
            commands::set_overlay_locked,
            commands::get_overlay_style,
            commands::get_overlay_toolbar_placement,
            commands::set_overlay_style,
            commands::nudge_overlay,
            commands::reset_overlay_bounds,
            commands::resize_overlay_edge,
            commands::fit_overlay_content,
            commands::fit_notch_lyrics_content,
            commands::show_main_window,
            commands::show_lyrics_style_settings,
            commands::show_quick_lyrics_window,
            commands::get_app_config,
            commands::set_theme,
            commands::resolve_system_media_applications,
            commands::set_system_media_filter_mode,
            commands::set_system_media_applications,
            commands::resolve_player_follower_application,
            commands::set_player_follower_application,
            commands::get_player_follower_service_status,
            commands::open_player_follower_system_settings,
            commands::open_automation_system_settings,
            commands::get_application_icons,
            commands::resolve_application_by_bundle_id,
            commands::set_language,
            commands::set_native_language,
            commands::get_global_shortcut_status,
            commands::set_global_shortcuts,
            commands::set_dock_icon_hidden,
            commands::set_silent_startup,
            commands::set_auto_check_updates,
            commands::set_overlay_hide_when_not_playing,
            commands::set_status_bar_lyrics_enabled,
            commands::set_list_lyrics_visible,
            commands::set_list_lyrics_options,
            commands::set_notch_lyrics_visible,
            commands::set_lyrics_display_preferences,
            commands::set_lyrics_base_appearance,
            commands::set_lyrics_style_inheritance,
            commands::reset_lyrics_base_appearance,
            commands::reset_lyrics_style_mode,
            commands::reset_lyrics_display_position,
            commands::reset_list_lyrics_window_size,
            commands::export_app_config,
            commands::reveal_config_directory,
            commands::get_config_editor_data,
            commands::validate_app_config_draft,
            commands::save_app_config_draft,
            commands::reset_settings_section,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Lyrics Plus");
    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if matches!(event, tauri::RunEvent::Reopen { .. }) {
            let _ = show_main_window_centered(app);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_startup_hides_only_after_accepting_the_notice() {
        assert!(should_show_main_window(false, true));
        assert!(should_show_main_window(true, false));
        assert!(!should_show_main_window(true, true));
    }

    #[test]
    fn overlay_initial_size_restores_the_saved_fixed_axis() {
        let horizontal = OverlayStyleSettings {
            horizontal_max_width: Some(540.0),
            ..OverlayStyleSettings::default()
        };
        assert_eq!(initial_overlay_dimensions(&horizontal), (540.0, 156.0));

        let vertical = OverlayStyleSettings {
            orientation: OverlayOrientation::Vertical,
            vertical_max_height: Some(480.0),
            ..OverlayStyleSettings::default()
        };
        assert_eq!(initial_overlay_dimensions(&vertical), (190.0, 480.0));
    }

    #[test]
    fn overlay_initial_size_uses_orientation_defaults_without_saved_geometry() {
        assert_eq!(
            initial_overlay_dimensions(&OverlayStyleSettings::default()),
            (760.0, 156.0)
        );
        let vertical = OverlayStyleSettings {
            orientation: OverlayOrientation::Vertical,
            ..OverlayStyleSettings::default()
        };
        assert_eq!(initial_overlay_dimensions(&vertical), (190.0, 620.0));
    }

    #[test]
    fn edge_snap_only_applies_inside_threshold() {
        assert_eq!(snap_coordinate(8, 0, 100), 0);
        assert_eq!(snap_coordinate(91, 0, 100), 100);
        assert_eq!(snap_coordinate(50, 0, 100), 50);
    }

    #[test]
    fn toolbar_placement_stays_until_opposite_edge() {
        use OverlayOrientation::{Horizontal, Vertical};
        use ToolbarPlacement::{Bottom, Left, Right, Top};

        let point = tauri::PhysicalPosition::new;
        let moved = |orientation, placement, x, y| {
            toolbar_placement_after_move(
                orientation,
                placement,
                point(x, y),
                tauri::PhysicalSize::new(300, 100),
                1.0,
                point(100, 200),
                tauri::PhysicalSize::new(1_200, 800),
            )
        };

        assert_eq!(moved(Horizontal, Top, 500, 205), (Bottom, point(500, 251)),);
        assert_eq!(
            moved(Horizontal, Bottom, 500, 500),
            (Bottom, point(500, 500)),
        );
        assert_eq!(moved(Horizontal, Bottom, 500, 890), (Top, point(500, 844)),);
        assert_eq!(moved(Vertical, Right, 995, 500), (Left, point(947, 500)),);
        assert_eq!(moved(Vertical, Left, 500, 500), (Left, point(500, 500)),);
        assert_eq!(moved(Vertical, Left, 105, 500), (Right, point(153, 500)),);
    }

    #[test]
    fn overlay_hover_is_frozen_while_primary_button_is_pressed() {
        assert!(stable_overlay_hover(Some(true), false, true));
        assert!(!stable_overlay_hover(Some(false), true, true));
        assert!(stable_overlay_hover(None, true, true));
        assert!(!stable_overlay_hover(Some(true), false, false));
    }

    #[test]
    fn old_position_records_remain_compatible() {
        let bounds: StoredBounds =
            serde_json::from_str(r#"{"x":12,"y":34,"width":760,"height":156}"#).unwrap();
        assert_eq!((bounds.x, bounds.y), (12, 34));
        assert_eq!(bounds.relative_x, None);
        assert_eq!(bounds.work_width, None);
        assert_eq!(bounds.toolbar_placement, None);
    }

    #[test]
    fn old_position_records_are_clamped_to_the_current_work_area() {
        let bounds: StoredBounds = serde_json::from_str(r#"{"x":900,"y":700}"#).unwrap();
        assert_eq!(
            restored_overlay_position(
                &bounds,
                tauri::PhysicalPosition::new(0, 0),
                tauri::PhysicalSize::new(800, 600),
                tauri::PhysicalSize::new(200, 100),
                2.0,
            ),
            tauri::PhysicalPosition::new(600, 500),
        );
    }

    #[test]
    fn relative_position_adapts_to_resolution_and_monitor_origin_changes() {
        let bounds: StoredBounds = serde_json::from_str(
            r#"{"x":400,"y":200,"workX":0,"workY":0,"workWidth":1000,"workHeight":800,"scaleFactor":2.0,"relativeX":0.5,"relativeY":0.25}"#,
        )
        .unwrap();
        assert_eq!(
            restored_overlay_position(
                &bounds,
                tauri::PhysicalPosition::new(1920, 0),
                tauri::PhysicalSize::new(2000, 1200),
                tauri::PhysicalSize::new(200, 100),
                1.0,
            ),
            tauri::PhysicalPosition::new(2820, 275),
        );
    }

    #[test]
    fn unchanged_work_area_preserves_exact_saved_position() {
        let bounds: StoredBounds = serde_json::from_str(
            r#"{"x":321,"y":234,"workX":0,"workY":24,"workWidth":1440,"workHeight":876,"scaleFactor":2.0,"relativeX":0.1,"relativeY":0.9}"#,
        )
        .unwrap();
        assert_eq!(
            restored_overlay_position(
                &bounds,
                tauri::PhysicalPosition::new(0, 24),
                tauri::PhysicalSize::new(1440, 876),
                tauri::PhysicalSize::new(760, 156),
                2.0,
            ),
            tauri::PhysicalPosition::new(321, 234),
        );
    }

    #[test]
    fn main_window_is_centered_inside_negative_origin_work_area() {
        assert_eq!(
            centered_position(
                tauri::PhysicalPosition::new(-1920, 24),
                tauri::PhysicalSize::new(1920, 1056),
                tauri::PhysicalSize::new(980, 720),
            ),
            tauri::PhysicalPosition::new(-1450, 192),
        );
    }

    #[test]
    fn topology_changes_preserve_preferred_monitor_and_clear_programmatic_move() {
        let topology = |width| {
            vec![MonitorTopologyEntry {
                id: "external".into(),
                x: 0,
                y: 0,
                width,
                height: 1080,
                work_x: 0,
                work_y: 24,
                work_width: width,
                work_height: 1056,
                scale_factor_bits: 1.0_f64.to_bits(),
            }]
        };
        let mut placement = OverlayPlacementState {
            preferred_monitor: Some("external".into()),
            expected_programmatic_position: Some(tauri::PhysicalPosition::new(10, 20)),
            ..OverlayPlacementState::default()
        };
        assert!(!placement.update_topology(topology(1920)));
        assert!(placement.consume_programmatic_move(tauri::PhysicalPosition::new(10, 20)));
        placement.expected_programmatic_position = Some(tauri::PhysicalPosition::new(30, 40));
        placement.programmatic_move_started_at = Some(Instant::now());
        assert!(placement.update_topology(topology(2560)));
        assert_eq!(placement.preferred_monitor.as_deref(), Some("external"));
        assert_eq!(placement.expected_programmatic_position, None);
        assert_eq!(placement.programmatic_move_started_at, None);
    }

    #[test]
    fn programmatic_move_suppression_expires() {
        let now = Instant::now();
        let mut placement = OverlayPlacementState {
            expected_programmatic_position: Some(tauri::PhysicalPosition::new(10, 20)),
            programmatic_move_started_at: Some(
                now - PROGRAMMATIC_MOVE_SUPPRESSION - Duration::from_millis(1),
            ),
            ..OverlayPlacementState::default()
        };
        assert!(!placement.suppress_persistence(now));
        assert_eq!(placement.expected_programmatic_position, None);
    }

    #[test]
    fn horizontal_unlock_handle_is_centered_at_the_top() {
        let overlay_position = tauri::PhysicalPosition::new(100, 200);
        let overlay_size = tauri::PhysicalSize::new(760, 156);
        let handle_size = tauri::PhysicalSize::new(28, 28);
        assert_eq!(
            unlock_handle_position(
                ToolbarPlacement::Top,
                overlay_position,
                overlay_size,
                handle_size,
                46,
                6,
            ),
            tauri::PhysicalPosition::new(466, 212),
        );
        assert_eq!(
            unlock_handle_position(
                ToolbarPlacement::Bottom,
                overlay_position,
                overlay_size,
                handle_size,
                46,
                6,
            ),
            tauri::PhysicalPosition::new(466, 316),
        );
    }

    #[test]
    fn vertical_unlock_handle_is_centered_at_the_right() {
        let overlay_position = tauri::PhysicalPosition::new(100, 200);
        let overlay_size = tauri::PhysicalSize::new(190, 620);
        let handle_size = tauri::PhysicalSize::new(28, 28);
        assert_eq!(
            unlock_handle_position(
                ToolbarPlacement::Right,
                overlay_position,
                overlay_size,
                handle_size,
                48,
                6,
            ),
            tauri::PhysicalPosition::new(248, 496),
        );
        assert_eq!(
            unlock_handle_position(
                ToolbarPlacement::Left,
                overlay_position,
                overlay_size,
                handle_size,
                48,
                6,
            ),
            tauri::PhysicalPosition::new(114, 496),
        );
    }

    #[test]
    fn toolbar_flip_compensates_position_and_uses_hysteresis() {
        let work_position = tauri::PhysicalPosition::new(0, 25);
        let work_size = tauri::PhysicalSize::new(1920, 1055);
        let horizontal_size = tauri::PhysicalSize::new(760, 156);
        let (placement, position) = toolbar_placement_after_move(
            OverlayOrientation::Horizontal,
            ToolbarPlacement::Top,
            tauri::PhysicalPosition::new(300, 25),
            horizontal_size,
            1.0,
            work_position,
            work_size,
        );
        assert_eq!(placement, ToolbarPlacement::Bottom);
        assert_eq!(position, tauri::PhysicalPosition::new(300, 71));
        assert_eq!(
            toolbar_placement_after_move(
                OverlayOrientation::Horizontal,
                placement,
                position,
                horizontal_size,
                1.0,
                work_position,
                work_size,
            ),
            (placement, position),
        );
        assert_eq!(
            toolbar_placement_after_move(
                OverlayOrientation::Horizontal,
                placement,
                tauri::PhysicalPosition::new(300, 84),
                horizontal_size,
                1.0,
                work_position,
                work_size,
            ),
            (placement, tauri::PhysicalPosition::new(300, 84)),
        );

        let vertical_size = tauri::PhysicalSize::new(380, 1240);
        let (placement, position) = toolbar_placement_after_move(
            OverlayOrientation::Vertical,
            ToolbarPlacement::Right,
            tauri::PhysicalPosition::new(1540, 100),
            vertical_size,
            2.0,
            tauri::PhysicalPosition::new(0, 0),
            tauri::PhysicalSize::new(1920, 2160),
        );
        assert_eq!(placement, ToolbarPlacement::Left);
        assert_eq!(position, tauri::PhysicalPosition::new(1444, 100));
        assert_eq!(
            toolbar_placement_after_move(
                OverlayOrientation::Vertical,
                placement,
                tauri::PhysicalPosition::new(1431, 100),
                vertical_size,
                2.0,
                tauri::PhysicalPosition::new(0, 0),
                tauri::PhysicalSize::new(1920, 2160),
            ),
            (placement, tauri::PhysicalPosition::new(1431, 100)),
        );
    }

    #[test]
    fn point_in_window_bounds_uses_exclusive_right_and_bottom_edges() {
        let position = tauri::PhysicalPosition::new(100, 200);
        let size = tauri::PhysicalSize::new(28, 28);
        assert!(point_in_window_bounds(
            tauri::PhysicalPosition::new(100.0, 200.0),
            position,
            size,
        ));
        assert!(point_in_window_bounds(
            tauri::PhysicalPosition::new(127.9, 227.9),
            position,
            size,
        ));
        assert!(!point_in_window_bounds(
            tauri::PhysicalPosition::new(128.0, 228.0),
            position,
            size,
        ));
    }

    #[test]
    fn overlay_hover_requires_visible_unlocked_overlay() {
        let cursor = tauri::PhysicalPosition::new(110.0, 210.0);
        let position = tauri::PhysicalPosition::new(100, 200);
        let size = tauri::PhysicalSize::new(28, 28);
        let mut settings = OverlaySettings::default();

        assert!(should_hover_overlay(&settings, cursor, position, size));

        settings.visible = false;
        assert!(!should_hover_overlay(&settings, cursor, position, size));

        settings.visible = true;
        settings.locked = true;
        assert!(!should_hover_overlay(&settings, cursor, position, size));
    }

    #[test]
    fn overlay_visibility_respects_preference_and_playback_state() {
        assert!(!should_show_overlay(false, false, false));
        assert!(!should_show_overlay(false, false, true));
        assert!(!should_show_overlay(false, true, false));
        assert!(!should_show_overlay(false, true, true));
        assert!(should_show_overlay(true, false, false));
        assert!(should_show_overlay(true, false, true));
        assert!(!should_show_overlay(true, true, false));
        assert!(should_show_overlay(true, true, true));
    }

    #[test]
    fn overlay_hover_uses_window_bounds() {
        let settings = OverlaySettings::default();
        let position = tauri::PhysicalPosition::new(100, 200);
        let size = tauri::PhysicalSize::new(28, 28);

        assert!(should_hover_overlay(
            &settings,
            tauri::PhysicalPosition::new(127.9, 227.9),
            position,
            size,
        ));
        assert!(!should_hover_overlay(
            &settings,
            tauri::PhysicalPosition::new(128.0, 228.0),
            position,
            size,
        ));
    }
}
