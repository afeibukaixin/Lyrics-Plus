fn set_surface_runtime_state(
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

fn is_managed_surface_label(label: &str) -> bool {
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

fn surface_is_destroying(app: &tauri::AppHandle, label: &str) -> bool {
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
fn configure_web_content_process_handler(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    builder.on_web_content_process_terminate(handle_web_content_process_terminated)
}

#[cfg(not(target_os = "macos"))]
fn configure_web_content_process_handler(
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

fn prepare_surface_show(
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

fn toggle_quick_lyrics_reopen_while_destroying(app: &tauri::AppHandle) -> bool {
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
            lifecycle.pending_reopen.insert(
                "quick-lyrics".to_owned(),
                SurfaceReopenRequest::QuickLyrics,
            );
            true
        }
    };
    log::debug!(
        "Updated quick lyrics reopen intent during destruction: enabled={pending_reopen}"
    );
    true
}

fn finish_surface_destroy(
    app: &tauri::AppHandle,
    label: &str,
) -> Option<SurfaceReopenRequest> {
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

fn reopen_surface_after_destroy(
    app: &tauri::AppHandle,
    reopen_request: SurfaceReopenRequest,
) {
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
        let next_generation = lifecycle
            .generations
            .entry(label.to_owned())
            .or_insert(0);
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

#[cfg(target_os = "macos")]
fn apply_joining_other_apps_fullscreen_on_main(
    window: &tauri::WebviewWindow,
) -> tauri::Result<()> {
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
    let original_behavior = ns_window.collectionBehavior();
    let mut behavior = original_behavior;
    behavior.remove(
        NSWindowCollectionBehavior::CanJoinAllApplications
            | NSWindowCollectionBehavior::Primary
            | NSWindowCollectionBehavior::Auxiliary
            | NSWindowCollectionBehavior::FullScreenPrimary
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::FullScreenNone,
    );
    // CanJoinAllApplications 允许悬浮窗加入其他应用，而 FullScreenAuxiliary 明确允许
    // 它进入其他应用占用的全屏 Space；二者结合后才能跨显示器拖入全屏应用。
    behavior.insert(
        NSWindowCollectionBehavior::CanJoinAllApplications
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    if behavior != original_behavior {
        ns_window.setCollectionBehavior(behavior);
    }
    Ok(())
}

const MIN_VERTICAL_HOST_WIDTH: f64 = 49.0;

#[cfg(target_os = "macos")]
fn run_window_collection_behavior_update(
    window: &tauri::WebviewWindow,
    operation: impl FnOnce(&tauri::WebviewWindow) -> tauri::Result<()> + Send + 'static,
) -> tauri::Result<()> {
    use objc2::MainThreadMarker;

    if MainThreadMarker::new().is_some() {
        return operation(window);
    }

    // Playback monitoring can reconcile visibility off the main thread. AppKit
    // collection behavior must still be changed on the main thread.
    let target = window.clone();
    let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(1);
    window.run_on_main_thread(move || {
        let result = operation(&target).map_err(|error| error.to_string());
        let _ = result_sender.send(result);
    })?;
    match result_receiver.recv() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(std::io::Error::other(error).into()),
        Err(error) => Err(std::io::Error::other(format!(
            "macOS window behavior update was interrupted: {error}"
        ))
        .into()),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn apply_joining_other_apps_fullscreen(
    window: &tauri::WebviewWindow,
) -> tauri::Result<()> {
    run_window_collection_behavior_update(window, apply_joining_other_apps_fullscreen_on_main)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apply_joining_other_apps_fullscreen(
    _window: &tauri::WebviewWindow,
) -> tauri::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn apply_lyrics_window_space_behavior_on_main(
    window: &tauri::WebviewWindow,
    enabled: bool,
) -> tauri::Result<()> {
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
    let original_behavior = ns_window.collectionBehavior();
    let mut behavior = original_behavior;
    // Managed 与 Transient 同时决定窗口参与 Spaces 和 Mission Control 的方式；
    // 先清理互斥的 Space 行为，再按统一设置选择最终模式。
    behavior.remove(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::MoveToActiveSpace
            | NSWindowCollectionBehavior::Managed
            | NSWindowCollectionBehavior::Transient
            | NSWindowCollectionBehavior::Stationary,
    );
    if enabled {
        behavior.insert(
            NSWindowCollectionBehavior::Managed
                | NSWindowCollectionBehavior::CanJoinAllSpaces,
        );
    } else {
        behavior.insert(NSWindowCollectionBehavior::Transient);
    }
    if behavior != original_behavior {
        ns_window.setCollectionBehavior(behavior);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn apply_lyrics_window_space_behavior(
    window: &tauri::WebviewWindow,
    enabled: bool,
) -> tauri::Result<()> {
    run_window_collection_behavior_update(window, move |target| {
        apply_lyrics_window_space_behavior_on_main(target, enabled)
    })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apply_lyrics_window_space_behavior(
    window: &tauri::WebviewWindow,
    enabled: bool,
) -> tauri::Result<()> {
    window.set_visible_on_all_workspaces(enabled)
}

pub(crate) fn apply_lyrics_windows_space_behavior(
    app: &tauri::AppHandle,
    enabled: bool,
) -> tauri::Result<()> {
    for label in ["lyrics-overlay", "lyrics-list", "lyrics-notch"] {
        if let Some(window) = app.get_webview_window(label) {
            apply_lyrics_window_space_behavior(&window, enabled)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn enable_notch_window_behavior(
    window: &tauri::WebviewWindow,
) -> tauri::Result<()> {
    use objc2_app_kit::{NSStatusWindowLevel, NSWindow};

    apply_joining_other_apps_fullscreen(window)?;
    let ns_window = window.ns_window()?;
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };
    ns_window.setLevel(NSStatusWindowLevel);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn enable_notch_window_behavior(
    window: &tauri::WebviewWindow,
) -> tauri::Result<()> {
    apply_joining_other_apps_fullscreen(window)
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
    let minimum_width = match style.orientation {
        OverlayOrientation::Vertical => MIN_VERTICAL_HOST_WIDTH,
        OverlayOrientation::Horizontal => 190.0,
    };

    let window = WebviewWindowBuilder::new(
        app,
        "lyrics-overlay",
        WebviewUrl::App("index.html?view=overlay".into()),
    )
    .title(UiLanguage::ZhCn.native_labels().overlay_title)
    .inner_size(initial_width, initial_height)
    .min_inner_size(minimum_width, 76.0)
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

    apply_joining_other_apps_fullscreen(&window)?;
    let enabled = app
        .state::<AppState>()
        .config
        .snapshot()
        .app
        .lyrics_windows_show_on_all_spaces;
    apply_lyrics_window_space_behavior(&window, enabled)?;
    refresh_overlay_mouse_tracking(&window);
    sync_overlay_vibrancy(&window, &style);

    Ok(())
}

pub(crate) fn show_quick_lyrics_window(app: &tauri::AppHandle) -> Result<(), String> {
    if prepare_surface_show(
        app,
        "quick-lyrics",
        SurfaceReopenRequest::QuickLyrics,
    ) {
        return Ok(());
    }
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
        set_surface_runtime_state(app, &window, SurfaceRuntimeState::Active);
        window.set_focus().map_err(|error| error.to_string())?;
        if let Err(error) = window.emit(QUICK_LYRICS_REFRESH_EVENT, ()) {
            log::debug!("Failed to request quick lyrics refresh: {error}");
        }
        return Ok(());
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
    .visible(false)
    .build()
    .map_err(|error| error.to_string())?;

    window.show().map_err(|error| error.to_string())?;
    set_surface_runtime_state(app, &window, SurfaceRuntimeState::Active);
    window.set_focus().map_err(|error| error.to_string())
}

pub(crate) fn toggle_quick_lyrics_window(app: &tauri::AppHandle) -> Result<(), String> {
    if toggle_quick_lyrics_reopen_while_destroying(app) {
        return Ok(());
    }
    if let Some(window) = app.get_webview_window("quick-lyrics") {
        if window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false) {
            hide_surface(app, "quick-lyrics")?;
            schedule_surface_destroy(app, "quick-lyrics");
            return Ok(());
        }
    }
    show_quick_lyrics_window(app)
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
    let locked = app
        .state::<AppState>()
        .config
        .snapshot()
        .lyrics
        .displays
        .list_window
        .locked;
    let window = WebviewWindowBuilder::new(
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
    let enabled = app
        .state::<AppState>()
        .config
        .snapshot()
        .app
        .lyrics_windows_show_on_all_spaces;
    apply_lyrics_window_space_behavior(&window, enabled)?;
    apply_list_lyrics_window_lock(app, locked).map_err(std::io::Error::other)?;
    Ok(())
}

pub(crate) fn apply_list_lyrics_window_lock(
    app: &tauri::AppHandle,
    locked: bool,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("lyrics-list") {
        window
            .set_ignore_cursor_events(locked)
            .map_err(|error| error.to_string())?;
        window
            .set_focusable(!locked)
            .map_err(|error| error.to_string())?;
        window
            .set_resizable(!locked)
            .map_err(|error| error.to_string())?;
        if !locked {
            refresh_overlay_mouse_tracking(&window);
        }
    }
    sync_list_unlock_handle(app);
    Ok(())
}

pub(crate) fn reset_list_lyrics_window_size(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("lyrics-list")
        .ok_or_else(|| "歌词窗口不存在".to_string())?;
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
    .title("Lyrics Plus 菜单栏歌词")
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
    apply_joining_other_apps_fullscreen(&window)?;
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
    use objc2_core_graphics::{CGDisplayBounds, CGDisplayPixelsHigh, CGDisplayPixelsWide};
    use objc2_foundation::{NSNumber, NSString};

    let Some(marker) = MainThreadMarker::new() else {
        return NotchLayoutMetrics::default();
    };
    let monitor_name = monitor.name().map(String::as_str);
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let screens = NSScreen::screens(marker);
    let screen_matches_monitor = |screen: &NSScreen| {
        let description = screen.deviceDescription();
        let screen_number_key = NSString::from_str("NSScreenNumber");
        let Some(display_id) = description
            .objectForKey(&screen_number_key)
            .and_then(|value| value.downcast_ref::<NSNumber>().map(NSNumber::unsignedIntValue))
        else {
            return false;
        };
        let bounds = CGDisplayBounds(display_id);
        let scale_factor = screen.backingScaleFactor();
        let x = (bounds.origin.x * scale_factor).round() as i32;
        let y = (bounds.origin.y * scale_factor).round() as i32;
        let width = (CGDisplayPixelsWide(display_id) as f64 * scale_factor).round() as u32;
        let height = (CGDisplayPixelsHigh(display_id) as f64 * scale_factor).round() as u32;

        monitor_position.x == x
            && monitor_position.y == y
            && monitor_size.width == width
            && monitor_size.height == height
    };
    let metrics_for = |screen: &NSScreen| {
        let top_inset = screen.safeAreaInsets().top.max(0.0);
        let left_area = screen.auxiliaryTopLeftArea();
        let right_area = screen.auxiliaryTopRightArea();
        let left_edge = left_area.origin.x + left_area.size.width;
        let center_gap_width = (right_area.origin.x - left_edge).max(0.0);
        let has_notch = top_inset > 0.0 && center_gap_width > 0.0;
        let scale_factor = monitor.scale_factor();
        let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor
        } else {
            1.0
        };
        let top_bar_height = f64::from(
            monitor
                .work_area()
                .position
                .y
                .saturating_sub(monitor_position.y)
                .max(0),
        ) / scale_factor;

        NotchLayoutMetrics {
            has_notch,
            top_inset: if has_notch { top_inset } else { top_bar_height },
            center_gap_width: if has_notch { center_gap_width } else { 0.0 },
        }
    };

    if let Some(screen) = screens.iter().find(|screen| screen_matches_monitor(screen)) {
        return metrics_for(&screen);
    }
    if let Some(screen) = screens
        .iter()
        .find(|screen| monitor_name.is_some_and(|name| screen.localizedName().to_string() == name))
    {
        return metrics_for(&screen);
    }
    let mut available = screens.iter();
    match (available.next(), available.next()) {
        (Some(screen), None) => metrics_for(&screen),
        _ => NotchLayoutMetrics::default(),
    }
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

pub(crate) fn notch_window_position(
    monitor: &tauri::Monitor,
    width: u32,
) -> tauri::PhysicalPosition<i32> {
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let x = monitor_position.x + monitor_size.width.saturating_sub(width) as i32 / 2;
    tauri::PhysicalPosition::new(x, monitor_position.y)
}

#[cfg(target_os = "macos")]
fn set_window_frame_on_main(
    window: &tauri::WebviewWindow,
    next_size: tauri::PhysicalSize<u32>,
    next_position: tauri::PhysicalPosition<i32>,
    scale: f64,
) -> tauri::Result<()> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSWindow;
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    if MainThreadMarker::new().is_none() {
        return Err(std::io::Error::other(
            "macOS window frame must be updated on the main thread",
        )
        .into());
    }

    let current_position = window.outer_position()?;
    let ns_window = window.ns_window()?;
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };
    let frame = ns_window.frame();
    let target_width = f64::from(next_size.width) / scale;
    let target_height = f64::from(next_size.height) / scale;
    let delta_x = (f64::from(next_position.x) - f64::from(current_position.x)) / scale;
    let delta_y = (f64::from(next_position.y) - f64::from(current_position.y)) / scale;
    let target_top = frame.origin.y + frame.size.height - delta_y;
    let target_frame = NSRect::new(
        NSPoint::new(frame.origin.x + delta_x, target_top - target_height),
        NSSize::new(target_width, target_height),
    );

    // AppKit 一次性更新尺寸和位置，避免先 set_size 后 set_position 产生可见的中心偏移。
    ns_window.setFrame_display(target_frame, true);
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_window_frame(
    window: &tauri::WebviewWindow,
    _current_size: tauri::PhysicalSize<u32>,
    _current_position: tauri::PhysicalPosition<i32>,
    next_size: tauri::PhysicalSize<u32>,
    next_position: tauri::PhysicalPosition<i32>,
    scale: f64,
) -> tauri::Result<()> {
    use objc2::MainThreadMarker;

    if MainThreadMarker::new().is_some() {
        return set_window_frame_on_main(window, next_size, next_position, scale);
    }

    let target = window.clone();
    let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(1);
    window.run_on_main_thread(move || {
        let result = set_window_frame_on_main(&target, next_size, next_position, scale)
            .map_err(|error| error.to_string());
        let _ = result_sender.send(result);
    })?;
    match result_receiver.recv() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(std::io::Error::other(error).into()),
        Err(error) => Err(std::io::Error::other(format!(
            "macOS window frame update was interrupted: {error}"
        ))
        .into()),
    }
}

#[cfg(not(target_os = "macos"))]
fn set_window_frame(
    window: &tauri::WebviewWindow,
    current_size: tauri::PhysicalSize<u32>,
    current_position: tauri::PhysicalPosition<i32>,
    next_size: tauri::PhysicalSize<u32>,
    next_position: tauri::PhysicalPosition<i32>,
    _scale: f64,
) -> tauri::Result<()> {
    if current_size != next_size {
        window.set_size(next_size)?;
    }
    if current_position != next_position {
        window.set_position(next_position)?;
    }
    Ok(())
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
    let metrics = screen_notch_layout(&monitor);
    let next_position = notch_window_position(&monitor, width);
    if window.outer_position().ok() != Some(next_position) {
        let _ = window.set_position(next_position);
    }
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
    // 宿主窗口固定为最大内容宽度加左右留白，实时预览只调整内部 Visual Island。
    let width = 656.0;
    let window = WebviewWindowBuilder::new(
        app,
        "lyrics-notch",
        WebviewUrl::App("index.html?view=lyrics-notch".into()),
    )
    .title(UiLanguage::ZhCn.native_labels().notch_title)
    .inner_size(width, 220.0)
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
    // 在 WebView 首次绘制前标记窗口类型，避免透明样式延迟生效造成黑色闪屏。
    .initialization_script("document.documentElement.dataset.window = 'lyrics-notch';")
    .visible(false)
    .build()?;
    // 窗口按展开态预留尺寸，WebView 挂载前必须先让透明区域穿透鼠标事件。
    if let Err(error) = window.set_ignore_cursor_events(true) {
        log::warn!("Failed to enable initial Dynamic Island pointer passthrough: {error}");
    }
    enable_notch_window_behavior(&window)?;
    let enabled = app
        .state::<AppState>()
        .config
        .snapshot()
        .app
        .lyrics_windows_show_on_all_spaces;
    apply_lyrics_window_space_behavior(&window, enabled)?;
    refresh_overlay_mouse_tracking(&window);
    schedule_notch_position(app, &window);
    Ok(())
}

// 该函数会创建窗口并调用 AppKit，只能由主线程入口调用。
fn reconcile_auxiliary_lyrics_windows(app: &tauri::AppHandle) -> Result<(), String> {
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

    let show_notch = displays.notch.enabled
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
                apply_joining_other_apps_fullscreen(&window)
                    .map_err(|error| error.to_string())?;
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
