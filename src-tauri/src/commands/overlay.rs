#[tauri::command]
pub fn get_overlay_settings(state: State<'_, AppState>) -> OverlaySettings {
    get_overlay_settings_inner(&state)
}

pub fn update_overlay_locked(app: &tauri::AppHandle, locked: bool) -> Result<(), String> {
    let window = app.get_webview_window("lyrics-overlay");
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
            if let Some(window) = window.as_ref() {
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
        }
        if let Some(window) = window.as_ref() {
            window
                .set_ignore_cursor_events(locked)
                .map_err(|error| error.to_string())?;
            let _ = window.set_focusable(!locked);
            if !locked {
                crate::refresh_overlay_mouse_tracking(window);
            }
            let _ = window.set_resizable(false);
        }
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
pub fn start_overlay_drag(app: tauri::AppHandle) -> Result<(), String> {
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

    crate::set_overlay_drag_active(&app, true);
    let drag_result = window.start_dragging().map_err(|error| error.to_string());
    if drag_result.is_err() {
        crate::set_overlay_drag_active(&app, false);
        return drag_result;
    }

    #[cfg(target_os = "macos")]
    {
        // Tauri 的 start_dragging 只负责把原生拖动请求投递到主线程，因此要等鼠标真正松开后再收尾。
        let drag_app = app.clone();
        let drag_window = window.clone();
        tauri::async_runtime::spawn(async move {
            while crate::primary_mouse_button_pressed() {
                tokio::time::sleep(Duration::from_millis(16)).await;
            }
            let finish_app = drag_app.clone();
            if let Err(error) = drag_app.run_on_main_thread(move || {
                crate::set_overlay_drag_active(&finish_app, false);
                if let Ok(position) = drag_window.outer_position() {
                    crate::settle_overlay_position_at(&finish_app, &drag_window, position);
                }
            }) {
                crate::set_overlay_drag_active(&drag_app, false);
                log::warn!("完成桌面歌词原生拖动失败：{error}");
            }
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        crate::set_overlay_drag_active(&app, false);
        if let Ok(position) = window.outer_position() {
            crate::settle_overlay_position_at(&app, &window, position);
        }
    }

    Ok(())
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
    let toolbar_placement = Some(
        state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .toolbar_placement
            .normalized(style.orientation),
    );
    let minimum_width_logical = match style.orientation {
        OverlayOrientation::Horizontal => MIN_HORIZONTAL_WINDOW_WIDTH,
        OverlayOrientation::Vertical => MIN_VERTICAL_HOST_WIDTH,
    };
    let (next_position, next_size) = fit_overlay_content_bounds(
        position,
        current_size,
        width,
        height,
        scale,
        work_area.position,
        work_area.size,
        toolbar_placement,
        minimum_width_logical,
    );
    let size_changed = current_size.width.abs_diff(next_size.width) > 2
        || current_size.height.abs_diff(next_size.height) > 2;
    if size_changed || position != next_position {
        crate::mark_overlay_programmatic_position(&app, next_position);
        crate::set_window_frame(
            &window,
            current_size,
            position,
            next_size,
            next_position,
            scale,
        )
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
    if size_changed || current_position != next_position {
        crate::set_window_frame(
            &window,
            current_size,
            current_position,
            next_size,
            next_position,
            scale,
        )
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
