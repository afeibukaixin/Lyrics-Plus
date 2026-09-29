use crate::overlay_model::OverlayStyleSettings;

#[cfg(target_os = "macos")]
pub(crate) const OVERLAY_TOOLBAR_PLACEMENT_KEY: &str = "overlay.toolbar-placement";

#[cfg(target_os = "macos")]
pub(crate) fn load_overlay_toolbar_placement(
    storage: &crate::storage::Storage,
    last_monitor: Option<&str>,
    orientation: crate::OverlayOrientation,
) -> super::state::ToolbarPlacement {
    use super::state::ToolbarPlacement;

    fn placement_from_record(raw: &str) -> Option<ToolbarPlacement> {
        serde_json::from_str::<serde_json::Value>(raw)
            .ok()?
            .get("toolbarPlacement")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    let saved = storage
        .get_preference(OVERLAY_TOOLBAR_PLACEMENT_KEY)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<ToolbarPlacement>(&raw).ok());
    let legacy = || {
        storage
            .get_preference(crate::window_placement::OVERLAY_POSITION_KEY)
            .ok()
            .flatten()
            .and_then(|raw| placement_from_record(&raw))
            .or_else(|| {
                last_monitor.and_then(|id| {
                    storage
                        .get_preference(&format!("overlay.position.{id}"))
                        .ok()
                        .flatten()
                        .and_then(|raw| placement_from_record(&raw))
                })
            })
    };
    let placement = saved
        .or_else(legacy)
        .unwrap_or_else(|| ToolbarPlacement::for_orientation(orientation))
        .normalized(orientation);
    if saved != Some(placement) {
        if let Ok(raw) = serde_json::to_string(&placement) {
            if let Err(error) = storage.set_preference(OVERLAY_TOOLBAR_PLACEMENT_KEY, &raw) {
                log::warn!("保存桌面歌词工具栏方向失败：{error}");
            }
        }
    }
    placement
}

#[cfg(not(target_os = "macos"))]
use super::state::HorizontalAnchor;

#[cfg(not(target_os = "macos"))]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredBounds {
    pub(crate) x: i32,
    pub(crate) y: i32,
    #[serde(default)]
    pub(crate) window_width: Option<u32>,
    #[serde(default)]
    pub(crate) window_height: Option<u32>,
    #[serde(default)]
    pub(crate) work_x: Option<i32>,
    #[serde(default)]
    pub(crate) work_y: Option<i32>,
    #[serde(default)]
    pub(crate) work_width: Option<u32>,
    #[serde(default)]
    pub(crate) work_height: Option<u32>,
    #[serde(default)]
    pub(crate) scale_factor: Option<f64>,
    #[serde(default)]
    pub(crate) toolbar_placement: Option<super::state::ToolbarPlacement>,
    #[serde(default)]
    pub(crate) horizontal_anchor: Option<HorizontalAnchor>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct StoredOverlayGeometry {
    pub(crate) horizontal_max_width: Option<f64>,
    pub(crate) vertical_max_height: Option<f64>,
}

pub(crate) fn overlay_geometry(
    storage: &crate::storage::Storage,
    monitor_id: Option<&str>,
) -> StoredOverlayGeometry {
    let geometry_key = monitor_id
        .map(|id| format!("overlay.geometry.{id}"))
        .unwrap_or_else(|| "overlay.geometry.default".into());
    if let Ok(Some(raw)) = storage.get_preference(&geometry_key) {
        if let Ok(geometry) = serde_json::from_str(&raw) {
            return geometry;
        }
    }
    let legacy_key = monitor_id
        .map(|id| format!("overlay.style.{id}"))
        .unwrap_or_else(|| "overlay.style.default".into());
    storage
        .get_preference(&legacy_key)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<OverlayStyleSettings>(&raw).ok())
        .map(|style| StoredOverlayGeometry {
            horizontal_max_width: style.horizontal_max_width,
            vertical_max_height: style.vertical_max_height,
        })
        .unwrap_or_default()
}
