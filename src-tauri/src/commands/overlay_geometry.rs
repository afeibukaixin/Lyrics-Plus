use crate::overlay_model::{OverlayOrientation, OverlayStyleSettings};

pub(super) const MIN_HORIZONTAL_WINDOW_WIDTH: f64 = 190.0;
pub(super) const MIN_VERTICAL_HOST_WIDTH: f64 = 49.0;

pub(super) fn clear_manual_overlay_bounds(style: &mut OverlayStyleSettings) {
    style.horizontal_max_width = None;
    style.vertical_max_height = None;
}

pub(super) fn reset_overlay_dimensions(
    orientation: OverlayOrientation,
    current_width: f64,
    current_height: f64,
) -> (f64, f64) {
    match orientation {
        OverlayOrientation::Horizontal => (760.0, current_height.max(76.0)),
        OverlayOrientation::Vertical => (current_width.max(190.0), 620.0),
    }
}

pub(super) fn resize_overlay_edge_bounds(
    position: tauri::PhysicalPosition<i32>,
    current_size: tauri::PhysicalSize<u32>,
    edge: crate::commands::OverlayResizeEdge,
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
    let fallback_size =
        match edge {
            crate::commands::OverlayResizeEdge::Left
            | crate::commands::OverlayResizeEdge::Right => current_size.width,
            crate::commands::OverlayResizeEdge::Top
            | crate::commands::OverlayResizeEdge::Bottom => current_size.height,
        };
    let requested = if requested_main_size.is_finite() {
        (requested_main_size.max(0.0) * scale).round() as u32
    } else {
        fallback_size
    };

    match edge {
        crate::commands::OverlayResizeEdge::Left => {
            let fixed_right = (position.x as i64 + current_size.width as i64)
                .clamp(work_left + minimum_width as i64, work_right);
            let maximum_width = (fixed_right - work_left) as u32;
            let width = requested.clamp(minimum_width, maximum_width.max(minimum_width));
            (
                tauri::PhysicalPosition::new((fixed_right - width as i64) as i32, position.y),
                tauri::PhysicalSize::new(width, current_size.height),
            )
        }
        crate::commands::OverlayResizeEdge::Right => {
            let fixed_left =
                (position.x as i64).clamp(work_left, work_right - minimum_width as i64);
            let maximum_width = (work_right - fixed_left) as u32;
            let width = requested.clamp(minimum_width, maximum_width.max(minimum_width));
            (
                tauri::PhysicalPosition::new(fixed_left as i32, position.y),
                tauri::PhysicalSize::new(width, current_size.height),
            )
        }
        crate::commands::OverlayResizeEdge::Top => {
            let fixed_bottom = (position.y as i64 + current_size.height as i64)
                .clamp(work_top + minimum_height as i64, work_bottom);
            let maximum_height = (fixed_bottom - work_top) as u32;
            let height = requested.clamp(minimum_height, maximum_height.max(minimum_height));
            (
                tauri::PhysicalPosition::new(position.x, (fixed_bottom - height as i64) as i32),
                tauri::PhysicalSize::new(current_size.width, height),
            )
        }
        crate::commands::OverlayResizeEdge::Bottom => {
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

pub(super) fn fixed_axis_content_size(
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

#[cfg(test)]
pub(super) fn fit_overlay_bounds(
    position: tauri::PhysicalPosition<i32>,
    requested_width: f64,
    requested_height: f64,
    scale: f64,
    monitor_position: tauri::PhysicalPosition<i32>,
    monitor_size: tauri::PhysicalSize<u32>,
) -> (tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>) {
    fit_overlay_bounds_with_minimum(
        position,
        requested_width,
        requested_height,
        scale,
        monitor_position,
        monitor_size,
        MIN_HORIZONTAL_WINDOW_WIDTH,
    )
}

pub(super) fn fit_overlay_bounds_with_minimum(
    position: tauri::PhysicalPosition<i32>,
    requested_width: f64,
    requested_height: f64,
    scale: f64,
    monitor_position: tauri::PhysicalPosition<i32>,
    monitor_size: tauri::PhysicalSize<u32>,
    minimum_width_logical: f64,
) -> (tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let margin = 0_u32;
    let minimum_width = (minimum_width_logical * scale).round() as u32;
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

// 歌词窗口以工具栏相反侧为锚点；向工作区边缘增长时只限制尺寸，不移动锚点。
pub(super) fn fit_overlay_content_bounds(
    position: tauri::PhysicalPosition<i32>,
    current_size: tauri::PhysicalSize<u32>,
    requested_width: f64,
    requested_height: f64,
    scale: f64,
    monitor_position: tauri::PhysicalPosition<i32>,
    monitor_size: tauri::PhysicalSize<u32>,
    toolbar_placement: Option<crate::ToolbarPlacement>,
    minimum_width_logical: f64,
) -> (tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>) {
    let (mut next_position, mut next_size) = fit_overlay_bounds_with_minimum(
        position,
        requested_width,
        requested_height,
        scale,
        monitor_position,
        monitor_size,
        minimum_width_logical,
    );
    let Some(toolbar_placement) = toolbar_placement else {
        return (next_position, next_size);
    };
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let work_left = monitor_position.x as i64;
    let work_right = work_left + monitor_size.width as i64;
    let work_top = monitor_position.y as i64;
    let work_bottom = work_top + monitor_size.height as i64;
    let minimum_width = (minimum_width_logical * scale).round() as u32;
    let minimum_height = (76.0 * scale).round() as u32;
    let fixed_position_limit =
        |position: i64| position.clamp(i32::MIN as i64, i32::MAX as i64) as i32;

    match toolbar_placement {
        crate::ToolbarPlacement::Left => {
            let fixed_right =
                (position.x as i64 + current_size.width as i64).clamp(work_left, work_right);
            let maximum_width = fixed_right
                .saturating_sub(work_left)
                .clamp(0, u32::MAX as i64) as u32;
            let width = next_size.width.min(maximum_width.max(minimum_width));
            next_size.width = width;
            next_position.x = fixed_position_limit(fixed_right - width as i64);
        }
        crate::ToolbarPlacement::Right => {
            let fixed_left = (position.x as i64).clamp(work_left, work_right);
            let maximum_width = work_right
                .saturating_sub(fixed_left)
                .clamp(0, u32::MAX as i64) as u32;
            let width = next_size.width.min(maximum_width.max(minimum_width));
            next_size.width = width;
            next_position.x = fixed_position_limit(fixed_left);
        }
        crate::ToolbarPlacement::Top => {
            let fixed_bottom =
                (position.y as i64 + current_size.height as i64).clamp(work_top, work_bottom);
            let maximum_height = fixed_bottom
                .saturating_sub(work_top)
                .clamp(0, u32::MAX as i64) as u32;
            let height = next_size.height.min(maximum_height.max(minimum_height));
            next_size.height = height;
            next_position.y = fixed_position_limit(fixed_bottom - height as i64);
        }
        crate::ToolbarPlacement::Bottom => {
            let fixed_top = (position.y as i64).clamp(work_top, work_bottom);
            let maximum_height = work_bottom
                .saturating_sub(fixed_top)
                .clamp(0, u32::MAX as i64) as u32;
            let height = next_size.height.min(maximum_height.max(minimum_height));
            next_size.height = height;
            next_position.y = fixed_position_limit(fixed_top);
        }
    }

    (next_position, next_size)
}
