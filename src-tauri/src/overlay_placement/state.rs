use super::desktop::DesktopPlacement;
use crate::overlay_model::OverlayOrientation;

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarPlacement {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

impl ToolbarPlacement {
    pub(crate) fn for_orientation(orientation: OverlayOrientation) -> Self {
        match orientation {
            OverlayOrientation::Horizontal => Self::Top,
            OverlayOrientation::Vertical => Self::Right,
        }
    }

    pub(crate) fn normalized(self, orientation: OverlayOrientation) -> Self {
        match (orientation, self) {
            (OverlayOrientation::Horizontal, Self::Top | Self::Bottom)
            | (OverlayOrientation::Vertical, Self::Left | Self::Right) => self,
            _ => Self::for_orientation(orientation),
        }
    }
}

#[derive(Default)]
pub(crate) struct OverlayPlacementState {
    pub(crate) toolbar_placement: ToolbarPlacement,
    pub(crate) drag_active: bool,
    pub(crate) record: Option<DesktopPlacement>,
    pub(crate) window_generation: u64,
    pub(crate) latest_fit_sequence: u64,
    pub(crate) layout_ready: bool,
}

pub(crate) fn should_show_main_window(notice_accepted: bool, silent_startup: bool) -> bool {
    !notice_accepted || !silent_startup
}
