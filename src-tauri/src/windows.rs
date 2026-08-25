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
    let appearance = app
        .state::<AppState>()
        .config
        .snapshot()
        .lyrics
        .displays
        .notch
        .appearance;
    // 创建时直接预留悬停展开宽度；前端挂载后会在允许交互前完成一次精确高度适配。
    let width = f64::from(appearance.max_width.max(appearance.expanded_max_width)) + 16.0;
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
    .visible(false)
    .build()?;
    // 窗口按展开态预留尺寸，WebView 挂载前必须先让透明区域穿透鼠标事件。
    if let Err(error) = window.set_ignore_cursor_events(true) {
        log::warn!("Failed to enable initial Dynamic Island pointer passthrough: {error}");
    }
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
