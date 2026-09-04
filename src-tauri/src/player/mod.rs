pub(crate) mod automation;
mod control;
mod model;
mod routing;
mod spectrum;
mod system;

pub(crate) use control::{control_playback, run_with_timeout, seek_playback};
pub use model::{
    PlaybackAction, PlaybackArtwork, PlaybackErrorCode, PlaybackSnapshot, PlaybackSpectrumColors,
    PlaybackSpectrumColumnColors, PlayerKind, PlayerSelection,
};
pub use routing::query_selected_player;
pub use spectrum::{PlaybackSpectrumService, PlaybackSpectrumState};
pub use system::SystemMediaService;

use model::{ensure_track_id, normalized_track_component, now_ms};

#[cfg(test)]
use crate::config::{RegisteredApplication, SystemMediaFilterMode};
#[cfg(test)]
use routing::{filter_system_source, query_auto_player, system_source_allowed};

#[cfg(test)]
include!("tests.rs");
