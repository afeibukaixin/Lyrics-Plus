use rusqlite::{params, Connection};

mod v01_v07;
mod v08_v14;
mod v15_v17;

use v01_v07::{
    migrate_artist_credit_schema, migrate_base_schema, migrate_compatibility_schema,
    migrate_external_id_schema, migrate_lyric_asset_schema, migrate_recording_schema,
    migrate_track_observation_schema,
};
use v08_v14::{
    migrate_library_root_schema, migrate_recording_split_schema, migrate_search_run_schema,
    migrate_search_timing_schema, migrate_search_trace_schema, migrate_v2_compatibility_schema,
};
use v15_v17::{
    migrate_library_manager_schema, migrate_library_performance_repair_schema,
    migrate_library_performance_schema, migrate_song_similarity_schema,
    migrate_system_source_app_schema,
};

pub(super) const BASE_SCHEMA_VERSION: i64 = 1;
pub(super) const COMPATIBILITY_SCHEMA_VERSION: i64 = 2;
pub(super) const RECORDING_SCHEMA_VERSION: i64 = 3;
pub(super) const TRACK_OBSERVATION_SCHEMA_VERSION: i64 = 4;
pub(super) const EXTERNAL_ID_SCHEMA_VERSION: i64 = 5;
pub(super) const ARTIST_CREDIT_SCHEMA_VERSION: i64 = 6;
pub(super) const LYRIC_ASSET_SCHEMA_VERSION: i64 = 7;
pub(super) const LIBRARY_ROOT_SCHEMA_VERSION: i64 = 8;
pub(super) const SEARCH_RUN_SCHEMA_VERSION: i64 = 9;
pub(super) const SEARCH_TRACE_SCHEMA_VERSION: i64 = 10;
pub(super) const V2_COMPATIBILITY_SCHEMA_VERSION: i64 = 11;
pub(super) const RECORDING_SPLIT_SCHEMA_VERSION: i64 = 13;
pub(super) const SEARCH_TIMING_SCHEMA_VERSION: i64 = 14;
pub(super) const LIBRARY_MANAGER_SCHEMA_VERSION: i64 = 15;
pub(super) const SONG_SIMILARITY_SCHEMA_VERSION: i64 = 16;
pub(super) const LIBRARY_PERFORMANCE_SCHEMA_VERSION: i64 = 17;
pub(super) const LIBRARY_PERFORMANCE_REPAIR_SCHEMA_VERSION: i64 = 18;
pub(super) const SYSTEM_SOURCE_APP_SCHEMA_VERSION: i64 = 19;
pub(super) const STORAGE_SCHEMA_VERSION: i64 = SYSTEM_SOURCE_APP_SCHEMA_VERSION;

/// Applies the versioned base schema. Column backfills below remain a
/// compatibility bridge for databases created before `user_version` existed.
pub(super) fn migrate_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    ensure_migration_log_schema(connection)?;
    let version = connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
    if version > STORAGE_SCHEMA_VERSION {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                format!("歌词数据库版本 {version} 高于当前支持的版本 {STORAGE_SCHEMA_VERSION}"),
            ),
        )));
    }
    let mut current_version = version;
    for (target_version, migration) in [
        (
            BASE_SCHEMA_VERSION,
            migrate_base_schema as fn(&mut Connection) -> rusqlite::Result<()>,
        ),
        (COMPATIBILITY_SCHEMA_VERSION, migrate_compatibility_schema),
        (RECORDING_SCHEMA_VERSION, migrate_recording_schema),
        (
            TRACK_OBSERVATION_SCHEMA_VERSION,
            migrate_track_observation_schema,
        ),
        (EXTERNAL_ID_SCHEMA_VERSION, migrate_external_id_schema),
        (ARTIST_CREDIT_SCHEMA_VERSION, migrate_artist_credit_schema),
        (LYRIC_ASSET_SCHEMA_VERSION, migrate_lyric_asset_schema),
        (LIBRARY_ROOT_SCHEMA_VERSION, migrate_library_root_schema),
        (SEARCH_RUN_SCHEMA_VERSION, migrate_search_run_schema),
        (SEARCH_TRACE_SCHEMA_VERSION, migrate_search_trace_schema),
        (
            V2_COMPATIBILITY_SCHEMA_VERSION,
            migrate_v2_compatibility_schema,
        ),
        (
            RECORDING_SPLIT_SCHEMA_VERSION,
            migrate_recording_split_schema,
        ),
        (SEARCH_TIMING_SCHEMA_VERSION, migrate_search_timing_schema),
        (
            LIBRARY_MANAGER_SCHEMA_VERSION,
            migrate_library_manager_schema,
        ),
        (
            SONG_SIMILARITY_SCHEMA_VERSION,
            migrate_song_similarity_schema,
        ),
        (
            LIBRARY_PERFORMANCE_SCHEMA_VERSION,
            migrate_library_performance_schema,
        ),
        (
            LIBRARY_PERFORMANCE_REPAIR_SCHEMA_VERSION,
            migrate_library_performance_repair_schema,
        ),
        (
            SYSTEM_SOURCE_APP_SCHEMA_VERSION,
            migrate_system_source_app_schema,
        ),
    ] {
        if current_version >= target_version {
            continue;
        }
        run_migration(connection, current_version, target_version, migration)?;
        current_version = target_version;
    }
    Ok(())
}

fn ensure_migration_log_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS storage_migration_logs (
           migration_id INTEGER PRIMARY KEY AUTOINCREMENT,
           from_version INTEGER NOT NULL,
           to_version INTEGER NOT NULL,
           started_at INTEGER NOT NULL DEFAULT (unixepoch()),
           finished_at INTEGER,
           status TEXT NOT NULL CHECK (status IN ('started', 'completed', 'failed')),
           failure_reason TEXT
         );
         CREATE INDEX IF NOT EXISTS storage_migration_logs_started_at
           ON storage_migration_logs(started_at DESC);",
    )
}

fn run_migration(
    connection: &mut Connection,
    from_version: i64,
    to_version: i64,
    migration: fn(&mut Connection) -> rusqlite::Result<()>,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO storage_migration_logs (from_version, to_version, status)
         VALUES (?1, ?2, 'started')",
        params![from_version, to_version],
    )?;
    let migration_id = connection.last_insert_rowid();
    match migration(connection) {
        Ok(()) => {
            connection.execute(
                "UPDATE storage_migration_logs
                 SET finished_at=unixepoch(), status='completed', failure_reason=NULL
                 WHERE migration_id=?1",
                params![migration_id],
            )?;
            Ok(())
        }
        Err(error) => {
            let failure_reason = error.to_string();
            let _ = connection.execute(
                "UPDATE storage_migration_logs
                 SET finished_at=unixepoch(), status='failed', failure_reason=?2
                 WHERE migration_id=?1",
                params![migration_id, failure_reason],
            );
            Err(error)
        }
    }
}
