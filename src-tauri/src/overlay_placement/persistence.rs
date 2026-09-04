use crate::overlay_model::OverlayStyleSettings;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredBounds {
    pub(crate) x: i32,
    pub(crate) y: i32,
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
    pub(crate) relative_x: Option<f64>,
    #[serde(default)]
    pub(crate) relative_y: Option<f64>,
    #[serde(default)]
    pub(crate) toolbar_placement: Option<super::state::ToolbarPlacement>,
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
