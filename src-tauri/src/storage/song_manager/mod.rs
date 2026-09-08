mod associations;
mod candidates;
mod identity;
mod lyrics;
mod models;

pub(super) use identity::exact_track_external_id;
pub(super) use identity::recording_version_tags;
pub(super) use lyrics::normalize_platform_lyrics;
pub use models::{ExternalIdentifier, RecordingView, SongAssociationCandidate, TrackObservation};
