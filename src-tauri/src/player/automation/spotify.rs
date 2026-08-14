use objc2_core_services::AEKeyword;

use super::super::{PlaybackSnapshot, PlayerKind};

const BUNDLE_ID: &str = "com.spotify.client";
const TRACK_ID: AEKeyword = u32::from_be_bytes(*b"ID  ");

pub(super) fn snapshot() -> PlaybackSnapshot {
    super::query(PlayerKind::Spotify, BUNDLE_ID, 1, TRACK_ID)
}
