use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use super::platform::{
    apply_lyrics_window_space_behavior, enable_notch_window_behavior,
    refresh_overlay_mouse_tracking,
};
use crate::{AppState, NotchLayoutMetrics, UiLanguage};

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
            .and_then(|value| {
                value
                    .downcast_ref::<NSNumber>()
                    .map(NSNumber::unsignedIntValue)
            })
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
        return Err(
            std::io::Error::other("macOS window frame must be updated on the main thread").into(),
        );
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
pub(crate) fn set_window_frame(
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
pub(crate) fn set_window_frame(
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

pub(super) fn schedule_notch_position(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let target = window.clone();
    let handle = app.clone();
    if let Err(error) = window.run_on_main_thread(move || position_notch_window(&handle, &target)) {
        log::warn!("Failed to schedule Dynamic Island lyrics positioning: {error}");
    }
}

pub(super) fn create_notch_lyrics_window(app: &tauri::AppHandle) -> tauri::Result<()> {
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
