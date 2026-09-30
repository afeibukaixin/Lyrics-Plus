use crate::overlay_placement::geometry::{centered_position, monitor_contains_point};
use crate::{
    setup_tray, sync_app_menu_bar_icon_visibility, AppState, SurfaceReopenRequest,
    SurfaceRuntimeState,
};
use tauri::{Manager, WebviewWindowBuilder};

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

pub(crate) fn show_main_window_at(
    app: &tauri::AppHandle,
    route: Option<&str>,
) -> Result<(), String> {
    let runtime_started = app.try_state::<AppState>().is_some_and(|state| {
        *state
            .runtime_started
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    });
    if runtime_started {
        setup_tray(app).map_err(|error| error.to_string())?;
        sync_app_menu_bar_icon_visibility(app)?;
    }

    if crate::prepare_surface_show(
        app,
        "main",
        SurfaceReopenRequest::Main {
            route: route.map(str::to_owned),
        },
    ) {
        return Ok(());
    }

    let existing = app.get_webview_window("main");
    let opening = existing.as_ref().is_none_or(|window| {
        !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(false)
    });
    let window = if let Some(window) = existing.as_ref() {
        window.clone()
    } else {
        let path = route
            .map(|route| format!("index.html{route}"))
            .unwrap_or_else(|| "index.html".to_string());
        let title = crate::daily_quote::base_window_title();
        WebviewWindowBuilder::new(app, "main", crate::webview_url(app, &path))
            .title(title)
            .inner_size(980.0, 720.0)
            .min_inner_size(760.0, 560.0)
            .resizable(false)
            .maximizable(false)
            .visible(false)
            .build()
            .map_err(|error| error.to_string())?
    };
    if existing.is_some() {
        if let Some(route) = route {
            window
                .eval(format!("window.location.hash = {route:?}"))
                .map_err(|error| error.to_string())?;
        }
    }
    if !window.is_visible().unwrap_or(false) {
        if let Err(error) = center_main_window_on_cursor(app, &window) {
            log::warn!("Failed to center the main window; using its current position: {error}");
        }
    }
    if let Err(error) = window.unminimize() {
        log::warn!("Failed to unminimize the main window: {error}");
    }
    if let Some(state) = app.try_state::<AppState>() {
        let enabled = state.config.snapshot().app.daily_quote.is_enabled();
        if let Err(error) = window.set_title(&state.daily_quote.window_title(enabled)) {
            log::warn!("Failed to apply the cached daily quote: {error}");
        }
    }
    window.show().map_err(|error| error.to_string())?;
    if opening {
        crate::daily_quote::on_main_window_opened(app);
    }
    crate::set_surface_runtime_state(app, &window, SurfaceRuntimeState::Active);
    window.set_focus().map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn mark_overlay_programmatic_position(
    app: &tauri::AppHandle,
    position: tauri::PhysicalPosition<i32>,
) {
    if let Some(placements) = app.try_state::<crate::window_placement::LyricsWindowPlacements>() {
        placements
            .overlay
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .mark_programmatic(position);
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn mark_overlay_programmatic_position(
    _app: &tauri::AppHandle,
    _position: tauri::PhysicalPosition<i32>,
) {
}

pub(crate) fn set_overlay_position(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) {
    mark_overlay_programmatic_position(app, position);
    let _ = window.set_position(position);
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
