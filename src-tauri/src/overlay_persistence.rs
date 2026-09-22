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

fn restored_overlay_position_with_anchor(
    bounds: &StoredBounds,
    work_position: tauri::PhysicalPosition<i32>,
    work_size: tauri::PhysicalSize<u32>,
    window_size: tauri::PhysicalSize<u32>,
    scale_factor: f64,
    horizontal_anchor: Option<HorizontalAnchor>,
) -> tauri::PhysicalPosition<i32> {
    let same_work_area = bounds.work_x == Some(work_position.x)
        && bounds.work_y == Some(work_position.y)
        && bounds.work_width == Some(work_size.width)
        && bounds.work_height == Some(work_size.height)
        && bounds
            .scale_factor
            .is_some_and(|saved| (saved - scale_factor).abs() < 0.001);

    let (mut x, y) = if same_work_area {
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

    // 竖排窗口恢复时沿用保存的横向锚点，内容适配改变宽度后仍保持同一边缘或中心。
    if same_work_area {
        if let Some(saved_width) = bounds.window_width {
            let width_delta = saved_width as i64 - window_size.width as i64;
            let offset = match horizontal_anchor {
                Some(HorizontalAnchor::Right) => width_delta,
                Some(HorizontalAnchor::Free) => (width_delta as f64 / 2.0).round() as i64,
                Some(HorizontalAnchor::Left) | None => 0,
            };
            x = (x as i64 + offset).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        }
    } else if let (Some(anchor), Some(saved_work_x), Some(saved_work_width), Some(saved_width)) = (
        horizontal_anchor,
        bounds.work_x,
        bounds.work_width,
        bounds.window_width,
    ) {
        let saved_scale = bounds.scale_factor.filter(|scale| scale.is_finite() && *scale > 0.0);
        let scale_ratio = saved_scale.map_or(1.0, |saved| scale_factor / saved);
        match anchor {
            HorizontalAnchor::Left => {
                let saved_gap = bounds.x as i64 - saved_work_x as i64;
                x = work_position.x.saturating_add((saved_gap as f64 * scale_ratio).round() as i32);
            }
            HorizontalAnchor::Right => {
                let saved_right = saved_work_x as i64 + saved_work_width as i64;
                let saved_gap = saved_right - (bounds.x as i64 + saved_width as i64);
                let gap = (saved_gap as f64 * scale_ratio).round() as i64;
                x = (work_position.x as i64 + work_size.width as i64
                    - window_size.width as i64
                    - gap)
                    .clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            }
            HorizontalAnchor::Free => {}
        }
    }

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
    horizontal_anchor: HorizontalAnchor,
) -> StoredBounds {
    let work_area = monitor.work_area();
    StoredBounds {
        x: position.x,
        y: position.y,
        window_width: Some(window_size.width),
        window_height: Some(window_size.height),
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
        horizontal_anchor: Some(horizontal_anchor),
    }
}

fn resolved_horizontal_anchor(
    bounds: &StoredBounds,
    orientation: crate::OverlayOrientation,
    scale_factor: f64,
) -> HorizontalAnchor {
    if orientation != crate::OverlayOrientation::Vertical {
        return HorizontalAnchor::Free;
    }
    if let Some(anchor) = bounds.horizontal_anchor {
        return anchor;
    }

    // 旧记录没有显式锚点：相对位置为 0/1 时可直接判定贴左/右边。
    if bounds.relative_x.is_some_and(|relative| relative <= 0.001) {
        return HorizontalAnchor::Left;
    }
    if bounds.relative_x.is_some_and(|relative| relative >= 0.999) {
        return HorizontalAnchor::Right;
    }

    let (Some(work_x), Some(work_width), Some(window_width)) =
        (bounds.work_x, bounds.work_width, bounds.window_width)
    else {
        return HorizontalAnchor::Free;
    };
    let work_right = work_x as i64 + work_width as i64;
    let left_gap = bounds.x as i64 - work_x as i64;
    let right_gap = work_right - (bounds.x as i64 + window_width as i64);
    let inset = (crate::overlay_surface::VERTICAL_OVERLAY_SURFACE_INSET * scale_factor).round()
        as i64;
    let edge_distance = |gap: i64| gap.unsigned_abs().min(gap.abs_diff(inset));
    let left_distance = edge_distance(left_gap);
    let right_distance = edge_distance(right_gap);
    let threshold = crate::overlay_placement::OVERLAY_EDGE_SNAP_DISTANCE as u64;

    if left_distance <= threshold && left_distance <= right_distance {
        HorizontalAnchor::Left
    } else if right_distance <= threshold {
        HorizontalAnchor::Right
    } else {
        HorizontalAnchor::Free
    }
}

pub(crate) fn update_overlay_horizontal_anchor(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    snapped_position: tauri::PhysicalPosition<i32>,
) {
    let state = app.state::<AppState>();
    let orientation = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .orientation;
    let anchor = if orientation == crate::OverlayOrientation::Vertical {
        let (Ok(Some(monitor)), Ok(window_size)) =
            (window.current_monitor(), window.outer_size())
        else {
            set_overlay_horizontal_anchor(app, HorizontalAnchor::Free);
            return;
        };
        let monitor_position = monitor.position();
        let monitor_right = monitor_position.x as i64 + monitor.size().width as i64;
        if snapped_position.x == monitor_position.x {
            HorizontalAnchor::Left
        } else if snapped_position.x as i64 + window_size.width as i64 == monitor_right {
            HorizontalAnchor::Right
        } else {
            HorizontalAnchor::Free
        }
    } else {
        HorizontalAnchor::Free
    };
    set_overlay_horizontal_anchor(app, anchor);
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
    let (toolbar_placement, horizontal_anchor) = {
        let placement = state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (placement.toolbar_placement, placement.horizontal_anchor)
    };
    let bounds = stored_bounds(
        position,
        window_size,
        &monitor,
        toolbar_placement,
        horizontal_anchor,
    );
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
    mark_legacy_restore: bool,
) -> bool {
    let state = app.state::<AppState>();
    let id = monitor_id(monitor);
    let key = format!("overlay.position.{id}");
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
    let orientation = state
        .overlay_style
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .orientation;
    let work_area = monitor.work_area();
    let horizontal_anchor = resolved_horizontal_anchor(&bounds, orientation, monitor.scale_factor());
    let position = restored_overlay_position_with_anchor(
        &bounds,
        work_area.position,
        work_area.size,
        window_size,
        monitor.scale_factor(),
        (orientation == crate::OverlayOrientation::Vertical).then_some(horizontal_anchor),
    );
    let legacy_vertical_position =
        orientation == crate::OverlayOrientation::Vertical && bounds.window_width.is_none();
    set_overlay_toolbar_placement(
        app,
        bounds
            .toolbar_placement
            .unwrap_or_else(|| ToolbarPlacement::for_orientation(orientation))
            .normalized(orientation),
    );
    set_overlay_horizontal_anchor(app, horizontal_anchor);
    set_overlay_position(app, window, position);
    if mark_legacy_restore {
        state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pending_legacy_restore_monitor = legacy_vertical_position.then_some(id);
    }
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
            if apply_stored_overlay_position(app, window, monitor, false) {
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
        if apply_stored_overlay_position(app, window, monitor, true) {
            return;
        }
    }
    cancel_pending_legacy_overlay_restore(app);
    set_overlay_horizontal_anchor(app, HorizontalAnchor::Free);
    move_overlay_to_primary(app, window);
}

pub(crate) fn cancel_pending_legacy_overlay_restore(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state
            .overlay_placement
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .cancel_pending_legacy_restore();
    }
}

pub(crate) fn pending_legacy_overlay_restore_position(
    app: &tauri::AppHandle,
    monitor: &tauri::Monitor,
    window_size: tauri::PhysicalSize<u32>,
) -> Option<tauri::PhysicalPosition<i32>> {
    let state = app.state::<AppState>();
    let id = monitor_id(monitor);
    let pending = state
        .overlay_placement
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .pending_legacy_restore_monitor
        .as_deref()
        == Some(id.as_str());
    if !pending {
        return None;
    }
    let bounds = state
        .storage
        .get_preference(&format!("overlay.position.{id}"))
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<StoredBounds>(&raw).ok())?;
    if bounds.window_width.is_some() {
        return None;
    }
    let work_area = monitor.work_area();
    let anchor = resolved_horizontal_anchor(
        &bounds,
        crate::OverlayOrientation::Vertical,
        monitor.scale_factor(),
    );
    if anchor == HorizontalAnchor::Right && bounds.window_width.is_none() {
        return Some(tauri::PhysicalPosition::new(
            work_area.position.x
                + work_area.size.width.saturating_sub(window_size.width) as i32,
            clamp_axis(
                bounds.y,
                work_area.position.y,
                work_area.size.height,
                window_size.height,
            ),
        ));
    }
    Some(restored_overlay_position_with_anchor(
        &bounds,
        work_area.position,
        work_area.size,
        window_size,
        monitor.scale_factor(),
        Some(anchor),
    ))
}

pub(crate) fn complete_pending_legacy_overlay_restore(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    position: tauri::PhysicalPosition<i32>,
) {
    persist_overlay_state_at(app, window, position);
    cancel_pending_legacy_overlay_restore(app);
}
