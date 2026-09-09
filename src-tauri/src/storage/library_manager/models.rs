use serde::Serialize;

use crate::lyrics::LyricsDocument;
use crate::storage::{
    LyricAsset, LyricsBinding, PlatformLyricOverride, RecordingIdentity, SongAssociationCandidate,
};

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLyricPage {
    pub items: Vec<LibraryLyricSummary>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub status_counts: LibraryLyricStatusCounts,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLyricStatusCounts {
    pub in_use: u64,
    pub candidate: u64,
    pub unbound: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIndexEntry {
    pub index_kind: String,
    pub phase: String,
    pub processed: u64,
    pub total: u64,
    pub pending: u64,
    pub revision: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIndexStatus {
    pub indexes: Vec<LibraryIndexEntry>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySongSource {
    pub platform: String,
    pub source_app_bundle_id: Option<String>,
    pub source_app_name: Option<String>,
    pub observation_count: u64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySongSummary {
    pub recording_id: i64,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub version_tags: Vec<String>,
    #[serde(default)]
    pub source_count: u64,
    #[serde(default)]
    pub sources: Vec<LibrarySongSource>,
    pub lyric_count: u64,
    pub default_lyric: Option<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongSimilarityPair {
    pub pair_id: String,
    pub songs: [LibrarySongSummary; 2],
    pub recommended_recording_id: i64,
    pub evidence_source_recording_id: i64,
    pub evidence: SongAssociationCandidate,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLyricSummary {
    pub asset_id: i64,
    pub title: String,
    pub artist: String,
    pub source_name: String,
    pub source_kind: String,
    pub original_format: String,
    pub language: String,
    pub has_word_timing: bool,
    pub has_translation: bool,
    pub has_romanization: bool,
    pub available: bool,
    pub active_count: u64,
    pub binding_count: u64,
    pub source_count: u64,
    pub file_size: u64,
    pub status: String,
    #[serde(default)]
    pub can_cleanup: bool,
    pub content_fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLyricBatchFailure {
    pub asset_id: i64,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearCandidateLyricsResult {
    pub processed_assets: u64,
    pub removed_bindings: u64,
    pub failures: Vec<LibraryLyricBatchFailure>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryArtistSummary {
    pub artist_id: i64,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub song_count: u64,
    pub raw_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySongDetail {
    pub recording: RecordingIdentity,
    pub bindings: Vec<LyricsBinding>,
    pub platform_overrides: Vec<PlatformLyricOverride>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLyricSource {
    pub source_id: Option<i64>,
    pub source_kind: String,
    pub source_name: String,
    pub root_id: Option<String>,
    pub root_name: Option<String>,
    pub relative_path: Option<String>,
    pub available: bool,
    pub writable: bool,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLyricRecording {
    pub recording_id: i64,
    pub title: String,
    pub artists: Vec<String>,
    pub is_default: bool,
    pub offset_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLyricDetail {
    pub summary: LibraryLyricSummary,
    pub asset: LyricAsset,
    pub sources: Vec<LibraryLyricSource>,
    pub recordings: Vec<LibraryLyricRecording>,
    pub document: Option<LyricsDocument>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryArtistDetail {
    pub summary: LibraryArtistSummary,
    pub songs: Vec<LibrarySongSummary>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricSimilarityGroup {
    pub group_id: String,
    pub score: f64,
    pub high_similarity: bool,
    pub recommended_asset_id: i64,
    pub duration_warning: bool,
    pub version_warning: bool,
    pub items: Vec<LibraryLyricSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryLyricSimilarityPage {
    pub items: Vec<LyricSimilarityGroup>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub previews: std::collections::HashMap<i64, Option<LyricsDocument>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnboundCleanupPreview {
    pub items: Vec<LibraryLyricSummary>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
    pub file_count: u64,
    pub total_size: u64,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItemResult {
    pub asset_id: i64,
    pub deleted_files: u64,
    pub released_bytes: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnboundCleanupResult {
    pub items: Vec<CleanupItemResult>,
    pub deleted_files: u64,
    pub released_bytes: u64,
}
