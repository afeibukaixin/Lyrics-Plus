mod adapter;
mod artwork;
mod compat;
mod metadata;
mod palette;
mod service;

pub use service::SystemMediaService;

#[cfg(test)]
use adapter::sync_elapsed_from_adapter;
#[cfg(test)]
use media_remote::NowPlayingInfo;
#[cfg(test)]
use metadata::{
    milliseconds, snapshot_from_info, system_track_id, timed_info, valid_elapsed_time, TimedInfo,
};
#[cfg(test)]
use std::sync::RwLock;
#[cfg(test)]
use std::time::{Duration, Instant, SystemTime};

#[cfg(test)]
include!("tests.rs");
