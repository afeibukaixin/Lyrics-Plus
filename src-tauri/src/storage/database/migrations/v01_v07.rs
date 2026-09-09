use rusqlite::Connection;

use super::{
    ARTIST_CREDIT_SCHEMA_VERSION, BASE_SCHEMA_VERSION, COMPATIBILITY_SCHEMA_VERSION,
    EXTERNAL_ID_SCHEMA_VERSION, LYRIC_ASSET_SCHEMA_VERSION, RECORDING_SCHEMA_VERSION,
    TRACK_OBSERVATION_SCHEMA_VERSION,
};
use crate::storage::library;
use crate::storage::{ensure_column, migrate_provider_source_names};

pub(super) fn migrate_base_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS lyric_associations (
           track_key TEXT PRIMARY KEY,
           title TEXT NOT NULL,
           artist TEXT NOT NULL,
           source TEXT NOT NULL,
           content_path TEXT NOT NULL,
           offset_ms INTEGER NOT NULL DEFAULT 0,
           original_format TEXT NOT NULL DEFAULT 'lrc',
           manual_selected INTEGER NOT NULL DEFAULT 0,
           provider_id TEXT,
           provider_item_id TEXT,
           updated_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE TABLE IF NOT EXISTS lyric_files (
           content_path TEXT PRIMARY KEY,
           title TEXT NOT NULL,
           artist TEXT NOT NULL,
           source TEXT NOT NULL,
           original_format TEXT NOT NULL DEFAULT 'lrc',
           manual_selected INTEGER NOT NULL DEFAULT 0,
           content_hash TEXT NOT NULL,
           updated_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE TABLE IF NOT EXISTS app_preferences (
           key TEXT PRIMARY KEY,
           value TEXT NOT NULL,
           updated_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE TABLE IF NOT EXISTS lyric_history (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           track_key TEXT NOT NULL,
           title TEXT NOT NULL,
           artist TEXT NOT NULL,
           source TEXT NOT NULL,
           used_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE TABLE IF NOT EXISTS lyric_track_aliases (
           observed_track_key TEXT PRIMARY KEY,
           canonical_track_key TEXT NOT NULL,
           title_norm TEXT NOT NULL,
           artist_norm TEXT NOT NULL,
           album_norm TEXT,
           duration_ms INTEGER,
           evidence_kind TEXT NOT NULL DEFAULT 'unverified_legacy',
           updated_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE TABLE IF NOT EXISTS storage_migration_logs (
           migration_id INTEGER PRIMARY KEY AUTOINCREMENT,
           from_version INTEGER NOT NULL,
           to_version INTEGER NOT NULL,
           started_at INTEGER NOT NULL DEFAULT (unixepoch()),
           finished_at INTEGER,
           status TEXT NOT NULL CHECK (status IN ('started', 'completed', 'failed')),
           failure_reason TEXT
         );
         CREATE INDEX IF NOT EXISTS lyric_history_used_at
           ON lyric_history(used_at DESC);
         CREATE INDEX IF NOT EXISTS lyric_track_aliases_canonical
           ON lyric_track_aliases(canonical_track_key);
         CREATE INDEX IF NOT EXISTS lyric_track_aliases_identity
           ON lyric_track_aliases(title_norm, artist_norm);
         CREATE INDEX IF NOT EXISTS lyric_files_title_artist
           ON lyric_files(title, artist);
         CREATE INDEX IF NOT EXISTS storage_migration_logs_started_at
           ON storage_migration_logs(started_at DESC);",
    )?;
    transaction.execute_batch(&format!("PRAGMA user_version = {BASE_SCHEMA_VERSION};"))?;
    transaction.commit()
}

pub(super) fn migrate_compatibility_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    ensure_column(
        &transaction,
        "lyric_associations",
        "original_format",
        "TEXT NOT NULL DEFAULT 'lrc'",
    )?;
    library::initialize_schema(&transaction)?;
    let added_manual_selected = ensure_column(
        &transaction,
        "lyric_associations",
        "manual_selected",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(&transaction, "lyric_associations", "provider_id", "TEXT")?;
    ensure_column(
        &transaction,
        "lyric_associations",
        "provider_item_id",
        "TEXT",
    )?;
    if added_manual_selected {
        transaction.execute("UPDATE lyric_associations SET manual_selected=1", [])?;
    } else {
        transaction.execute(
            "UPDATE lyric_associations SET manual_selected=1
             WHERE source IN ('本地导入', '手动导入')",
            [],
        )?;
    }
    migrate_provider_source_names(&transaction)?;
    library::normalize_collision_metadata(&transaction)?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {COMPATIBILITY_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_recording_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS recordings (
           recording_id INTEGER PRIMARY KEY AUTOINCREMENT,
           title TEXT NOT NULL CHECK (length(trim(title)) > 0),
           album TEXT,
           duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
           version_tags_json TEXT NOT NULL DEFAULT '[\"original\"]',
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           updated_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE INDEX IF NOT EXISTS recordings_title_album
           ON recordings(title, album);
         CREATE INDEX IF NOT EXISTS recordings_duration
           ON recordings(duration_ms);",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {RECORDING_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_track_observation_schema(
    connection: &mut Connection,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS track_observations (
           observation_id INTEGER PRIMARY KEY AUTOINCREMENT,
           track_key TEXT NOT NULL,
           platform TEXT NOT NULL,
           raw_title TEXT NOT NULL,
           raw_artists_json TEXT NOT NULL DEFAULT '[]',
           raw_album TEXT,
           duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
           observed_at INTEGER NOT NULL DEFAULT (unixepoch()),
           recording_id INTEGER NOT NULL REFERENCES recordings(recording_id),
           UNIQUE(platform, track_key)
         );
         CREATE INDEX IF NOT EXISTS track_observations_recording
           ON track_observations(recording_id);
         CREATE INDEX IF NOT EXISTS track_observations_platform
           ON track_observations(platform);",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {TRACK_OBSERVATION_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_external_id_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS recording_external_ids (
           external_id INTEGER PRIMARY KEY AUTOINCREMENT,
           namespace TEXT NOT NULL CHECK (length(trim(namespace)) > 0),
           id_kind TEXT NOT NULL CHECK (length(trim(id_kind)) > 0),
           value TEXT NOT NULL CHECK (length(trim(value)) > 0),
           recording_id INTEGER NOT NULL REFERENCES recordings(recording_id),
           confidence INTEGER NOT NULL DEFAULT 0 CHECK (confidence BETWEEN 0 AND 100),
           confirmed INTEGER NOT NULL DEFAULT 0 CHECK (confirmed IN (0, 1)),
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
           UNIQUE(namespace, id_kind, value)
         );
         CREATE INDEX IF NOT EXISTS recording_external_ids_recording
           ON recording_external_ids(recording_id);
         CREATE INDEX IF NOT EXISTS recording_external_ids_lookup
           ON recording_external_ids(namespace, id_kind, value);",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {EXTERNAL_ID_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_artist_credit_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS artists (
           artist_id INTEGER PRIMARY KEY AUTOINCREMENT,
           canonical_name TEXT NOT NULL CHECK (length(trim(canonical_name)) > 0),
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           updated_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE TABLE IF NOT EXISTS recording_artist_credits (
           credit_id INTEGER PRIMARY KEY AUTOINCREMENT,
           recording_id INTEGER NOT NULL REFERENCES recordings(recording_id),
           artist_id INTEGER REFERENCES artists(artist_id),
           raw_name TEXT NOT NULL CHECK (length(trim(raw_name)) > 0),
           credit_order INTEGER NOT NULL CHECK (credit_order >= 0),
           role TEXT NOT NULL CHECK (role IN ('lead', 'featured', 'unknown')),
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
           UNIQUE(recording_id, credit_order)
         );
         CREATE INDEX IF NOT EXISTS recording_artist_credits_recording
           ON recording_artist_credits(recording_id);
         CREATE INDEX IF NOT EXISTS recording_artist_credits_artist
           ON recording_artist_credits(artist_id);
         CREATE TABLE IF NOT EXISTS artist_aliases (
           alias_id INTEGER PRIMARY KEY AUTOINCREMENT,
           artist_id INTEGER NOT NULL REFERENCES artists(artist_id),
           alias TEXT NOT NULL CHECK (length(trim(alias)) > 0),
           normalized_alias TEXT NOT NULL CHECK (length(trim(normalized_alias)) > 0),
           confirmed INTEGER NOT NULL DEFAULT 0 CHECK (confirmed IN (0, 1)),
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
           UNIQUE(artist_id, normalized_alias)
         );
         CREATE INDEX IF NOT EXISTS artist_aliases_lookup
           ON artist_aliases(normalized_alias, confirmed);",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {ARTIST_CREDIT_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_lyric_asset_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS lyric_assets (
           asset_id INTEGER PRIMARY KEY AUTOINCREMENT,
           source_kind TEXT NOT NULL CHECK (source_kind IN ('managed', 'local', 'legacy', 'cache')),
           source_name TEXT NOT NULL DEFAULT '',
           provider_id TEXT,
           provider_item_id TEXT,
           original_format TEXT NOT NULL DEFAULT 'lrc',
           language TEXT NOT NULL DEFAULT 'und',
           has_word_timing INTEGER NOT NULL DEFAULT 0 CHECK (has_word_timing IN (0, 1)),
           has_translation INTEGER NOT NULL DEFAULT 0 CHECK (has_translation IN (0, 1)),
           has_romanization INTEGER NOT NULL DEFAULT 0 CHECK (has_romanization IN (0, 1)),
           content_fingerprint TEXT NOT NULL CHECK (length(trim(content_fingerprint)) > 0),
           root_dir_id TEXT,
           relative_path TEXT,
           available INTEGER NOT NULL DEFAULT 1 CHECK (available IN (0, 1)),
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
           UNIQUE(content_fingerprint)
         );
         CREATE INDEX IF NOT EXISTS lyric_assets_provider
           ON lyric_assets(provider_id, provider_item_id);
         CREATE INDEX IF NOT EXISTS lyric_assets_available
           ON lyric_assets(available);
         CREATE TABLE IF NOT EXISTS lyric_asset_sources (
           source_id INTEGER PRIMARY KEY AUTOINCREMENT,
           asset_id INTEGER NOT NULL REFERENCES lyric_assets(asset_id),
           source_kind TEXT NOT NULL CHECK (source_kind IN ('managed', 'local', 'legacy', 'cache')),
           source_name TEXT NOT NULL DEFAULT '',
           provider_id TEXT,
           provider_item_id TEXT,
           root_dir_id TEXT,
           relative_path TEXT,
           available INTEGER NOT NULL DEFAULT 1 CHECK (available IN (0, 1)),
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
           UNIQUE(asset_id, source_kind, provider_id, provider_item_id, root_dir_id, relative_path)
         );
         CREATE INDEX IF NOT EXISTS lyric_asset_sources_asset
           ON lyric_asset_sources(asset_id);
         CREATE TABLE IF NOT EXISTS recording_lyric_bindings (
           binding_id INTEGER PRIMARY KEY AUTOINCREMENT,
           recording_id INTEGER NOT NULL REFERENCES recordings(recording_id),
           asset_id INTEGER NOT NULL REFERENCES lyric_assets(asset_id),
           selection_source TEXT NOT NULL CHECK (selection_source IN ('user', 'automatic', 'migration')),
           confidence INTEGER NOT NULL DEFAULT 0 CHECK (confidence BETWEEN 0 AND 100),
           evidence_json TEXT NOT NULL DEFAULT '{}',
           is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
           offset_ms INTEGER NOT NULL DEFAULT 0,
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
           UNIQUE(recording_id, asset_id)
         );
         CREATE INDEX IF NOT EXISTS recording_lyric_bindings_recording
           ON recording_lyric_bindings(recording_id);
         CREATE UNIQUE INDEX IF NOT EXISTS recording_lyric_bindings_default
           ON recording_lyric_bindings(recording_id) WHERE is_default=1;
         CREATE TABLE IF NOT EXISTS platform_lyric_overrides (
           override_id INTEGER PRIMARY KEY AUTOINCREMENT,
           recording_id INTEGER NOT NULL REFERENCES recordings(recording_id),
           platform TEXT NOT NULL CHECK (length(trim(platform)) > 0),
           asset_id INTEGER REFERENCES lyric_assets(asset_id),
           offset_ms INTEGER NOT NULL DEFAULT 0,
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
           UNIQUE(recording_id, platform)
         );
         CREATE INDEX IF NOT EXISTS platform_lyric_overrides_recording
           ON platform_lyric_overrides(recording_id);",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {LYRIC_ASSET_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}
