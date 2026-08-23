use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotchLayoutMetrics {
    pub has_notch: bool,
    pub top_inset: f64,
    pub center_gap_width: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OverlaySettings {
    pub visible: bool,
    pub locked: bool,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            visible: true,
            locked: false,
        }
    }
}
