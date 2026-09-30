use rusqlite::Connection;

use super::{v20::cleanup_spotify_dj_interludes, SPOTIFY_DJ_REIMPORT_CLEANUP_SCHEMA_VERSION};

pub(super) fn remove_reimported_spotify_dj_interludes(
    connection: &mut Connection,
) -> rusqlite::Result<()> {
    cleanup_spotify_dj_interludes(connection, SPOTIFY_DJ_REIMPORT_CLEANUP_SCHEMA_VERSION)
}
