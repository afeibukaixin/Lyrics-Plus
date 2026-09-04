use crate::{AppState, TrayMenuState};
use tauri::Manager;

mod display_driver;
mod icon;
mod payload;
mod renderer;

pub(crate) fn sync_app_icon_visibility(app: &tauri::AppHandle, visible: bool) -> tauri::Result<()> {
    icon::sync_app_icon_visibility_inner(app, visible)
}

pub(crate) fn configure_lyrics_icon_identity(app: &tauri::AppHandle) -> tauri::Result<()> {
    icon::configure_lyrics_icon_identity_inner(app)
}

pub(crate) fn sync(app: &tauri::AppHandle) {
    let Some(tray_state) = app.try_state::<TrayMenuState>() else {
        return;
    };
    let payload = payload::render_payload(app);
    let visible = payload.is_some();
    let _ = tray_state.lyrics_icon.with_inner_tray_icon(move |inner| {
        if let Some(status_item) = inner.ns_status_item() {
            if status_item.isVisible() != visible {
                status_item.setVisible(visible);
            }
        }
    });
    if let Some(payload) = payload {
        renderer::render_on_main(payload, &tray_state.lyrics_icon);
    } else {
        renderer::reset_scroll();
    }
    display_driver::update_display_driver_activity(app);
}

pub(crate) fn start(app: tauri::AppHandle) {
    display_driver::start_driver(app, sync);
}

pub(crate) fn wake(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.status_bar_wake.notify_one();
    }
    display_driver::wake_driver(app, sync);
}
