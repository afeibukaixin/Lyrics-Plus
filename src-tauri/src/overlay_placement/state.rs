use std::time::{Duration, Instant};

use crate::overlay_model::OverlayOrientation;

pub(crate) const PROGRAMMATIC_MOVE_SUPPRESSION: Duration = Duration::from_secs(2);

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MonitorTopologyEntry {
    pub(crate) id: String,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) work_x: i32,
    pub(crate) work_y: i32,
    pub(crate) work_width: u32,
    pub(crate) work_height: u32,
    pub(crate) scale_factor_bits: u64,
}

#[derive(Default)]
pub(crate) struct OverlayPlacementState {
    pub(crate) preferred_monitor: Option<String>,
    pub(crate) topology: Vec<MonitorTopologyEntry>,
    pub(crate) toolbar_placement: ToolbarPlacement,
    pub(crate) drag_active: bool,
    pub(crate) expected_programmatic_position: Option<tauri::PhysicalPosition<i32>>,
    pub(crate) programmatic_move_started_at: Option<Instant>,
}

impl OverlayPlacementState {
    pub(crate) fn update_topology(&mut self, next: Vec<MonitorTopologyEntry>) -> bool {
        if self.topology.is_empty() {
            self.topology = next;
            return false;
        }
        if self.topology == next {
            return false;
        }
        self.topology = next;
        self.expected_programmatic_position = None;
        self.programmatic_move_started_at = None;
        true
    }

    pub(crate) fn consume_programmatic_move(
        &mut self,
        position: tauri::PhysicalPosition<i32>,
    ) -> bool {
        let expected = self.expected_programmatic_position.take();
        self.programmatic_move_started_at = None;
        let Some(expected) = expected else {
            return false;
        };
        expected.x.abs_diff(position.x) <= 2 && expected.y.abs_diff(position.y) <= 2
    }

    pub(crate) fn suppress_persistence(&mut self, now: Instant) -> bool {
        let active = self.programmatic_move_started_at.is_some_and(|started| {
            now.saturating_duration_since(started) <= PROGRAMMATIC_MOVE_SUPPRESSION
        });
        if !active {
            self.expected_programmatic_position = None;
            self.programmatic_move_started_at = None;
        }
        active
    }
}

pub(crate) fn monitor_topology(monitors: &[tauri::Monitor]) -> Vec<MonitorTopologyEntry> {
    let mut topology = monitors
        .iter()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            let work_area = monitor.work_area();
            MonitorTopologyEntry {
                id: crate::monitor_id(monitor),
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
                work_x: work_area.position.x,
                work_y: work_area.position.y,
                work_width: work_area.size.width,
                work_height: work_area.size.height,
                scale_factor_bits: monitor.scale_factor().to_bits(),
            }
        })
        .collect::<Vec<_>>();
    topology.sort_by(|left, right| {
        (&left.id, left.x, left.y, left.width, left.height).cmp(&(
            &right.id,
            right.x,
            right.y,
            right.width,
            right.height,
        ))
    });
    topology
}

pub(crate) fn should_show_main_window(notice_accepted: bool, silent_startup: bool) -> bool {
    !notice_accepted || !silent_startup
}
