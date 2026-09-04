use crate::overlay_effect::{HORIZONTAL_OVERLAY_SURFACE_INSET, VERTICAL_OVERLAY_SURFACE_INSET};
use crate::overlay_model::OverlayOrientation;

use super::state::ToolbarPlacement;

pub(crate) const OVERLAY_EDGE_SNAP_DISTANCE: i32 = 12;

pub(crate) fn centered_position(
    work_position: tauri::PhysicalPosition<i32>,
    work_size: tauri::PhysicalSize<u32>,
    window_size: tauri::PhysicalSize<u32>,
) -> tauri::PhysicalPosition<i32> {
    tauri::PhysicalPosition::new(
        work_position.x + work_size.width.saturating_sub(window_size.width) as i32 / 2,
        work_position.y + work_size.height.saturating_sub(window_size.height) as i32 / 2,
    )
}

pub(crate) fn monitor_contains_point(
    monitor: &tauri::Monitor,
    point: tauri::PhysicalPosition<f64>,
) -> bool {
    let position = monitor.position();
    let size = monitor.size();
    point.x >= position.x as f64
        && point.x < position.x as f64 + size.width as f64
        && point.y >= position.y as f64
        && point.y < position.y as f64 + size.height as f64
}

pub(crate) fn toolbar_placement_after_move(
    orientation: OverlayOrientation,
    placement: ToolbarPlacement,
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    scale: f64,
    work_position: tauri::PhysicalPosition<i32>,
    work_size: tauri::PhysicalSize<u32>,
) -> (ToolbarPlacement, tauri::PhysicalPosition<i32>) {
    let placement = placement.normalized(orientation);
    let inset = (match orientation {
        OverlayOrientation::Horizontal => HORIZONTAL_OVERLAY_SURFACE_INSET,
        OverlayOrientation::Vertical => VERTICAL_OVERLAY_SURFACE_INSET,
    } * scale)
        .round() as i32;
    match (orientation, placement) {
        (OverlayOrientation::Horizontal, ToolbarPlacement::Top)
            if position.y <= work_position.y.saturating_add(OVERLAY_EDGE_SNAP_DISTANCE) =>
        {
            (
                ToolbarPlacement::Bottom,
                tauri::PhysicalPosition::new(position.x, position.y.saturating_add(inset)),
            )
        }
        (OverlayOrientation::Horizontal, ToolbarPlacement::Bottom) => {
            let window_bottom = position.y as i64 + size.height as i64;
            let work_bottom = work_position.y as i64 + work_size.height as i64;
            if window_bottom >= work_bottom - OVERLAY_EDGE_SNAP_DISTANCE as i64 {
                (
                    ToolbarPlacement::Top,
                    tauri::PhysicalPosition::new(position.x, position.y.saturating_sub(inset)),
                )
            } else {
                (placement, position)
            }
        }
        (OverlayOrientation::Vertical, ToolbarPlacement::Right) => {
            let window_right = position.x as i64 + size.width as i64;
            let work_right = work_position.x as i64 + work_size.width as i64;
            if window_right >= work_right - OVERLAY_EDGE_SNAP_DISTANCE as i64 {
                (
                    ToolbarPlacement::Left,
                    tauri::PhysicalPosition::new(position.x.saturating_sub(inset), position.y),
                )
            } else {
                (placement, position)
            }
        }
        (OverlayOrientation::Vertical, ToolbarPlacement::Left) => {
            if position.x as i64 <= work_position.x as i64 + OVERLAY_EDGE_SNAP_DISTANCE as i64 {
                (
                    ToolbarPlacement::Right,
                    tauri::PhysicalPosition::new(position.x.saturating_add(inset), position.y),
                )
            } else {
                (placement, position)
            }
        }
        _ => (placement, position),
    }
}
