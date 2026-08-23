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
