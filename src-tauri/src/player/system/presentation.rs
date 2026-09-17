//! 展示用播放状态：只缓冲暂停，不影响真实状态、曲目信息和进度。
use std::time::{Duration, Instant};

use super::metadata::TimedInfo;

const PAUSE_CONFIRM_DELAY: Duration = Duration::from_millis(300);

#[derive(Default)]
pub(super) struct PlaybackPresentation {
    pub(super) playing: bool,
    running: bool,
    source: Option<String>,
    revision: u64,
    pending_pause: Option<(u64, Instant)>,
}

impl PlaybackPresentation {
    fn cancel_pending(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.pending_pause = None;
    }

    pub(super) fn clear(&mut self) {
        self.cancel_pending();
        self.source = None;
        self.running = false;
        self.playing = false;
    }

    pub(super) fn update(&mut self, next: Option<&TimedInfo>) {
        let Some(next) = next else {
            self.clear();
            return;
        };
        let now = Instant::now();
        let source_changed = !self.running || self.source != next.info.bundle_id;
        if source_changed {
            self.clear();
            self.source = next.info.bundle_id.clone();
            self.running = true;
        }
        if next.info.is_playing == Some(true) {
            self.cancel_pending();
            self.playing = true;
        } else if source_changed {
            self.cancel_pending();
            self.playing = false;
        } else if self.playing && self.pending_pause.is_none() {
            self.revision = self.revision.wrapping_add(1);
            self.pending_pause = Some((self.revision, now + PAUSE_CONFIRM_DELAY));
        }
        // 同源切歌、封面补全、重复暂停均不重置已经开始的确认窗口。
    }

    pub(super) fn pending_pause(&self) -> Option<(u64, Instant)> {
        self.pending_pause
    }

    pub(super) fn confirm_pause(&mut self, revision: u64, deadline: Instant) -> bool {
        if self.pending_pause != Some((revision, deadline)) || Instant::now() < deadline {
            return false;
        }
        self.cancel_pending();
        self.playing = false;
        true
    }
}
