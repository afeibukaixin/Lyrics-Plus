#[tauri::command]
pub fn get_overlay_settings(state: State<'_, AppState>) -> OverlaySettings {
    get_overlay_settings_inner(&state)
}

pub fn update_overlay_locked(app: &tauri::AppHandle, locked: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("lyrics-overlay")
        .ok_or_else(|| "歌词浮窗不存在".to_string())?;
    let state = app.state::<AppState>();
    let previous_settings = {
        let mut settings = state
            .overlay_settings
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let previous = settings.clone();
        settings.locked = locked;
        previous
    };
    let update_result = (|| {
        if locked {
            let current_size = window.outer_size().map_err(|error| error.to_string())?;
            let scale = window.scale_factor().map_err(|error| error.to_string())?;
            let scale = if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            };
            let mut style = state
                .overlay_style
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            match style.orientation {
                OverlayOrientation::Horizontal => {
                    style.horizontal_max_width = Some(current_size.width as f64 / scale);
                }
                OverlayOrientation::Vertical => {
                    style.vertical_max_height = Some(current_size.height as f64 / scale);
                }
            }
            let style = style.normalized();
            *state
                .overlay_style
                .write()
                .unwrap_or_else(|error| error.into_inner()) = style.clone();
            persist_overlay_style_for_current_monitor(app, &state, &style)?;
        }
        window
            .set_ignore_cursor_events(locked)
            .map_err(|error| error.to_string())?;
        let _ = window.set_focusable(!locked);
        if !locked {
            crate::refresh_overlay_mouse_tracking(&window);
        }
        let _ = window.set_resizable(false);
        state
            .config
            .update(|config| config.overlay.locked = locked)?;
        crate::sync_unlock_handle(app);
        app.emit("overlay://settings", get_overlay_settings_inner(&state))
            .map_err(|error| error.to_string())
    })();
    if let Err(error) = update_result {
        *state
            .overlay_settings
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = previous_settings;
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub fn set_overlay_locked(app: tauri::AppHandle, locked: bool) -> Result<(), String> {
    update_overlay_locked(&app, locked)
}

#[tauri::command]
pub fn get_overlay_style(state: State<'_, AppState>) -> OverlayStyleSettings {
    state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

#[tauri::command]
pub fn get_overlay_toolbar_placement(state: State<'_, AppState>) -> crate::ToolbarPlacement {
    state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .toolbar_placement
}

fn persist_overlay_style_for_current_monitor(
    app: &tauri::AppHandle,
    state: &AppState,
    style: &OverlayStyleSettings,
) -> Result<(), String> {
    let monitor_id = app
        .get_webview_window("lyrics-overlay")
        .and_then(|window| window.current_monitor().ok().flatten())
        .map(|monitor| crate::monitor_id(&monitor));
    *state
        .overlay_monitor
        .write()
        .unwrap_or_else(|error| error.into_inner()) = monitor_id.clone();
    let key = monitor_id
        .map(|id| format!("overlay.geometry.{id}"))
        .unwrap_or_else(|| "overlay.geometry.default".into());
    let geometry = crate::StoredOverlayGeometry {
        horizontal_max_width: style.horizontal_max_width,
        vertical_max_height: style.vertical_max_height,
    };
    let raw =
        serde_json::to_string(&geometry).map_err(|error| format!("无法序列化浮窗尺寸：{error}"))?;
    state.storage.set_preference(&key, &raw)?;
    state
        .config
        .update(|config| config.overlay.appearance = OverlayAppearance::from(style))?;
    if let Some(window) = app.get_webview_window("lyrics-overlay") {
        crate::sync_overlay_vibrancy(&window, style);
    }
    app.emit("overlay://style", style)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_overlay_style(
    app: tauri::AppHandle,
    style: OverlayStyleSettings,
    state: State<'_, AppState>,
) -> Result<OverlayStyleSettings, String> {
    let style = style.normalized();
    let previous_orientation = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .orientation;
    *state
        .overlay_style
        .write()
        .unwrap_or_else(|error| error.into_inner()) = style.clone();
    if previous_orientation != style.orientation {
        crate::reset_overlay_toolbar_placement(&app, style.orientation);
    }
    persist_overlay_style_for_current_monitor(&app, &state, &style)?;
    crate::sync_unlock_handle(&app);
    Ok(style)
}

#[tauri::command]
pub fn nudge_overlay(app: tauri::AppHandle, dx: i32, dy: i32) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .locked
    {
        return Err("请先解锁歌词浮窗".into());
    }
    let window = app
        .get_webview_window("lyrics-overlay")
        .ok_or_else(|| "歌词浮窗不存在".to_string())?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    window
        .set_position(tauri::PhysicalPosition::new(
            position.x.saturating_add(dx.clamp(-20, 20)),
            position.y.saturating_add(dy.clamp(-20, 20)),
        ))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reset_overlay_bounds(app: tauri::AppHandle) -> Result<OverlayStyleSettings, String> {
    let state = app.state::<AppState>();
    let locked = state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .locked;
    let window = match app.get_webview_window("lyrics-overlay") {
        Some(window) => window,
        None => {
            crate::create_overlay(&app).map_err(|error| error.to_string())?;
            app.get_webview_window("lyrics-overlay")
                .ok_or_else(|| "无法创建歌词浮窗".to_string())?
        }
    };
    let (current_width, current_height) = window
        .outer_size()
        .ok()
        .and_then(|size| {
            let scale = window.scale_factor().ok()?;
            let scale = if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            };
            Some((size.width as f64 / scale, size.height as f64 / scale))
        })
        .unwrap_or((190.0, 156.0));
    let style = {
        let mut current = state
            .overlay_style
            .write()
            .unwrap_or_else(|error| error.into_inner());
        clear_manual_overlay_bounds(&mut current);
        current.clone()
    };
    state
        .storage
        .remove_preferences_with_prefix("overlay.position.")?;
    state
        .storage
        .remove_preferences_with_prefix("overlay.geometry.")?;
    state.storage.remove_preference("overlay.last_monitor")?;
    *state
        .overlay_monitor
        .write()
        .unwrap_or_else(|error| error.into_inner()) = None;
    state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .preferred_monitor = None;
    let (reset_width, reset_height) =
        reset_overlay_dimensions(style.orientation, current_width, current_height);
    window
        .set_size(tauri::LogicalSize::new(reset_width, reset_height))
        .map_err(|error| error.to_string())?;
    window
        .set_ignore_cursor_events(locked)
        .map_err(|error| error.to_string())?;
    let _ = window.set_focusable(!locked);
    if !locked {
        crate::refresh_overlay_mouse_tracking(&window);
    }
    let _ = window.set_resizable(false);
    crate::move_overlay_to_primary(&app, &window);
    persist_overlay_style_for_current_monitor(&app, &state, &style)?;
    state
        .overlay_settings
        .write()
        .unwrap_or_else(|error| error.into_inner())
        .visible = true;
    state
        .config
        .update(|config| config.overlay.visible = true)?;
    crate::sync_tray_overlay_checked(&app, true);
    crate::reconcile_overlay_visibility(&app)?;
    app.emit("overlay://settings", get_overlay_settings_inner(&state))
        .map_err(|error| error.to_string())?;
    Ok(style)
}

fn clear_manual_overlay_bounds(style: &mut OverlayStyleSettings) {
    style.horizontal_max_width = None;
    style.vertical_max_height = None;
}

fn reset_overlay_dimensions(
    orientation: OverlayOrientation,
    current_width: f64,
    current_height: f64,
) -> (f64, f64) {
    match orientation {
        OverlayOrientation::Horizontal => (760.0, current_height.max(76.0)),
        OverlayOrientation::Vertical => (current_width.max(190.0), 620.0),
    }
}

fn resize_overlay_edge_bounds(
    position: tauri::PhysicalPosition<i32>,
    current_size: tauri::PhysicalSize<u32>,
    edge: OverlayResizeEdge,
    requested_main_size: f64,
    minimum_main_size: f64,
    scale: f64,
    monitor_position: tauri::PhysicalPosition<i32>,
    monitor_size: tauri::PhysicalSize<u32>,
) -> (tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let margin = 0_i64;
    let minimum_main_size = if minimum_main_size.is_finite() {
        minimum_main_size.max(0.0)
    } else {
        0.0
    };
    let work_left = monitor_position.x as i64 + margin;
    let work_top = monitor_position.y as i64 + margin;
    let work_right = monitor_position.x as i64 + monitor_size.width as i64 - margin;
    let work_bottom = monitor_position.y as i64 + monitor_size.height as i64 - margin;
    let available_width = (work_right - work_left).max(1) as u32;
    let available_height = (work_bottom - work_top).max(1) as u32;
    let minimum_width =
        ((minimum_main_size.max(320.0) * scale).round() as u32).min(available_width);
    let minimum_height =
        ((minimum_main_size.max(280.0) * scale).round() as u32).min(available_height);
    let fallback_size = match edge {
        OverlayResizeEdge::Left | OverlayResizeEdge::Right => current_size.width,
        OverlayResizeEdge::Top | OverlayResizeEdge::Bottom => current_size.height,
    };
    let requested = if requested_main_size.is_finite() {
        (requested_main_size.max(0.0) * scale).round() as u32
    } else {
        fallback_size
    };

    match edge {
        OverlayResizeEdge::Left => {
            let fixed_right = (position.x as i64 + current_size.width as i64)
                .clamp(work_left + minimum_width as i64, work_right);
            let maximum_width = (fixed_right - work_left) as u32;
            let width = requested.clamp(minimum_width, maximum_width.max(minimum_width));
            (
                tauri::PhysicalPosition::new((fixed_right - width as i64) as i32, position.y),
                tauri::PhysicalSize::new(width, current_size.height),
            )
        }
        OverlayResizeEdge::Right => {
            let fixed_left =
                (position.x as i64).clamp(work_left, work_right - minimum_width as i64);
            let maximum_width = (work_right - fixed_left) as u32;
            let width = requested.clamp(minimum_width, maximum_width.max(minimum_width));
            (
                tauri::PhysicalPosition::new(fixed_left as i32, position.y),
                tauri::PhysicalSize::new(width, current_size.height),
            )
        }
        OverlayResizeEdge::Top => {
            let fixed_bottom = (position.y as i64 + current_size.height as i64)
                .clamp(work_top + minimum_height as i64, work_bottom);
            let maximum_height = (fixed_bottom - work_top) as u32;
            let height = requested.clamp(minimum_height, maximum_height.max(minimum_height));
            (
                tauri::PhysicalPosition::new(position.x, (fixed_bottom - height as i64) as i32),
                tauri::PhysicalSize::new(current_size.width, height),
            )
        }
        OverlayResizeEdge::Bottom => {
            let fixed_top =
                (position.y as i64).clamp(work_top, work_bottom - minimum_height as i64);
            let maximum_height = (work_bottom - fixed_top) as u32;
            let height = requested.clamp(minimum_height, maximum_height.max(minimum_height));
            (
                tauri::PhysicalPosition::new(position.x, fixed_top as i32),
                tauri::PhysicalSize::new(current_size.width, height),
            )
        }
    }
}

#[tauri::command]
pub fn resize_overlay_edge(
    app: tauri::AppHandle,
    edge: OverlayResizeEdge,
    main_size: f64,
    minimum_main_size: f64,
    state: State<'_, AppState>,
) -> Result<OverlayResizeBounds, String> {
    if state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .locked
    {
        return Err("请先解锁歌词浮窗".into());
    }
    let window = app
        .get_webview_window("lyrics-overlay")
        .ok_or_else(|| "歌词浮窗不存在".to_string())?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let current_size = window.outer_size().map_err(|error| error.to_string())?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or_else(|| "无法读取显示器信息".to_string())?;
    let work_area = monitor.work_area();
    let (next_position, next_size) = resize_overlay_edge_bounds(
        position,
        current_size,
        edge,
        main_size,
        minimum_main_size,
        scale,
        work_area.position,
        work_area.size,
    );
    if current_size != next_size {
        window
            .set_size(next_size)
            .map_err(|error| error.to_string())?;
    }
    if position != next_position {
        window
            .set_position(next_position)
            .map_err(|error| error.to_string())?;
    }
    let applied = window.outer_size().unwrap_or(next_size);
    crate::sync_unlock_handle(&app);
    Ok(OverlayResizeBounds {
        width: applied.width as f64 / scale,
        height: applied.height as f64 / scale,
    })
}

fn fixed_axis_content_size(
    style: &OverlayStyleSettings,
    requested_width: f64,
    requested_height: f64,
    current_width: f64,
    current_height: f64,
    locked: bool,
) -> (f64, f64) {
    match style.orientation {
        OverlayOrientation::Horizontal => (
            if locked {
                current_width
            } else {
                style.horizontal_max_width.unwrap_or(760.0)
            },
            requested_height,
        ),
        OverlayOrientation::Vertical => (
            requested_width,
            if locked {
                current_height
            } else {
                style.vertical_max_height.unwrap_or(620.0)
            },
        ),
    }
}

fn fit_overlay_bounds(
    position: tauri::PhysicalPosition<i32>,
    requested_width: f64,
    requested_height: f64,
    scale: f64,
    monitor_position: tauri::PhysicalPosition<i32>,
    monitor_size: tauri::PhysicalSize<u32>,
) -> (tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let margin = 0_u32;
    let minimum_width = (190.0 * scale).round() as u32;
    let minimum_height = (76.0 * scale).round() as u32;
    let maximum_width = monitor_size
        .width
        .saturating_sub(margin.saturating_mul(2))
        .max(minimum_width);
    let maximum_height = monitor_size
        .height
        .saturating_sub(margin.saturating_mul(2))
        .max(minimum_height);
    let requested_width = if requested_width.is_finite() {
        (requested_width.max(0.0) * scale).round() as u32
    } else {
        minimum_width
    };
    let requested_height = if requested_height.is_finite() {
        (requested_height.max(0.0) * scale).round() as u32
    } else {
        minimum_height
    };
    let size = tauri::PhysicalSize::new(
        requested_width.clamp(minimum_width, maximum_width),
        requested_height.clamp(minimum_height, maximum_height),
    );

    let minimum_x = monitor_position.x as i64 + margin as i64;
    let minimum_y = monitor_position.y as i64 + margin as i64;
    let maximum_x =
        monitor_position.x as i64 + monitor_size.width as i64 - margin as i64 - size.width as i64;
    let maximum_y =
        monitor_position.y as i64 + monitor_size.height as i64 - margin as i64 - size.height as i64;
    let x = (position.x as i64).clamp(minimum_x, maximum_x.max(minimum_x));
    let y = (position.y as i64).clamp(minimum_y, maximum_y.max(minimum_y));

    (tauri::PhysicalPosition::new(x as i32, y as i32), size)
}

#[tauri::command]
pub fn fit_overlay_content(app: tauri::AppHandle, width: f64, height: f64) -> Result<bool, String> {
    let window = app
        .get_webview_window("lyrics-overlay")
        .ok_or_else(|| "歌词浮窗不存在".to_string())?;
    if crate::primary_mouse_button_pressed() {
        return Ok(false);
    }
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let current_size = window.outer_size().map_err(|error| error.to_string())?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or_else(|| "无法读取显示器信息".to_string())?;
    let work_area = monitor.work_area();
    let state = app.state::<AppState>();
    let style = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let locked = state
        .overlay_settings
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .locked;
    let current_width = current_size.width as f64 / scale;
    let current_height = current_size.height as f64 / scale;
    let (width, height) =
        fixed_axis_content_size(&style, width, height, current_width, current_height, locked);
    let (next_position, next_size) = fit_overlay_bounds(
        position,
        width,
        height,
        scale,
        work_area.position,
        work_area.size,
    );
    let size_changed = current_size.width.abs_diff(next_size.width) > 2
        || current_size.height.abs_diff(next_size.height) > 2;
    if size_changed {
        window
            .set_size(next_size)
            .map_err(|error| error.to_string())?;
    }
    if position != next_position {
        window
            .set_position(next_position)
            .map_err(|error| error.to_string())?;
    }
    crate::sync_unlock_handle(&app);
    Ok(true)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotchWindowFitResult {
    pub physical_width: u32,
    pub physical_height: u32,
    pub size_changed: bool,
}

#[tauri::command]
pub fn fit_notch_lyrics_content(
    app: tauri::AppHandle,
    width: f64,
    height: f64,
) -> Result<NotchWindowFitResult, String> {
    let window = app
        .get_webview_window("lyrics-notch")
        .ok_or_else(|| "灵动岛歌词窗口不存在".to_string())?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or_else(|| "无法读取灵动岛歌词所在的显示器".to_string())?;
    let monitor_size = monitor.size();
    let requested_width = if width.is_finite() {
        width.max(120.0)
    } else {
        120.0
    };
    let requested_height = if height.is_finite() {
        height.max(44.0)
    } else {
        44.0
    };
    let next_size = tauri::PhysicalSize::new(
        ((requested_width * scale).round() as u32).min(monitor_size.width),
        ((requested_height * scale).round() as u32).min(monitor_size.height),
    );
    let next_position = crate::notch_window_position(&monitor, next_size.width);
    let current_size = window.outer_size().map_err(|error| error.to_string())?;
    let current_position = window.outer_position().map_err(|error| error.to_string())?;
    let size_changed = current_size.width.abs_diff(next_size.width) > 1
        || current_size.height.abs_diff(next_size.height) > 1;
    if size_changed {
        window
            .set_size(next_size)
            .map_err(|error| error.to_string())?;
    }
    if current_position != next_position {
        window
            .set_position(next_position)
            .map_err(|error| error.to_string())?;
    }
    if size_changed {
        crate::refresh_overlay_mouse_tracking(&window);
    }
    Ok(NotchWindowFitResult {
        physical_width: next_size.width,
        physical_height: next_size.height,
        size_changed,
    })
}

#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::show_main_window_centered(&app)
}

#[tauri::command]
pub fn show_lyrics_style_settings(
    app: tauri::AppHandle,
    mode: LyricsStyleMode,
) -> Result<(), String> {
    let mode = match mode {
        LyricsStyleMode::Desktop => "desktop",
        LyricsStyleMode::StatusBar => "statusBar",
        LyricsStyleMode::ListWindow => "listWindow",
        LyricsStyleMode::Notch => "notch",
    };
    let route = format!("#/settings/style?mode={mode}");
    crate::show_main_window_at(&app, Some(&route))
}

#[tauri::command]
pub fn show_quick_lyrics_window(app: tauri::AppHandle) -> Result<(), String> {
    crate::show_quick_lyrics_window(&app)
}
