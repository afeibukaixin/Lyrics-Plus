use objc2_core_services::AEKeyword;

use super::super::{PlaybackSnapshot, PlayerKind};

const BUNDLE_ID: &str = "com.apple.Music";
const TRACK_ID: AEKeyword = u32::from_be_bytes(*b"pPIS");

pub(super) fn snapshot() -> PlaybackSnapshot {
    super::query(PlayerKind::AppleMusic, BUNDLE_ID, 1000, TRACK_ID)
}
