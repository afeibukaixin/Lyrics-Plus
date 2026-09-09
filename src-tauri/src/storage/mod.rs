use std::collections::{hash_map::DefaultHasher, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::lyrics::encoding::decode_lyrics_bytes;
use crate::lyrics::provider::{
    score_candidate, title_matches, LyricsSearchInput, LyricsSearchResult, KUGOU_DISPLAY_NAME,
    NETEASE_DISPLAY_NAME, QQMUSIC_DISPLAY_NAME,
};
use crate::lyrics::{parse_lrc_with_options, LyricsDocument};

pub mod library;

pub const LOCAL_PROVIDER_ID: &str = "local";
pub const LOCAL_FILE_SOURCE: &str = "本地文件";
const MAX_LOCAL_SEARCH_RESULTS: usize = 8;
const MIN_LOCAL_SEARCH_SCORE: f64 = 0.5;

pub(super) fn is_user_owned_source(source: &str) -> bool {
    matches!(source, LOCAL_FILE_SOURCE | "本地导入" | "手动导入")
}

include!("models.rs");
mod database;
include!("lyrics.rs");
include!("associations.rs");
include!("preferences.rs");
include!("helpers.rs");
include!("aliases.rs");
mod song_manager;
use song_manager::{
    artist_alias_ids_for_removal, confirmed_artist_alias_groups, equivalent_artist_names,
    lyric_asset_content_paths, normalize_lyric_content, normalized_artist_alias,
    normalized_identity_artist, recording_version_tags, recording_view,
};
pub use song_manager::{
    ExternalIdentifier, RecordingView, SongAssociationCandidate, TrackObservation,
};
include!("bindings.rs");
include!("roots.rs");
include!("search_runs.rs");
include!("v2_views.rs");
mod library_manager;
pub use library_manager::{
    ClearCandidateLyricsResult, LibraryArtistDetail, LibraryArtistSummary, LibraryIndexStatus,
    LibraryLyricDetail, LibraryLyricPage, LibraryLyricSimilarityPage, LibraryPage,
    LibrarySongDetail, LibrarySongSummary, LyricSimilarityGroup, SongSimilarityPair,
    UnboundCleanupPreview, UnboundCleanupResult,
};

#[cfg(test)]
include!("tests.rs");
