mod audio_tap;
mod dsp;
mod input;
mod model;
mod service;
mod worker;

pub const PLAYBACK_SPECTRUM_FRAME_EVENT: &str = "playback://spectrum-frame";
pub const PLAYBACK_SPECTRUM_STATE_EVENT: &str = "playback://spectrum-state";

use std::time::Duration;

pub(super) const FRAME_INTERVAL: Duration = Duration::from_millis(33);

pub use model::PlaybackSpectrumState;
pub use service::PlaybackSpectrumService;
