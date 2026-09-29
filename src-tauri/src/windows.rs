mod lifecycle;
mod list_lyrics;
#[cfg(not(target_os = "macos"))]
mod list_lyrics_position;
#[cfg(target_os = "macos")]
#[path = "windows/list_lyrics_position_macos.rs"]
mod list_lyrics_position;
mod notch;
mod overlay;
mod platform;
mod quick_lyrics;
mod reconcile;
#[cfg(target_os = "macos")]
mod screen_affinity;
#[cfg(target_os = "macos")]
pub(crate) use screen_affinity::{install_screen_observer, restore_screen_affinity, save_user_placement, save_geometry_on_target, reset_screen_affinity};
#[cfg(not(target_os = "macos"))]
mod status_bar;

pub(crate) use lifecycle::{
    cancel_surface_destroy, configure_web_content_process_handler, handle_surface_destroyed,
    hide_surface, is_managed_surface_label, prepare_surface_show, schedule_surface_destroy,
    set_surface_runtime_state, surface_is_destroying, SurfaceRuntimeState,
};
pub(crate) use list_lyrics::{apply_list_lyrics_window_lock, reset_list_lyrics_window_size};
#[cfg(not(target_os = "macos"))]
pub(crate) use list_lyrics_position::{
    list_lyrics_window_moved, list_lyrics_window_resized, reconcile_list_lyrics_placement,
    reset_list_lyrics_position,
};
#[cfg(target_os = "macos")]
pub(crate) use list_lyrics_position::reset_list_lyrics_position;
pub(crate) use notch::{
    notch_monitor_id, notch_window_position, screen_notch_layout, set_window_frame,
};
pub(crate) use overlay::create_overlay;
#[cfg(test)]
pub(crate) use overlay::initial_overlay_dimensions;
pub(crate) use platform::{
    apply_joining_other_apps_fullscreen, apply_lyrics_window_space_behavior,
    apply_lyrics_windows_space_behavior, refresh_overlay_mouse_tracking,
};
#[cfg(target_os = "macos")]
pub(crate) use platform::{clear_native_frame_autosave, OVERLAY_FRAME_AUTOSAVE_NAME};
pub(crate) use quick_lyrics::{show_quick_lyrics_window, toggle_quick_lyrics_window};
pub(crate) use reconcile::{
    position_auxiliary_lyrics_window_default, reconcile_auxiliary_lyrics_windows,
    sync_lyrics_surfaces,
};
