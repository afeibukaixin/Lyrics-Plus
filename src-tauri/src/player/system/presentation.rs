//! 展示用播放状态：只缓冲暂停，不影响真实状态、曲目信息和进度。
use std::time::{Duration, Instant};

use super::super::PlaybackAction;
use super::metadata::TimedInfo;

const PAUSE_CONFIRM_DELAY: Duration = Duration::from_millis(300);
// 控制命令已接受但系统事件尚未到达时，只在一次查询超时范围内等待暂停反馈。
const CONTROL_CONFIRM_TIMEOUT: Duration = Duration::from_secs(3);

pub(super) struct ControlContext {
    pub(super) action: PlaybackAction,
    pause_requested: bool,
    source_epoch: u64,
    command_revision: u64,
}

#[derive(Default)]
pub(super) struct PlaybackPresentation {
    pub(super) playing: bool,
    running: bool,
    raw_playing: bool,
    source: Option<String>,
    source_epoch: u64,
    revision: u64,
    pending_pause: Option<(u64, Instant)>,
    explicit_pause_until: Option<Instant>,
    command_revision: u64,
}

impl PlaybackPresentation {
    fn cancel_pending(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.pending_pause = None;
    }

    pub(super) fn clear(&mut self) {
        self.cancel_pending();
        self.source_epoch = self.source_epoch.wrapping_add(1);
        self.source = None;
        self.running = false;
        self.raw_playing = false;
        self.playing = false;
        self.explicit_pause_until = None;
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
        self.raw_playing = next.info.is_playing == Some(true);
        if self
            .explicit_pause_until
            .is_some_and(|deadline| now >= deadline)
        {
            self.explicit_pause_until = None;
        }
        if self.raw_playing {
            self.cancel_pending();
            self.playing = true;
        } else if source_changed || self.explicit_pause_until.take().is_some() {
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

    pub(super) fn prepare_control(
        &mut self,
        action: PlaybackAction,
    ) -> Result<ControlContext, String> {
        if !self.running {
            return Err("当前没有可控制的系统媒体".into());
        }
        let action =
            if action == PlaybackAction::TogglePlayPause && self.playing && !self.raw_playing {
                PlaybackAction::Pause
            } else {
                action
            };
        self.command_revision = self.command_revision.wrapping_add(1);
        Ok(ControlContext {
            action,
            pause_requested: action == PlaybackAction::Pause
                || (action == PlaybackAction::TogglePlayPause && self.raw_playing),
            source_epoch: self.source_epoch,
            command_revision: self.command_revision,
        })
    }

    /// 只处理成功命令；过期命令或旧来源的回执不能确认当前曲目的暂停。
    pub(super) fn acknowledge_control(&mut self, context: &ControlContext) -> bool {
        if context.source_epoch != self.source_epoch
            || context.command_revision != self.command_revision
        {
            return false;
        }
        self.explicit_pause_until = None;
        if !context.pause_requested {
            return false;
        }
        if self.raw_playing {
            self.explicit_pause_until = Some(Instant::now() + CONTROL_CONFIRM_TIMEOUT);
            false
        } else {
            self.cancel_pending();
            let changed = self.playing;
            self.playing = false;
            changed
        }
    }
}
