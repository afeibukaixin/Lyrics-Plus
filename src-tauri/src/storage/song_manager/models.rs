use serde::Serialize;
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalIdentifier {
    pub external_id: i64,
    pub namespace: String,
    pub id_kind: String,
    pub value: String,
    pub confidence: u8,
    pub confirmed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackObservation {
    pub observation_id: i64,
    pub track_key: String,
    pub platform: String,
    pub raw_title: String,
    pub raw_artists: Vec<String>,
    pub raw_album: Option<String>,
    pub duration_ms: Option<u64>,
    pub observed_at: i64,
    pub recording_id: i64,
    pub split_from_recording_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingView {
    pub recording_id: i64,
    pub title: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub version_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SongAssociationCandidate {
    pub kind: SongCandidateKind,
    pub gates: AssociationGates,
    pub score_weights: crate::lyrics::provider::MatchWeights,
    pub conflicts: Vec<String>,
    pub warnings: Vec<String>,
    pub can_associate: bool,
    pub lyrics_content_same: bool,
    pub recording_id: i64,
    pub title: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub version_tags: Vec<String>,
    pub observations: Vec<TrackObservation>,
    pub external_identifiers: Vec<ExternalIdentifier>,
    pub is_original_recording: bool,
    pub title_similarity: f64,
    pub artist_similarity: f64,
    pub album_similarity: f64,
    pub duration_delta_ms: Option<u64>,
    pub lyrics_similarity: Option<f64>,
    pub isrc: Option<String>,
    pub shared_identifier: Option<ExternalIdentifier>,
    pub confirmed_artist_aliases: Vec<String>,
    pub score: f64,
    pub match_reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum SongCandidateKind {
    Metadata,
    SharedIdentifier,
    OriginalRelation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssociationGates {
    pub title: bool,
    pub artist: bool,
    pub duration: bool,
    pub version: bool,
    pub current_observation_id: i64,
    pub candidate_observation_id: i64,
}

#[derive(Debug, Clone)]
pub(super) struct EffectiveLyricBinding {
    pub(super) asset_id: i64,
    pub(super) selection_source: String,
    pub(super) confidence: i64,
    pub(super) evidence_json: String,
    pub(super) offset_ms: i64,
}
