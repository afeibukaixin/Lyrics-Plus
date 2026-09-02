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
    let Some(overlay) = app.get_webview_window("lyrics-overlay") else {
        let _ = destroy_surface(app, "lyrics-unlock-handle");
        return;
    };
    let should_show = settings.visible && settings.locked && overlay.is_visible().unwrap_or(false);
    if !should_show {
        let _ = destroy_surface(app, "lyrics-unlock-handle");
        return;
    }
    if app.get_webview_window("lyrics-unlock-handle").is_none() {
        if let Err(error) = create_unlock_handle(app) {
            log::warn!("Failed to create overlay unlock handle: {error}");
            return;
        }
    }
    position_unlock_handle(app);
    wake_overlay_pointer_monitor(app);
}

fn position_list_unlock_handle(app: &tauri::AppHandle) {
    let (Some(list), Some(handle)) = (
        app.get_webview_window("lyrics-list"),
        app.get_webview_window("lyrics-list-unlock-handle"),
    ) else {
        return;
    };
    let (Ok(position), Ok(size), Ok(handle_size), Ok(scale)) = (
        list.outer_position(),
        list.outer_size(),
        handle.outer_size(),
        list.scale_factor(),
    ) else {
        return;
    };
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let inset = (12.0 * scale).round() as u32;
    let x = position.x.saturating_add(
        size.width
            .saturating_sub(handle_size.width)
            .saturating_sub(inset) as i32,
    );
    let y = position.y.saturating_add(inset as i32);
    let _ = handle.set_position(tauri::PhysicalPosition::new(x, y));
}

const LIST_UNLOCK_HANDLE_HOVER_EVENT: &str = "lyrics-list-unlock-handle://hover";

pub(crate) fn sync_list_unlock_handle(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let preferences = state.config.snapshot().lyrics.displays.list_window;
    let Some(list) = app.get_webview_window("lyrics-list") else {
        let _ = destroy_surface(app, "lyrics-list-unlock-handle");
        return;
    };
    let should_show =
        preferences.enabled && preferences.locked && list.is_visible().unwrap_or(false);
    if !should_show {
        let _ = destroy_surface(app, "lyrics-list-unlock-handle");
        return;
    }
    if app
        .get_webview_window("lyrics-list-unlock-handle")
        .is_none()
    {
        if let Err(error) = create_list_unlock_handle(app) {
            log::warn!("Failed to create lyrics list unlock handle: {error}");
            return;
        }
    }
    position_list_unlock_handle(app);
    wake_overlay_pointer_monitor(app);
}

fn schedule_list_unlock_handle_sync(app: &tauri::AppHandle) {
    let target = app.clone();
    if let Err(error) = app.run_on_main_thread(move || sync_list_unlock_handle(&target)) {
        log::warn!("Failed to schedule lyrics list unlock handle synchronization: {error}");
    }
}

fn wake_overlay_pointer_monitor(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.pointer_monitor_wake.notify_waiters();
    }
}

fn start_overlay_pointer_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_inside_at: Option<Instant> = None;
        let mut last_handle_hovered: Option<bool> = None;
        let mut last_overlay_hovered: Option<bool> = None;

        loop {
            let has_visible_consumer = ["lyrics-overlay", "lyrics-notch"].iter().any(|label| {
                app.get_webview_window(label)
                    .and_then(|window| window.is_visible().ok())
                    .unwrap_or(false)
            });
            if !has_visible_consumer {
                last_inside_at = None;
                last_handle_hovered = None;
                last_overlay_hovered = None;
                if let Some(state) = app.try_state::<AppState>() {
                    let wake = state.pointer_monitor_wake.clone();
                    wake.notified().await;
                } else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                continue;
            }
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
                if notch.is_visible().unwrap_or(false) {
                    // 只上报坐标，实际 hover 区域由前端根据当前 Visual Island rect 判断。
                    if let (Ok(cursor), Ok(position), Ok(scale_factor)) = (
                        app.cursor_position(),
                        notch.outer_position(),
                        notch.scale_factor(),
                    ) {
                        let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
                            scale_factor
                        } else {
                            1.0
                        };
                        let _ = notch.emit(
                            NOTCH_POINTER_SAMPLE_EVENT,
                            NotchPointerSamplePayload {
                                client_x: (cursor.x - f64::from(position.x)) / scale,
                                client_y: (cursor.y - f64::from(position.y)) / scale,
                            },
                        );
                    }
                }
            }

            let Some(overlay) = app.get_webview_window("lyrics-overlay") else {
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
                if let Some(handle) = app.get_webview_window("lyrics-unlock-handle") {
                    if handle.is_visible().unwrap_or(false) {
                        let _ = handle.hide();
                    }
                    if last_handle_hovered != Some(false) {
                        let _ = handle.emit(UNLOCK_HANDLE_HOVER_EVENT, false);
                        last_handle_hovered = Some(false);
                    }
                }
                continue;
            }

            let Some(handle) = app.get_webview_window("lyrics-unlock-handle") else {
                continue;
            };

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

fn start_list_unlock_handle_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_inside_at: Option<Instant> = None;
        let mut last_handle_hovered: Option<bool> = None;

        loop {
            let list_visible = app
                .get_webview_window("lyrics-list")
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(false);
            if !list_visible {
                last_inside_at = None;
                last_handle_hovered = None;
                if let Some(handle) = app.get_webview_window("lyrics-list-unlock-handle") {
                    let _ = handle.hide();
                }
                if let Some(state) = app.try_state::<AppState>() {
                    let wake = state.pointer_monitor_wake.clone();
                    wake.notified().await;
                } else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                continue;
            }
            tokio::time::sleep(OVERLAY_POINTER_MONITOR_INTERVAL).await;

            let Some(state) = app.try_state::<AppState>() else {
                continue;
            };
            let preferences = state.config.snapshot().lyrics.displays.list_window;
            let Some(list) = app.get_webview_window("lyrics-list") else {
                continue;
            };
            let visible = list.is_visible().unwrap_or(false);
            if !visible || !preferences.enabled || !preferences.locked {
                last_inside_at = None;
                if let Some(handle) = app.get_webview_window("lyrics-list-unlock-handle") {
                    if handle.is_visible().unwrap_or(false) {
                        let _ = handle.hide();
                    }
                    if last_handle_hovered != Some(false) {
                        let _ = handle.emit(LIST_UNLOCK_HANDLE_HOVER_EVENT, false);
                        last_handle_hovered = Some(false);
                    }
                }
                continue;
            }

            let Some(handle) = app.get_webview_window("lyrics-list-unlock-handle") else {
                schedule_list_unlock_handle_sync(&app);
                continue;
            };
            let sample = (
                app.cursor_position(),
                list.outer_position(),
                list.outer_size(),
                handle.outer_position(),
                handle.outer_size(),
            );
            let (should_show, hovered) = match sample {
                (
                    Ok(cursor),
                    Ok(list_position),
                    Ok(list_size),
                    Ok(handle_position),
                    Ok(handle_size),
                ) => {
                    let now = Instant::now();
                    let inside_list = point_in_window_bounds(cursor, list_position, list_size);
                    if inside_list {
                        last_inside_at = Some(now);
                    }
                    let within_hide_delay = last_inside_at.is_some_and(|last_inside| {
                        now.duration_since(last_inside) < UNLOCK_HANDLE_HIDE_DELAY
                    });
                    (
                        inside_list || within_hide_delay,
                        inside_list && point_in_window_bounds(cursor, handle_position, handle_size),
                    )
                }
                _ => {
                    last_inside_at = None;
                    (true, false)
                }
            };

            if should_show != handle.is_visible().unwrap_or(false) {
                if should_show {
                    position_list_unlock_handle(&app);
                    let _ = handle.show();
                } else {
                    let _ = handle.hide();
                }
            }
            let hovered = should_show && hovered;
            if last_handle_hovered != Some(hovered) {
                let _ = handle.emit(LIST_UNLOCK_HANDLE_HOVER_EVENT, hovered);
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
    setup_tray(app).map_err(|error| error.to_string())?;
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
    #[cfg(target_os = "macos")]
    apply_dock_icon_hidden(app, configured.app.hide_dock_icon)?;
    sync_app_menu_bar_icon_visibility(app)?;
    start_overlay_pointer_monitor(app.clone());
    start_list_unlock_handle_monitor(app.clone());
    start_player_monitor(app.clone());
    player_lifecycle::start_exit_monitor(app.clone());
    Ok(())
}
