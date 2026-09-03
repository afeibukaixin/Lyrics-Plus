use std::time::Duration;

use tauri::{Emitter, Manager};

use super::lifecycle::{
    cancel_surface_destroy, hide_surface, schedule_surface_destroy, set_surface_runtime_state,
    surface_is_destroying, SurfaceRuntimeState,
};
use super::list_lyrics::{apply_list_lyrics_window_lock, create_list_lyrics_window};
use super::notch::{create_notch_lyrics_window, schedule_notch_position};
use super::platform::{apply_joining_other_apps_fullscreen, apply_lyrics_window_space_behavior};
#[cfg(not(target_os = "macos"))]
use super::status_bar::{create_status_bar_lyrics_window, position_status_bar_window_default};
#[cfg(target_os = "macos")]
use crate::macos_status_item;
#[cfg(not(target_os = "macos"))]
use crate::TrayMenuState;
use crate::{
    sync_list_unlock_handle, sync_tray_lyrics_display_checked, wake_overlay_pointer_monitor,
    AppState,
};

const NOTCH_VISIBILITY_TRANSITION_EVENT: &str = "notch://visibility-transition";
const NOTCH_EXIT_ANIMATION_DURATION: Duration = Duration::from_millis(400);

#[derive(Clone, serde::Serialize)]
struct NotchVisibilityTransitionPayload {
    visible: bool,
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

// 该函数会创建窗口并调用 AppKit，只能由主线程入口调用。
pub(crate) fn reconcile_auxiliary_lyrics_windows(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let configured = state.config.snapshot();
    let displays = configured.lyrics.displays;
    let lyrics_windows_show_on_all_spaces = configured.app.lyrics_windows_show_on_all_spaces;
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
            cancel_surface_destroy(app, "lyrics-status-bar");
            if !surface_is_destroying(app, "lyrics-status-bar") {
                create_status_bar_lyrics_window(app).map_err(|error| error.to_string())?;
                if let Some(window) = app.get_webview_window("lyrics-status-bar") {
                    let appearance = &displays.status_bar.appearance;
                    let height = appearance.font_size as f64 + 12.0;
                    let _ = window.set_size(tauri::LogicalSize::new(
                        appearance.width as f64,
                        height.max(26.0),
                    ));
                    if !window.is_visible().unwrap_or(false) {
                        window.show().map_err(|error| error.to_string())?;
                        set_surface_runtime_state(app, &window, SurfaceRuntimeState::Active);
                    }
                }
            }
        } else if !displays.status_bar.enabled {
            hide_surface(app, "lyrics-status-bar")?;
            schedule_surface_destroy(app, "lyrics-status-bar");
        } else if let Some(window) = app.get_webview_window("lyrics-status-bar") {
            if window.is_visible().unwrap_or(false) {
                set_surface_runtime_state(app, &window, SurfaceRuntimeState::Dormant);
                window.hide().map_err(|error| error.to_string())?;
            }
        }
    }
    if displays.list_window.enabled {
        cancel_surface_destroy(app, "lyrics-list");
        if !surface_is_destroying(app, "lyrics-list") {
            create_list_lyrics_window(app).map_err(|error| error.to_string())?;
            if let Some(window) = app.get_webview_window("lyrics-list") {
                window
                    .set_always_on_top(displays.list_window.always_on_top)
                    .map_err(|error| error.to_string())?;
                apply_list_lyrics_window_lock(app, displays.list_window.locked)?;
                apply_lyrics_window_space_behavior(&window, lyrics_windows_show_on_all_spaces)
                    .map_err(|error| error.to_string())?;
                if !window.is_visible().unwrap_or(false) {
                    window.show().map_err(|error| error.to_string())?;
                    set_surface_runtime_state(app, &window, SurfaceRuntimeState::Active);
                }
                sync_list_unlock_handle(app);
            }
        }
    } else {
        hide_surface(app, "lyrics-list")?;
        hide_surface(app, "lyrics-list-unlock-handle")?;
        schedule_surface_destroy(app, "lyrics-list");
        schedule_surface_destroy(app, "lyrics-list-unlock-handle");
        if let Some(window) = app.get_webview_window("lyrics-list") {
            apply_lyrics_window_space_behavior(&window, lyrics_windows_show_on_all_spaces)
                .map_err(|error| error.to_string())?;
        }
        sync_list_unlock_handle(app);
    }

    let show_notch =
        displays.notch.enabled && (!displays.notch.hide_when_not_playing || playback.is_playing);
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
        cancel_surface_destroy(app, "lyrics-notch");
        if !surface_is_destroying(app, "lyrics-notch") {
            create_notch_lyrics_window(app).map_err(|error| error.to_string())?;
            if let Some(window) = app.get_webview_window("lyrics-notch") {
                let was_visible = window.is_visible().unwrap_or(false);
                apply_joining_other_apps_fullscreen(&window).map_err(|error| error.to_string())?;
                // 先恢复 Space 归属，再显示窗口，避免显示后才切换窗口管理模式。
                apply_lyrics_window_space_behavior(&window, lyrics_windows_show_on_all_spaces)
                    .map_err(|error| error.to_string())?;
                if !was_visible {
                    window.show().map_err(|error| error.to_string())?;
                    set_surface_runtime_state(app, &window, SurfaceRuntimeState::Active);
                    wake_overlay_pointer_monitor(app);
                }
                if visibility_changed || !was_visible {
                    let _ = window.emit(
                        NOTCH_VISIBILITY_TRANSITION_EVENT,
                        NotchVisibilityTransitionPayload { visible: true },
                    );
                }
                schedule_notch_position(app, &window);
            }
        }
    } else if visibility_changed {
        if let Some(window) = app.get_webview_window("lyrics-notch") {
            if !displays.notch.enabled {
                schedule_surface_destroy(app, "lyrics-notch");
            }
            state.spectrum.unsubscribe(app, "lyrics-notch");
            apply_lyrics_window_space_behavior(&window, lyrics_windows_show_on_all_spaces)
                .map_err(|error| error.to_string())?;
            if window.is_visible().unwrap_or(false) {
                // 退出动画期间窗口仍可见，运行时切换也要立即同步全屏行为。
                apply_joining_other_apps_fullscreen(&window).map_err(|error| error.to_string())?;
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
                        !visibility.target_visible && visibility.generation == visibility_generation
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
                    let should_still_hide = !displays.notch.enabled
                        || (displays.notch.hide_when_not_playing && !playback.is_playing);
                    if should_still_hide {
                        let handle_for_main = handle.clone();
                        if let Err(error) = handle.run_on_main_thread(move || {
                            if let Some(window) = handle_for_main.get_webview_window("lyrics-notch")
                            {
                                set_surface_runtime_state(
                                    &handle_for_main,
                                    &window,
                                    SurfaceRuntimeState::Dormant,
                                );
                                let _ = window.hide();
                                if !displays.notch.enabled {
                                    schedule_surface_destroy(&handle_for_main, "lyrics-notch");
                                }
                            }
                        }) {
                            log::warn!("Failed to finish Dynamic Island lyrics hiding: {error}");
                        }
                    }
                });
            } else {
                if !displays.notch.enabled {
                    schedule_surface_destroy(app, "lyrics-notch");
                }
                apply_lyrics_window_space_behavior(&window, lyrics_windows_show_on_all_spaces)
                    .map_err(|error| error.to_string())?;
            }
        }
    } else if let Some(window) = app.get_webview_window("lyrics-notch") {
        if !displays.notch.enabled {
            if !window.is_visible().unwrap_or(false) {
                schedule_surface_destroy(app, "lyrics-notch");
            }
        }
        apply_joining_other_apps_fullscreen(&window).map_err(|error| error.to_string())?;
        apply_lyrics_window_space_behavior(&window, lyrics_windows_show_on_all_spaces)
            .map_err(|error| error.to_string())?;
    }
    sync_tray_lyrics_display_checked(app);
    Ok(())
}

fn sync_lyrics_surfaces_on_main(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    {
        macos_status_item::sync(app);
        macos_status_item::wake(app);
    }
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
