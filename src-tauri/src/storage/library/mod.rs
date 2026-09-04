mod discovery;
mod index;
mod metadata;
mod scan;
mod schema;

pub(super) const LIBRARY_DIRECTORY_PREFERENCE: &str = "lyrics.library_directory";

#[cfg(test)]
use discovery::{push_discovered_file, MAX_LYRIC_FILES};
pub(super) use scan::LibraryScanCoordinator;
#[cfg(test)]
use scan::LibraryScanPhase;
pub use scan::LibraryScanStatus;
pub(super) use schema::{initialize_schema, normalize_collision_metadata};

#[cfg(test)]
use super::Storage;
#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
include!("tests.rs");
