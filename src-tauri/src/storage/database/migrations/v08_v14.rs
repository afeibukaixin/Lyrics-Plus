use rusqlite::Connection;

use super::{
    LIBRARY_ROOT_SCHEMA_VERSION, RECORDING_SPLIT_SCHEMA_VERSION, SEARCH_RUN_SCHEMA_VERSION,
    SEARCH_TIMING_SCHEMA_VERSION, SEARCH_TRACE_SCHEMA_VERSION, V2_COMPATIBILITY_SCHEMA_VERSION,
};
use crate::storage::ensure_column;

pub(super) fn migrate_library_root_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    ensure_column(
        &transaction,
        "lyric_track_aliases",
        "evidence_kind",
        "TEXT NOT NULL DEFAULT 'unverified_legacy'",
    )?;
    ensure_column(
        &transaction,
        "lyric_files",
        "available",
        "INTEGER NOT NULL DEFAULT 1 CHECK (available IN (0, 1))",
    )?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS library_roots (
           root_id TEXT PRIMARY KEY,
           root_kind TEXT NOT NULL CHECK (root_kind IN ('managed', 'local', 'legacy', 'cache')),
           display_name TEXT NOT NULL,
           path TEXT NOT NULL UNIQUE,
           read_only INTEGER NOT NULL DEFAULT 0 CHECK (read_only IN (0, 1)),
           enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
           file_count INTEGER NOT NULL DEFAULT 0,
           unavailable_count INTEGER NOT NULL DEFAULT 0,
           last_scan_at INTEGER,
           last_error TEXT,
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           updated_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE INDEX IF NOT EXISTS library_roots_kind_enabled
           ON library_roots(root_kind, enabled);",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {LIBRARY_ROOT_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_search_run_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS lyrics_search_runs (
           run_id TEXT PRIMARY KEY,
           track_key TEXT NOT NULL,
           recording_id INTEGER REFERENCES recordings(recording_id),
           intent TEXT NOT NULL CHECK (intent IN ('automatic', 'manual')),
           title TEXT NOT NULL,
           artist TEXT NOT NULL,
           status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed')),
           started_at INTEGER NOT NULL DEFAULT (unixepoch()),
           finished_at INTEGER,
           total_elapsed_ms INTEGER,
           error_code TEXT,
           selected_provider_id TEXT,
           selected_provider_item_id TEXT,
           selection_reason TEXT
         );
         CREATE INDEX IF NOT EXISTS lyrics_search_runs_track_started
           ON lyrics_search_runs(track_key, started_at DESC);
         CREATE TABLE IF NOT EXISTS lyrics_search_candidates (
           candidate_id INTEGER PRIMARY KEY AUTOINCREMENT,
           run_id TEXT NOT NULL REFERENCES lyrics_search_runs(run_id) ON DELETE CASCADE,
           provider_id TEXT NOT NULL,
           provider_item_id TEXT NOT NULL,
           title TEXT NOT NULL,
           artists_json TEXT NOT NULL DEFAULT '[]',
           album TEXT,
           duration_ms INTEGER,
           version_tags_json TEXT NOT NULL DEFAULT '[]',
           score REAL NOT NULL DEFAULT 0,
           state TEXT NOT NULL CHECK (state IN ('fetched', 'selected', 'rejected')),
           rejection_reason TEXT,
           selection_reason TEXT,
           UNIQUE(run_id, provider_id, provider_item_id)
         );
         CREATE INDEX IF NOT EXISTS lyrics_search_candidates_run
           ON lyrics_search_candidates(run_id, score DESC);
         CREATE TABLE IF NOT EXISTS lyrics_search_providers (
           provider_run_id INTEGER PRIMARY KEY AUTOINCREMENT,
           run_id TEXT NOT NULL REFERENCES lyrics_search_runs(run_id) ON DELETE CASCADE,
           provider_id TEXT NOT NULL,
           status TEXT NOT NULL,
           candidate_count INTEGER NOT NULL DEFAULT 0,
           elapsed_ms INTEGER NOT NULL DEFAULT 0,
           UNIQUE(run_id, provider_id)
         );
         CREATE INDEX IF NOT EXISTS lyrics_search_providers_run
           ON lyrics_search_providers(run_id);",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {SEARCH_RUN_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_search_trace_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    ensure_column(
        &transaction,
        "lyrics_search_candidates",
        "score_evidence_json",
        "TEXT NOT NULL DEFAULT '{}'",
    )?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS lyrics_search_stages (
           stage_id INTEGER PRIMARY KEY AUTOINCREMENT,
           run_id TEXT NOT NULL REFERENCES lyrics_search_runs(run_id) ON DELETE CASCADE,
           stage TEXT NOT NULL,
           provider_id TEXT,
           provider_key TEXT NOT NULL DEFAULT '',
           status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed')),
           started_at INTEGER NOT NULL DEFAULT (unixepoch()),
           finished_at INTEGER,
           elapsed_ms INTEGER NOT NULL DEFAULT 0,
           candidate_count INTEGER NOT NULL DEFAULT 0,
           UNIQUE(run_id, stage, provider_key)
         );
         CREATE INDEX IF NOT EXISTS lyrics_search_stages_run
           ON lyrics_search_stages(run_id, stage_id);",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {SEARCH_TRACE_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_v2_compatibility_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    // v10 databases created these tables before source_name was introduced.
    // Backfill the columns so old V2 bindings can be rebuilt during startup.
    ensure_column(
        &transaction,
        "lyric_assets",
        "source_name",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        &transaction,
        "lyric_asset_sources",
        "source_name",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {V2_COMPATIBILITY_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_search_timing_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    ensure_column(
        &transaction,
        "lyrics_search_runs",
        "total_elapsed_ms",
        "INTEGER",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {SEARCH_TIMING_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_recording_split_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    ensure_column(
        &transaction,
        "track_observations",
        "split_from_recording_id",
        "INTEGER REFERENCES recordings(recording_id)",
    )?;
    transaction.execute_batch(
        "CREATE INDEX IF NOT EXISTS track_observations_split_source
           ON track_observations(split_from_recording_id);",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {RECORDING_SPLIT_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}
