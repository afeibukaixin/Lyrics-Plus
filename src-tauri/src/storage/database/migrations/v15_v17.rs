use rusqlite::Connection;

use super::{
    LIBRARY_MANAGER_SCHEMA_VERSION, LIBRARY_PERFORMANCE_REPAIR_SCHEMA_VERSION,
    LIBRARY_PERFORMANCE_SCHEMA_VERSION, SONG_SIMILARITY_SCHEMA_VERSION,
    SYSTEM_SOURCE_APP_SCHEMA_VERSION,
};

pub(super) fn migrate_system_source_app_schema(
    connection: &mut Connection,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "ALTER TABLE track_observations ADD COLUMN source_app_bundle_id TEXT;
         ALTER TABLE track_observations ADD COLUMN source_app_name TEXT;",
    )?;

    let observations = {
        let mut statement = transaction.prepare(
            "SELECT observation_id, track_key FROM track_observations WHERE platform='system'",
        )?;
        let observations = statement
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        observations
    };
    for (observation_id, track_key) in observations {
        let bundle_id = track_key
            .strip_prefix("system:system:")
            .and_then(|value| value.split_once('|'))
            .map(|(value, _)| value.trim())
            .filter(|value| !value.is_empty());
        if let Some(bundle_id) = bundle_id {
            transaction.execute(
                "UPDATE track_observations SET source_app_bundle_id=?2 WHERE observation_id=?1",
                rusqlite::params![observation_id, bundle_id],
            )?;
        }
    }

    // 匹配规则已改变，旧缓存中的 system 平台冲突必须重新计算。
    transaction.execute("DELETE FROM song_similarity_pairs", [])?;
    transaction.execute(
        "DELETE FROM library_similarity_state WHERE index_kind='song_similarity'",
        [],
    )?;
    transaction.execute(
        "UPDATE library_index_state
         SET phase='pending', processed=0, total=0, settings_fingerprint=NULL, last_error=NULL
         WHERE index_kind='song_similarity'",
        [],
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {SYSTEM_SOURCE_APP_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_library_manager_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS lyric_similarity_ignores (
           left_fingerprint TEXT NOT NULL CHECK (length(trim(left_fingerprint)) > 0),
           right_fingerprint TEXT NOT NULL CHECK (length(trim(right_fingerprint)) > 0),
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           PRIMARY KEY(left_fingerprint, right_fingerprint),
           CHECK(left_fingerprint < right_fingerprint)
         );",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {LIBRARY_MANAGER_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_song_similarity_schema(connection: &mut Connection) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS song_similarity_ignores (
           left_recording_id INTEGER NOT NULL,
           right_recording_id INTEGER NOT NULL,
           created_at INTEGER NOT NULL DEFAULT (unixepoch()),
           PRIMARY KEY(left_recording_id, right_recording_id),
           CHECK(left_recording_id < right_recording_id)
         );",
    )?;
    transaction.execute_batch(&format!(
        "PRAGMA user_version = {SONG_SIMILARITY_SCHEMA_VERSION};"
    ))?;
    transaction.commit()
}

pub(super) fn migrate_library_performance_schema(
    connection: &mut Connection,
) -> rusqlite::Result<()> {
    migrate_library_performance_schema_to(connection, LIBRARY_PERFORMANCE_SCHEMA_VERSION)
}

pub(super) fn migrate_library_performance_repair_schema(
    connection: &mut Connection,
) -> rusqlite::Result<()> {
    migrate_library_performance_schema_to(connection, LIBRARY_PERFORMANCE_REPAIR_SCHEMA_VERSION)
}

fn migrate_library_performance_schema_to(
    connection: &mut Connection,
    target_version: i64,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE INDEX IF NOT EXISTS recordings_library_order
           ON recordings(updated_at DESC, recording_id DESC);
         CREATE INDEX IF NOT EXISTS track_observations_recording_platform
           ON track_observations(recording_id, platform);
         CREATE INDEX IF NOT EXISTS recording_artist_credits_artist_recording
           ON recording_artist_credits(artist_id, recording_id);
         CREATE INDEX IF NOT EXISTS recording_lyric_bindings_asset_status
           ON recording_lyric_bindings(asset_id, is_default, updated_at DESC, recording_id);
         CREATE INDEX IF NOT EXISTS platform_lyric_overrides_asset_recording
           ON platform_lyric_overrides(asset_id, recording_id);
         CREATE INDEX IF NOT EXISTS lyric_assets_library_order
           ON lyric_assets(source_kind, updated_at DESC, asset_id DESC);
         CREATE INDEX IF NOT EXISTS lyric_files_hash_available_updated
           ON lyric_files(content_hash, available, updated_at DESC);

         CREATE TABLE IF NOT EXISTS library_search_terms (
           term_id INTEGER PRIMARY KEY AUTOINCREMENT,
           entity_kind TEXT NOT NULL,
           entity_id INTEGER NOT NULL,
           field_priority INTEGER NOT NULL,
           normalized_value TEXT NOT NULL,
           UNIQUE(entity_kind, entity_id, field_priority, normalized_value)
         );
         CREATE INDEX IF NOT EXISTS library_search_terms_entity
           ON library_search_terms(entity_kind, entity_id);
         CREATE INDEX IF NOT EXISTS library_search_terms_value
           ON library_search_terms(entity_kind, normalized_value, entity_id);
         CREATE TABLE IF NOT EXISTS library_search_grams (
           term_id INTEGER NOT NULL,
           gram TEXT NOT NULL,
           PRIMARY KEY(term_id, gram)
         );
         CREATE INDEX IF NOT EXISTS library_search_grams_lookup
           ON library_search_grams(gram, term_id);
         CREATE TABLE IF NOT EXISTS library_search_state (
           entity_kind TEXT NOT NULL,
           entity_id INTEGER NOT NULL,
           generation INTEGER NOT NULL,
           PRIMARY KEY(entity_kind, entity_id)
         );

         CREATE TABLE IF NOT EXISTS library_index_dirty (
           index_kind TEXT NOT NULL,
           entity_id INTEGER NOT NULL,
           generation INTEGER NOT NULL DEFAULT 1,
           queued_at INTEGER NOT NULL DEFAULT (unixepoch()),
           PRIMARY KEY(index_kind, entity_id)
         );
         CREATE TABLE IF NOT EXISTS library_index_state (
           index_kind TEXT PRIMARY KEY,
           phase TEXT NOT NULL DEFAULT 'pending',
           processed INTEGER NOT NULL DEFAULT 0,
           total INTEGER NOT NULL DEFAULT 0,
           revision INTEGER NOT NULL DEFAULT 0,
           last_error TEXT,
           settings_fingerprint TEXT
         );
         CREATE TABLE IF NOT EXISTS song_similarity_pairs (
           left_recording_id INTEGER NOT NULL,
           right_recording_id INTEGER NOT NULL,
           evidence_source_recording_id INTEGER NOT NULL,
           evidence_kind INTEGER NOT NULL,
           score REAL NOT NULL,
           lyrics_similarity REAL,
           evidence_json TEXT NOT NULL,
           updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
           PRIMARY KEY(left_recording_id, right_recording_id),
           CHECK(left_recording_id < right_recording_id)
         );
         CREATE INDEX IF NOT EXISTS song_similarity_pairs_rank
           ON song_similarity_pairs(evidence_kind DESC, score DESC, lyrics_similarity DESC);
         CREATE TABLE IF NOT EXISTS lyric_similarity_edges (
           left_asset_id INTEGER NOT NULL,
           right_asset_id INTEGER NOT NULL,
           score REAL NOT NULL,
           updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
           PRIMARY KEY(left_asset_id, right_asset_id),
           CHECK(left_asset_id < right_asset_id)
         );
         CREATE INDEX IF NOT EXISTS lyric_similarity_edges_rank
           ON lyric_similarity_edges(score DESC);
         CREATE TABLE IF NOT EXISTS lyric_similarity_groups (
           group_id TEXT PRIMARY KEY,
           score REAL NOT NULL,
           group_json TEXT NOT NULL,
           updated_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE INDEX IF NOT EXISTS lyric_similarity_groups_rank
           ON lyric_similarity_groups(score DESC, group_id);
         CREATE TABLE IF NOT EXISTS library_similarity_state (
           index_kind TEXT NOT NULL,
           entity_id INTEGER NOT NULL,
           generation INTEGER NOT NULL,
           PRIMARY KEY(index_kind, entity_id)
         );
         CREATE TABLE IF NOT EXISTS library_artist_projection (
           artist_id INTEGER PRIMARY KEY,
           canonical_name TEXT NOT NULL,
           aliases_json TEXT NOT NULL DEFAULT '[]',
           raw_names_json TEXT NOT NULL DEFAULT '[]',
           song_count INTEGER NOT NULL DEFAULT 0,
           updated_at INTEGER NOT NULL DEFAULT (unixepoch())
         );
         CREATE TABLE IF NOT EXISTS library_artist_projection_state (
           artist_id INTEGER PRIMARY KEY,
           generation INTEGER NOT NULL
         );

         INSERT OR IGNORE INTO library_index_state(index_kind, phase) VALUES
           ('search', 'pending'), ('artist', 'pending'),
           ('song_similarity', 'pending'), ('lyric_similarity', 'pending');
         INSERT OR IGNORE INTO library_index_dirty(index_kind, entity_id)
           SELECT 'song', recording_id FROM recordings;
         INSERT OR IGNORE INTO library_index_dirty(index_kind, entity_id)
           SELECT 'lyric', asset_id FROM lyric_assets;
         INSERT OR IGNORE INTO library_index_dirty(index_kind, entity_id)
           SELECT 'artist', artist_id FROM artists;",
    )?;
    install_library_dirty_triggers(&transaction)?;
    transaction.execute_batch(&format!("PRAGMA user_version = {target_version};"))?;
    transaction.commit()
}

fn install_library_dirty_triggers(connection: &Connection) -> rusqlite::Result<()> {
    // 触发器只登记失效实体，较重的规范化和相似度计算交给独立后台连接。
    connection.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS library_dirty_recordings_insert AFTER INSERT ON recordings BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_recordings_update AFTER UPDATE ON recordings BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_recordings_delete AFTER DELETE ON recordings BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', OLD.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           DELETE FROM song_similarity_pairs
             WHERE left_recording_id=OLD.recording_id OR right_recording_id=OLD.recording_id;
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_observations_insert AFTER INSERT ON track_observations BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_observations_update AFTER UPDATE ON track_observations BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', OLD.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_observations_delete AFTER DELETE ON track_observations BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', OLD.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_assets_insert AFTER INSERT ON lyric_assets BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('lyric', NEW.asset_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_assets_update AFTER UPDATE ON lyric_assets BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('lyric', NEW.asset_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_assets_delete AFTER DELETE ON lyric_assets BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('lyric', OLD.asset_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           DELETE FROM lyric_similarity_edges
             WHERE left_asset_id=OLD.asset_id OR right_asset_id=OLD.asset_id;
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_bindings_insert AFTER INSERT ON recording_lyric_bindings BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('lyric', NEW.asset_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_bindings_update AFTER UPDATE ON recording_lyric_bindings BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', OLD.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('lyric', OLD.asset_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('lyric', NEW.asset_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_bindings_delete AFTER DELETE ON recording_lyric_bindings BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', OLD.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('lyric', OLD.asset_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_credits_insert AFTER INSERT ON recording_artist_credits BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) SELECT 'artist', NEW.artist_id WHERE NEW.artist_id IS NOT NULL
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_credits_update AFTER UPDATE ON recording_artist_credits BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', OLD.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) SELECT 'artist', OLD.artist_id WHERE OLD.artist_id IS NOT NULL
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) SELECT 'artist', NEW.artist_id WHERE NEW.artist_id IS NOT NULL
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_credits_delete AFTER DELETE ON recording_artist_credits BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', OLD.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) SELECT 'artist', OLD.artist_id WHERE OLD.artist_id IS NOT NULL
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_artists_insert AFTER INSERT ON artists BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('artist', NEW.artist_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_artists_update AFTER UPDATE ON artists BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('artist', NEW.artist_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'song', recording_id FROM recording_artist_credits WHERE artist_id=NEW.artist_id
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_artists_delete AFTER DELETE ON artists BEGIN
           DELETE FROM library_artist_projection WHERE artist_id=OLD.artist_id;
           DELETE FROM library_artist_projection_state WHERE artist_id=OLD.artist_id;
           DELETE FROM library_search_grams WHERE term_id IN (
             SELECT term_id FROM library_search_terms
             WHERE entity_kind='artist' AND entity_id=OLD.artist_id
           );
           DELETE FROM library_search_terms
             WHERE entity_kind='artist' AND entity_id=OLD.artist_id;
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_aliases_insert AFTER INSERT ON artist_aliases BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('artist', NEW.artist_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'song', recording_id FROM recording_artist_credits WHERE artist_id=NEW.artist_id
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_aliases_update AFTER UPDATE ON artist_aliases BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('artist', OLD.artist_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'song', recording_id FROM recording_artist_credits WHERE artist_id=OLD.artist_id
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('artist', NEW.artist_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'song', recording_id FROM recording_artist_credits WHERE artist_id=NEW.artist_id
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_aliases_delete AFTER DELETE ON artist_aliases BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('artist', OLD.artist_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'song', recording_id FROM recording_artist_credits WHERE artist_id=OLD.artist_id
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_external_ids_insert AFTER INSERT ON recording_external_ids BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_external_ids_update AFTER UPDATE ON recording_external_ids BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', OLD.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_external_ids_delete AFTER DELETE ON recording_external_ids BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', OLD.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_asset_sources_insert AFTER INSERT ON lyric_asset_sources BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('lyric', NEW.asset_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_asset_sources_update AFTER UPDATE ON lyric_asset_sources BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('lyric', OLD.asset_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('lyric', NEW.asset_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_asset_sources_delete AFTER DELETE ON lyric_asset_sources BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('lyric', OLD.asset_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_lyric_files_insert AFTER INSERT ON lyric_files BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'lyric', asset_id FROM lyric_assets WHERE content_fingerprint=NEW.content_hash
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_lyric_files_update AFTER UPDATE ON lyric_files BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'lyric', asset_id FROM lyric_assets
             WHERE content_fingerprint=OLD.content_hash OR content_fingerprint=NEW.content_hash
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_lyric_files_delete AFTER DELETE ON lyric_files BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'lyric', asset_id FROM lyric_assets WHERE content_fingerprint=OLD.content_hash
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_overrides_insert AFTER INSERT ON platform_lyric_overrides BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'lyric', NEW.asset_id WHERE NEW.asset_id IS NOT NULL
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_overrides_update AFTER UPDATE ON platform_lyric_overrides BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', OLD.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', NEW.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'lyric', OLD.asset_id WHERE OLD.asset_id IS NOT NULL
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'lyric', NEW.asset_id WHERE NEW.asset_id IS NOT NULL
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;
         CREATE TRIGGER IF NOT EXISTS library_dirty_overrides_delete AFTER DELETE ON platform_lyric_overrides BEGIN
           INSERT INTO library_index_dirty(index_kind, entity_id) VALUES('song', OLD.recording_id)
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
           INSERT INTO library_index_dirty(index_kind, entity_id)
             SELECT 'lyric', OLD.asset_id WHERE OLD.asset_id IS NOT NULL
             ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=generation+1, queued_at=unixepoch();
         END;",
    )
}
