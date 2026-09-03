use std::time::Duration;

use tauri::{Emitter, Manager};

use super::{show_quick_lyrics_window, sync_lyrics_surfaces};
use crate::{
    show_main_window_at, AppState, SurfaceReopenRequest, LIST_UNLOCK_HANDLE_HOVER_EVENT,
    UNLOCK_HANDLE_HOVER_EVENT,
};

const SURFACE_RUNTIME_STATE_EVENT: &str = "surface://runtime-state";

#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SurfaceRuntimeState {
    Active,
    Dormant,
}

pub(crate) fn set_surface_runtime_state(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    runtime_state: SurfaceRuntimeState,
) {
    if matches!(runtime_state, SurfaceRuntimeState::Dormant) {
        if let Some(state) = app.try_state::<AppState>() {
            state.spectrum.unsubscribe(app, window.label());
        }
    }
    // Emitter::emit 会广播给所有 WebView；生命周期必须只投递给目标窗口，
    // 否则一个歌词窗口休眠会清空其他窗口的本地播放与歌词状态。
    let target = tauri::EventTarget::webview_window(window.label());
    if let Err(error) = app.emit_to(target, SURFACE_RUNTIME_STATE_EVENT, runtime_state) {
        log::warn!(
            "Failed to update surface runtime state: label={}, state={runtime_state:?}, error={error}",
            window.label()
        );
    }
}

const SURFACE_DESTROY_DELAY: Duration = Duration::from_secs(3);
#[cfg(target_os = "macos")]
const SURFACE_TERMINATION_TIMEOUT: Duration = Duration::from_secs(1);

fn is_lyrics_surface_label(label: &str) -> bool {
    matches!(
        label,
        "lyrics-overlay"
            | "lyrics-unlock-handle"
            | "lyrics-list"
            | "lyrics-list-unlock-handle"
            | "lyrics-notch"
            | "lyrics-status-bar"
    )
}

pub(crate) fn is_managed_surface_label(label: &str) -> bool {
    is_lyrics_surface_label(label) || matches!(label, "main" | "quick-lyrics")
}

fn is_runtime_surface_label(label: &str) -> bool {
    matches!(
        label,
        "main"
            | "quick-lyrics"
            | "lyrics-overlay"
            | "lyrics-list"
            | "lyrics-notch"
            | "lyrics-status-bar"
    )
}

fn surface_should_be_destroyed(app: &tauri::AppHandle, label: &str) -> bool {
    let configured = app.state::<AppState>().config.snapshot();
    match label {
        "lyrics-overlay" | "lyrics-unlock-handle" => !configured.overlay.visible,
        "lyrics-list" | "lyrics-list-unlock-handle" => {
            !configured.lyrics.displays.list_window.enabled
        }
        "lyrics-notch" => !configured.lyrics.displays.notch.enabled,
        "lyrics-status-bar" => !configured.lyrics.displays.status_bar.enabled,
        // 主窗口和快速歌词没有配置开关；只有显式关闭入口会为它们安排销毁。
        "main" | "quick-lyrics" => true,
        _ => false,
    }
}

pub(crate) fn surface_is_destroying(app: &tauri::AppHandle, label: &str) -> bool {
    app.try_state::<AppState>().is_some_and(|state| {
        state
            .webview_surface_lifecycle
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .destroying
            .contains(label)
    })
}

fn app_shutdown_requested(app: &tauri::AppHandle) -> bool {
    app.try_state::<AppState>().is_some_and(|state| {
        state
            .webview_surface_lifecycle
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .shutdown_requested
    })
}

#[cfg(target_os = "macos")]
fn terminate_web_content_process(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use objc2::runtime::{AnyObject, Bool, Sel};
    use objc2::{msg_send, sel};

    let label = window.label().to_owned();
    window.with_webview(move |webview| unsafe {
        let webview = webview.inner() as *mut AnyObject;
        let selector: Sel = sel!(_killWebContentProcessAndResetState);
        let responds: Bool = msg_send![webview, respondsToSelector: selector];
        if responds.as_bool() {
            let _: () = msg_send![webview, _killWebContentProcessAndResetState];
            log::debug!("Requested WebContent process termination: label={label}");
        } else {
            log::warn!(
                "WebKit cannot terminate the WebContent process on this system: label={label}"
            );
        }
    })
}

#[cfg(target_os = "macos")]
fn schedule_surface_destroy_fallback(app: &tauri::AppHandle, label: &str) {
    let handle = app.clone();
    let label = label.to_owned();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SURFACE_TERMINATION_TIMEOUT).await;
        if !surface_is_destroying(&handle, &label) {
            return;
        }
        let handle_for_main = handle.clone();
        let label_for_log = label.clone();
        if let Err(error) = handle.run_on_main_thread(move || {
            log::warn!(
                "WebContent termination timed out; destroying its window: label={label}"
            );
            if let Err(error) = destroy_surface(&handle_for_main, &label) {
                let _ = finish_surface_destroy(&handle_for_main, &label);
                log::warn!(
                    "Destroying WebView after termination timeout failed: label={label}, error={error}"
                );
            }
        }) {
            let _ = finish_surface_destroy(&handle, &label_for_log);
            log::warn!(
                "Failed to schedule WebView termination fallback: label={label_for_log}, error={error}"
            );
        }
    });
}

#[cfg(target_os = "macos")]
fn handle_web_content_process_terminated(webview: &tauri::Webview) {
    let app = webview.app_handle();
    let label = webview.label();
    if is_managed_surface_label(label) && surface_is_destroying(app, label) {
        log::debug!("WebContent process terminated: label={label}");
        if let Err(error) = destroy_surface(app, label) {
            let _ = finish_surface_destroy(app, label);
            log::warn!(
                "Destroying window after WebContent termination failed: label={label}, error={error}"
            );
        }
        return;
    }

    // 保留 Tauri 的默认恢复行为：非主动销毁导致的内容进程退出仍自动重载页面。
    log::warn!("WebContent process terminated unexpectedly; reloading: label={label}");
    if let Err(error) = webview.reload() {
        log::warn!("Failed to reload terminated WebView: label={label}, error={error}");
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn configure_web_content_process_handler(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    builder.on_web_content_process_terminate(handle_web_content_process_terminated)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn configure_web_content_process_handler(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    builder
}

pub(crate) fn cancel_surface_destroy(app: &tauri::AppHandle, label: &str) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let cancelled = state
        .webview_surface_lifecycle
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .pending_destroy
        .remove(label)
        .is_some();
    if cancelled {
        log::debug!("Cancelled deferred WebView destruction: label={label}");
    }
}

pub(crate) fn prepare_surface_show(
    app: &tauri::AppHandle,
    label: &str,
    reopen_request: SurfaceReopenRequest,
) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };
    let (cancelled, destroying) = {
        let mut lifecycle = state
            .webview_surface_lifecycle
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let cancelled = lifecycle.pending_destroy.remove(label).is_some();
        let destroying = lifecycle.destroying.contains(label);
        if destroying {
            // 主窗口的最后一个路由请求会覆盖较早请求；快速歌词只需保留一次打开意图。
            lifecycle
                .pending_reopen
                .insert(label.to_owned(), reopen_request);
        } else {
            lifecycle.pending_reopen.remove(label);
        }
        (cancelled, destroying)
    };
    if cancelled {
        log::debug!("Cancelled deferred WebView destruction: label={label}");
    }
    if destroying {
        log::debug!("Deferred WebView reopen until destruction completes: label={label}");
    }
    destroying
}

pub(super) fn toggle_quick_lyrics_reopen_while_destroying(app: &tauri::AppHandle) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };
    let pending_reopen = {
        let mut lifecycle = state
            .webview_surface_lifecycle
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !lifecycle.destroying.contains("quick-lyrics") {
            return false;
        }
        if lifecycle.pending_reopen.remove("quick-lyrics").is_some() {
            false
        } else {
            lifecycle
                .pending_reopen
                .insert("quick-lyrics".to_owned(), SurfaceReopenRequest::QuickLyrics);
            true
        }
    };
    log::debug!("Updated quick lyrics reopen intent during destruction: enabled={pending_reopen}");
    true
}

fn finish_surface_destroy(app: &tauri::AppHandle, label: &str) -> Option<SurfaceReopenRequest> {
    let Some(state) = app.try_state::<AppState>() else {
        return None;
    };
    let (finished, reopen_request) = {
        let mut lifecycle = state
            .webview_surface_lifecycle
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        lifecycle.pending_destroy.remove(label);
        let finished = lifecycle.destroying.remove(label);
        let reopen_request = if finished {
            lifecycle.pending_reopen.remove(label)
        } else {
            None
        };
        (finished, reopen_request)
    };
    if finished {
        log::debug!("WebView destruction completed: label={label}");
    }
    reopen_request
}

fn reopen_surface_after_destroy(app: &tauri::AppHandle, reopen_request: SurfaceReopenRequest) {
    let result = match reopen_request {
        SurfaceReopenRequest::Main { route } => show_main_window_at(app, route.as_deref()),
        SurfaceReopenRequest::QuickLyrics => show_quick_lyrics_window(app),
    };
    if let Err(error) = result {
        log::warn!("Failed to reopen WebView after destruction: error={error}");
    }
}

pub(crate) fn handle_surface_destroyed(app: &tauri::AppHandle, label: &str) {
    let reopen_request = finish_surface_destroy(app, label);
    if app_shutdown_requested(app) {
        return;
    }
    if is_lyrics_surface_label(label) {
        sync_lyrics_surfaces(app);
    }
    if let Some(reopen_request) = reopen_request {
        reopen_surface_after_destroy(app, reopen_request);
    }
}

pub(crate) fn schedule_surface_destroy(app: &tauri::AppHandle, label: &str) {
    // 关闭先隐藏并让前端释放高频任务，3 秒后才销毁，给快速开关保留复用窗口的机会。
    if app.get_webview_window(label).is_none() {
        return;
    }
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let generation = {
        let mut lifecycle = state
            .webview_surface_lifecycle
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if lifecycle.destroying.contains(label) || lifecycle.pending_destroy.contains_key(label) {
            return;
        }
        let next_generation = lifecycle.generations.entry(label.to_owned()).or_insert(0);
        *next_generation = next_generation.wrapping_add(1);
        let generation = *next_generation;
        lifecycle
            .pending_destroy
            .insert(label.to_owned(), generation);
        generation
    };
    log::debug!(
        "Scheduled WebView destruction: label={label}, delay_ms={}",
        SURFACE_DESTROY_DELAY.as_millis()
    );

    let handle = app.clone();
    let label = label.to_owned();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SURFACE_DESTROY_DELAY).await;
        let handle_for_main = handle.clone();
        let label_for_log = label.clone();
        if let Err(error) = handle.run_on_main_thread(move || {
            if app_shutdown_requested(&handle_for_main) {
                cancel_surface_destroy(&handle_for_main, &label);
                return;
            }
            let Some(state) = handle_for_main.try_state::<AppState>() else {
                return;
            };
            let is_current = state
                .webview_surface_lifecycle
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .pending_destroy
                .get(&label)
                .is_some_and(|current| *current == generation);
            if !is_current {
                return;
            }
            if !surface_should_be_destroyed(&handle_for_main, &label) {
                cancel_surface_destroy(&handle_for_main, &label);
                return;
            }
            let Some(window) = handle_for_main.get_webview_window(&label) else {
                cancel_surface_destroy(&handle_for_main, &label);
                return;
            };
            if window.is_visible().unwrap_or(false) {
                log::debug!(
                    "Deferred WebView destruction skipped because the window is visible: label={label}"
                );
                cancel_surface_destroy(&handle_for_main, &label);
                return;
            }
            {
                let mut lifecycle = state
                    .webview_surface_lifecycle
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                lifecycle.pending_destroy.remove(&label);
                lifecycle.destroying.insert(label.clone());
            }
            log::debug!("Destroying WebView surface: label={label}");
            #[cfg(target_os = "macos")]
            {
                // macOS 会缓存已经关闭的页面进程，只有明确结束内容进程才能及时归还内存。
                if let Err(error) = terminate_web_content_process(&window) {
                    log::warn!(
                        "Failed to request WebContent process termination: label={label}, error={error}"
                    );
                } else {
                    schedule_surface_destroy_fallback(&handle_for_main, &label);
                    return;
                }
            }
            if let Err(error) = destroy_surface(&handle_for_main, &label) {
                let _ = finish_surface_destroy(&handle_for_main, &label);
                log::warn!("Destroying WebView surface failed: label={label}, error={error}");
            }
        }) {
            log::warn!(
                "Failed to schedule WebView destruction: label={label_for_log}, error={error}"
            );
        }
    });
}

pub(crate) fn hide_surface(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window(label) else {
        return Ok(());
    };
    if is_runtime_surface_label(label) {
        set_surface_runtime_state(app, &window, SurfaceRuntimeState::Dormant);
    }
    if window.is_visible().unwrap_or(false) {
        window.hide().map_err(|error| error.to_string())?;
    }
    match label {
        "lyrics-unlock-handle" => {
            let _ = window.emit(UNLOCK_HANDLE_HOVER_EVENT, false);
        }
        "lyrics-list-unlock-handle" => {
            let _ = window.emit(LIST_UNLOCK_HANDLE_HOVER_EVENT, false);
        }
        _ => {}
    }
    Ok(())
}

fn destroy_surface(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window(label) else {
        return Ok(());
    };
    if let Some(state) = app.try_state::<AppState>() {
        state.spectrum.unsubscribe(app, label);
    }
    log::debug!("Destroying WebView surface: label={label}");
    window.destroy().map_err(|error| error.to_string())
}
