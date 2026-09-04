mod geometry;
mod list;
mod overlay;
mod runtime;

#[cfg(test)]
pub(crate) use geometry::{
    point_in_window_bounds, should_hover_overlay, stable_overlay_hover, unlock_handle_position,
};
pub(crate) use list::{sync_list_unlock_handle, LIST_UNLOCK_HANDLE_HOVER_EVENT};
pub(crate) use overlay::{position_unlock_handle, sync_unlock_handle};
pub(crate) use runtime::activate_runtime;

use crate::AppState;
use tauri::Manager;

pub(crate) fn wake_overlay_pointer_monitor(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.pointer_monitor_wake.notify_waiters();
    }
}
