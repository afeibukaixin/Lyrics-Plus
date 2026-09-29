mod geometry;
mod main_window;
mod persistence;
mod state;
mod toolbar;

#[cfg(test)]
pub(crate) use geometry::{centered_position, toolbar_placement_after_move};
pub(crate) use main_window::{
    mark_overlay_programmatic_position, primary_mouse_button_pressed, set_overlay_position,
    show_main_window_at, show_main_window_centered,
};
#[cfg(target_os = "macos")]
pub(crate) use persistence::load_overlay_toolbar_placement;
#[cfg(not(target_os = "macos"))]
pub(crate) use persistence::StoredBounds;
pub(crate) use persistence::{overlay_geometry, StoredOverlayGeometry};
pub use state::ToolbarPlacement;
pub(crate) use state::{should_show_main_window, HorizontalAnchor, OverlayPlacementState};
#[cfg(target_os = "macos")]
pub(crate) use toolbar::restore_overlay_content_position;
#[cfg(not(target_os = "macos"))]
pub(crate) use toolbar::set_overlay_toolbar_placement;
pub(crate) use toolbar::{
    overlay_drag_active, reset_overlay_toolbar_placement, set_overlay_drag_active,
    set_overlay_horizontal_anchor, settle_overlay_position_at,
    update_overlay_toolbar_placement_during_drag, NotchPointerSamplePayload,
    NOTCH_POINTER_SAMPLE_EVENT, OVERLAY_HOVER_EVENT, OVERLAY_POINTER_MONITOR_INTERVAL,
    UNLOCK_HANDLE_BACKGROUND_GAP, UNLOCK_HANDLE_HIDE_DELAY, UNLOCK_HANDLE_HOVER_EVENT,
};
