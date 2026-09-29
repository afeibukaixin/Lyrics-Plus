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

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HorizontalAnchor {
    Left,
    Right,
    #[default]
    Free,
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
    pub(crate) horizontal_anchor: HorizontalAnchor,
    pub(crate) drag_active: bool,
}

pub(crate) fn should_show_main_window(notice_accepted: bool, silent_startup: bool) -> bool {
    !notice_accepted || !silent_startup
}
