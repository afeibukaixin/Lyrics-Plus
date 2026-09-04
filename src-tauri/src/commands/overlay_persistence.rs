use crate::config::OverlayAppearance;
use crate::{AppState, OverlayStyleSettings};
use tauri::{Emitter, Manager};

pub(super) fn persist_overlay_style_for_current_monitor(
    app: &tauri::AppHandle,
    state: &AppState,
    style: &OverlayStyleSettings,
) -> Result<(), String> {
    let monitor_id = app
        .get_webview_window("lyrics-overlay")
        .and_then(|window| window.current_monitor().ok().flatten())
        .map(|monitor| crate::monitor_id(&monitor));
    *state
        .overlay_monitor
        .write()
        .unwrap_or_else(|error| error.into_inner()) = monitor_id.clone();
    let key = monitor_id
        .map(|id| format!("overlay.geometry.{id}"))
        .unwrap_or_else(|| "overlay.geometry.default".into());
    let geometry = crate::StoredOverlayGeometry {
        horizontal_max_width: style.horizontal_max_width,
        vertical_max_height: style.vertical_max_height,
    };
    let raw =
        serde_json::to_string(&geometry).map_err(|error| format!("无法序列化浮窗尺寸：{error}"))?;
    state.storage.set_preference(&key, &raw)?;
    state
        .config
        .update(|config| config.overlay.appearance = OverlayAppearance::from(style))?;
    if let Some(window) = app.get_webview_window("lyrics-overlay") {
        crate::sync_overlay_vibrancy(&window, style);
    }
    app.emit("overlay://style", style)
        .map_err(|error| error.to_string())
}
