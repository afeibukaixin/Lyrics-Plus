use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{PhysicalPosition, PhysicalSize};

use crate::monitor_id;

pub(crate) const OVERLAY_POSITION_KEY: &str = "lyrics-window-placement.overlay";
pub(crate) const LIST_POSITION_KEY: &str = "lyrics-window-placement.list";
const AUTOMATIC_MOVE_GUARD: Duration = Duration::from_millis(800);

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Edge {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
struct EdgeDistance {
    edge: Edge,
    logical_pixels: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
/// 每个歌词窗口只有一份位置记录；切屏更新坐标，拖动才更新贴边距离。
pub(crate) struct WindowPlacement {
    pub(crate) monitor_id: String,
    pub(crate) manual_monitor: bool,
    x: i32,
    y: i32,
    work_x: i32,
    work_y: i32,
    work_width: u32,
    work_height: u32,
    window_width: u32,
    window_height: u32,
    scale_factor: f64,
    horizontal: EdgeDistance,
    vertical: EdgeDistance,
}

#[derive(Clone, Copy)]
pub(crate) struct WorkArea {
    pub(crate) position: PhysicalPosition<i32>,
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) scale_factor: f64,
}

impl WorkArea {
    pub(crate) fn of(monitor: &tauri::Monitor) -> Self {
        let work = monitor.work_area();
        Self {
            position: work.position,
            size: work.size,
            scale_factor: monitor.scale_factor(),
        }
    }

    fn scale(self) -> f64 {
        if self.scale_factor.is_finite() && self.scale_factor > 0.0 {
            self.scale_factor
        } else {
            1.0
        }
    }
}

fn edge_distance(start_gap: i64, end_gap: i64, scale: f64) -> EdgeDistance {
    let (edge, gap) = if start_gap <= end_gap {
        (Edge::Start, start_gap)
    } else {
        (Edge::End, end_gap)
    };
    EdgeDistance {
        edge,
        logical_pixels: gap.max(0) as f64 / scale,
    }
}

fn clamp_axis(value: i64, start: i32, work_length: u32, window_length: u32) -> i32 {
    let minimum = start as i64;
    let maximum = minimum + work_length.saturating_sub(window_length) as i64;
    value
        .clamp(minimum, maximum)
        .clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn project_axis(
    distance: EdgeDistance,
    start: i32,
    work_length: u32,
    window_length: u32,
    scale: f64,
) -> i32 {
    let gap = (distance.logical_pixels.max(0.0) * scale).round() as i64;
    let end = start as i64 + work_length as i64 - window_length as i64;
    let value = match distance.edge {
        Edge::Start => start as i64 + gap,
        Edge::End => end - gap,
    };
    clamp_axis(value, start, work_length, window_length)
}

impl WindowPlacement {
    pub(crate) fn capture(
        monitor_id: String,
        manual_monitor: bool,
        position: PhysicalPosition<i32>,
        size: PhysicalSize<u32>,
        work: WorkArea,
    ) -> Self {
        let right = work.position.x as i64 + work.size.width as i64;
        let bottom = work.position.y as i64 + work.size.height as i64;
        let horizontal = edge_distance(
            position.x as i64 - work.position.x as i64,
            right - position.x as i64 - size.width as i64,
            work.scale(),
        );
        let vertical = edge_distance(
            position.y as i64 - work.position.y as i64,
            bottom - position.y as i64 - size.height as i64,
            work.scale(),
        );
        Self {
            monitor_id,
            manual_monitor,
            x: position.x,
            y: position.y,
            work_x: work.position.x,
            work_y: work.position.y,
            work_width: work.size.width,
            work_height: work.size.height,
            window_width: size.width,
            window_height: size.height,
            scale_factor: work.scale(),
            horizontal,
            vertical,
        }
    }

    pub(crate) fn position_on(
        &self,
        monitor: &tauri::Monitor,
        size: PhysicalSize<u32>,
    ) -> PhysicalPosition<i32> {
        let work = WorkArea::of(monitor);
        let unchanged = self.monitor_id == monitor_id(monitor)
            && self.work_x == work.position.x
            && self.work_y == work.position.y
            && self.work_width == work.size.width
            && self.work_height == work.size.height
            && self.window_width == size.width
            && self.window_height == size.height
            && (self.scale_factor - work.scale()).abs() < 0.001;
        let screen = monitor.position();
        let screen_size = monitor.size();
        let saved_visible = self.x >= screen.x
            && self.y >= screen.y
            && self.x as i64 + size.width as i64 <= screen.x as i64 + screen_size.width as i64
            && self.y as i64 + size.height as i64 <= screen.y as i64 + screen_size.height as i64;
        if unchanged && saved_visible {
            return PhysicalPosition::new(self.x, self.y);
        }
        let x = if unchanged {
            clamp_axis(self.x as i64, work.position.x, work.size.width, size.width)
        } else {
            project_axis(
                self.horizontal,
                work.position.x,
                work.size.width,
                size.width,
                work.scale(),
            )
        };
        let y = if unchanged {
            clamp_axis(
                self.y as i64,
                work.position.y,
                work.size.height,
                size.height,
            )
        } else {
            project_axis(
                self.vertical,
                work.position.y,
                work.size.height,
                size.height,
                work.scale(),
            )
        };
        PhysicalPosition::new(x, y)
    }

    /// 自动迁屏仅更新当前坐标，保留用户最后一次拖动产生的贴边距离。
    pub(crate) fn relocated(
        &self,
        monitor: &tauri::Monitor,
        size: PhysicalSize<u32>,
        position: PhysicalPosition<i32>,
        manual_monitor: bool,
    ) -> Self {
        let work = WorkArea::of(monitor);
        Self {
            monitor_id: monitor_id(monitor),
            manual_monitor,
            x: position.x,
            y: position.y,
            work_x: work.position.x,
            work_y: work.position.y,
            work_width: work.size.width,
            work_height: work.size.height,
            window_width: size.width,
            window_height: size.height,
            scale_factor: work.scale(),
            horizontal: self.horizontal,
            vertical: self.vertical,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DisplayEntry {
    id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    work_x: i32,
    work_y: i32,
    work_width: u32,
    work_height: u32,
    scale_bits: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DisplaySnapshot {
    primary_id: Option<String>,
    entries: Vec<DisplayEntry>,
}

impl DisplaySnapshot {
    fn new(monitors: &[tauri::Monitor], primary: Option<&tauri::Monitor>) -> Self {
        let mut entries = monitors
            .iter()
            .map(|monitor| {
                let position = monitor.position();
                let size = monitor.size();
                let work = monitor.work_area();
                DisplayEntry {
                    id: monitor_id(monitor),
                    x: position.x,
                    y: position.y,
                    width: size.width,
                    height: size.height,
                    work_x: work.position.x,
                    work_y: work.position.y,
                    work_width: work.size.width,
                    work_height: work.size.height,
                    scale_bits: monitor.scale_factor().to_bits(),
                }
            })
            .collect::<Vec<_>>();
        entries
            .sort_by(|left, right| (&left.id, left.x, left.y).cmp(&(&right.id, right.x, right.y)));
        Self {
            primary_id: primary.map(monitor_id),
            entries,
        }
    }
}

#[derive(Clone, Copy)]
struct MoveGuard {
    expected: PhysicalPosition<i32>,
    until: Instant,
}

#[derive(Default)]
pub(crate) struct PlacementRuntime {
    pub(crate) record: Option<WindowPlacement>,
    snapshot: Option<DisplaySnapshot>,
    // 阻止窗口管理器迟到的自动移动事件覆盖用户位置。
    guard: Option<MoveGuard>,
    pub(crate) waiting_for_fit: bool,
}

impl PlacementRuntime {
    pub(crate) fn initialize(&mut self, record: Option<WindowPlacement>) {
        if self.record.is_none() {
            self.record = record;
        }
    }

    pub(crate) fn displays_changed(
        &mut self,
        monitors: &[tauri::Monitor],
        primary: Option<&tauri::Monitor>,
    ) -> bool {
        let next = DisplaySnapshot::new(monitors, primary);
        let changed = self
            .snapshot
            .as_ref()
            .is_some_and(|previous| previous != &next);
        self.snapshot = Some(next);
        changed
    }

    pub(crate) fn target<'a>(
        &self,
        monitors: &'a [tauri::Monitor],
        primary: Option<&tauri::Monitor>,
    ) -> Option<&'a tauri::Monitor> {
        let chosen = self
            .record
            .as_ref()
            .filter(|record| record.manual_monitor)
            .and_then(|record| {
                monitors
                    .iter()
                    .find(|monitor| monitor_id(monitor) == record.monitor_id)
            });
        chosen
            .or_else(|| {
                primary.and_then(|monitor| {
                    monitors
                        .iter()
                        .find(|candidate| monitor_id(candidate) == monitor_id(monitor))
                })
            })
            .or_else(|| monitors.first())
    }

    pub(crate) fn place(
        &mut self,
        monitor: &tauri::Monitor,
        size: PhysicalSize<u32>,
        default: PhysicalPosition<i32>,
    ) -> PhysicalPosition<i32> {
        let position = self
            .record
            .as_ref()
            .map_or(default, |record| record.position_on(monitor, size));
        let manual = self.record.as_ref().is_some_and(|record| {
            record.manual_monitor && record.monitor_id == monitor_id(monitor)
        });
        self.record = Some(match self.record.as_ref() {
            Some(record) => record.relocated(monitor, size, position, manual),
            None => WindowPlacement::capture(
                monitor_id(monitor),
                false,
                position,
                size,
                WorkArea::of(monitor),
            ),
        });
        self.mark_programmatic(position);
        position
    }

    pub(crate) fn mark_programmatic(&mut self, position: PhysicalPosition<i32>) {
        self.guard = Some(MoveGuard {
            expected: position,
            until: Instant::now() + AUTOMATIC_MOVE_GUARD,
        });
    }

    pub(crate) fn ignore_move(
        &mut self,
        position: PhysicalPosition<i32>,
        user_dragging: bool,
    ) -> bool {
        if user_dragging {
            self.guard = None;
            return false;
        }
        let Some(guard) = self.guard else {
            return false;
        };
        if guard.expected.x.abs_diff(position.x) <= 2 && guard.expected.y.abs_diff(position.y) <= 2
        {
            return true;
        }
        if Instant::now() < guard.until || cfg!(target_os = "macos") {
            return true;
        }
        self.guard = None;
        false
    }

    pub(crate) fn expected_position(&self) -> Option<PhysicalPosition<i32>> {
        self.guard.map(|guard| guard.expected)
    }

    pub(crate) fn suppress_automatic_event(&self, user_active: bool) -> bool {
        !user_active
            && self
                .guard
                .is_some_and(|guard| Instant::now() < guard.until || cfg!(target_os = "macos"))
    }

    pub(crate) fn capture_user(
        &mut self,
        monitor: &tauri::Monitor,
        primary: Option<&tauri::Monitor>,
        position: PhysicalPosition<i32>,
        size: PhysicalSize<u32>,
    ) {
        self.guard = None;
        self.record = Some(WindowPlacement::capture(
            monitor_id(monitor),
            primary.is_some_and(|main| monitor_id(main) != monitor_id(monitor)),
            position,
            size,
            WorkArea::of(monitor),
        ));
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Default)]
pub(crate) struct LyricsWindowPlacements {
    pub(crate) overlay: Mutex<PlacementRuntime>,
    pub(crate) list: Mutex<PlacementRuntime>,
}

pub(crate) fn primary_monitor(
    window: &tauri::WebviewWindow,
    monitors: &[tauri::Monitor],
) -> Option<tauri::Monitor> {
    window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| monitors.first().cloned())
}
