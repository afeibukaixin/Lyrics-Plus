use crate::runtime_model::OverlaySettings;
use crate::ToolbarPlacement;

pub(crate) fn point_in_window_bounds(
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

pub(crate) fn should_hover_overlay(
    settings: &OverlaySettings,
    cursor: tauri::PhysicalPosition<f64>,
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
) -> bool {
    settings.visible && !settings.locked && point_in_window_bounds(cursor, position, size)
}

pub(crate) fn stable_overlay_hover(
    previous: Option<bool>,
    sampled: bool,
    mouse_pressed: bool,
) -> bool {
    if mouse_pressed {
        previous.unwrap_or(sampled)
    } else {
        sampled
    }
}

pub(crate) fn unlock_handle_position(
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
