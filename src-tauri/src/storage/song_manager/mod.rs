mod associations;
mod candidates;
mod identity;
mod lyrics;
mod models;

pub(super) use candidates::collect_song_association_candidates_for_targets;
pub(super) use identity::exact_track_external_id;
pub(super) use identity::recording_version_tags;
pub(super) use identity::{
    artist_alias_ids_for_removal, confirmed_artist_alias_groups, confirmed_artist_aliases,
    equivalent_artist_names, normalized_artist_alias, normalized_identity_artist, recording_view,
};
pub(super) use lyrics::{
    lyric_asset_content_paths, normalize_lyric_content, normalize_platform_lyrics,
};
pub(super) use models::SongCandidateKind;
pub use models::{ExternalIdentifier, RecordingView, SongAssociationCandidate, TrackObservation};
