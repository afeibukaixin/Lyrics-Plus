use std::time::{SystemTime, UNIX_EPOCH};

impl Storage {
    pub fn new(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let app_dir = app.path().app_data_dir()?;
        let library_dir = app
            .path()
            .home_dir()?
            .join("Music")
            .join("Lyrics Plus");
        Self::open(app_dir, library_dir)
    }

    pub(crate) fn open(
        app_dir: PathBuf,
        library_dir: PathBuf,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        fs::create_dir_all(&app_dir)?;
        fs::create_dir_all(&library_dir)?;
        let legacy_lyrics_dir = app_dir.join("lyrics");
        fs::create_dir_all(&legacy_lyrics_dir)?;
        let database_path = app_dir.join("lyrics-plus.sqlite3");
        let database_exists = database_path.is_file();
        let mut connection = Connection::open(&database_path)?;
        let stored_schema_version =
            connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
        if database_exists && stored_schema_version < STORAGE_SCHEMA_VERSION {
            backup_database(&connection, &app_dir)?;
        }
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
        migrate_schema(&mut connection)?;
        let configured_library_dir = connection
            .query_row(
                "SELECT value FROM app_preferences WHERE key=?1",
                params![library::LIBRARY_DIRECTORY_PREFERENCE],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(PathBuf::from)
            .filter(|path| path.is_dir());
        // 配置中的路径就是用户选择的托管目录，不再自动追加 Downloaded。
        // 旧默认目录位于该父目录下，会随父目录递归扫描读取，不移动现有文件。
        let library_dir = configured_library_dir.unwrap_or(library_dir);
        let legacy_root_dir = legacy_lyrics_dir.clone();
        fs::create_dir_all(&library_dir)?;
        let library_dir = library_dir.canonicalize().unwrap_or(library_dir);
        migrate_legacy_files(&mut connection, &legacy_root_dir, &library_dir)?;
        let readme = library_dir.join("README.txt");
        if !readme.exists() {
            fs::write(
                &readme,
                "Lyrics Plus 歌词库\n\n这里的歌词文件归你所有，可直接查看、编辑和备份。\n应用自动下载或手动导入的普通歌词会使用“歌手 - 歌名.lrc”格式保存；同名的不同歌词版本会追加稳定哈希后缀；Lyricsfile 歌词会保留为“歌手 - 歌名.lyricsfile.yaml”。\n外部歌词文件夹默认仅建立只读索引。\n",
            )?;
        }
        let scanner = library::LibraryScanCoordinator::new(&library_dir);
        let storage = Self {
            connection: Mutex::new(connection),
            database_path,
            library_dir: RwLock::new(library_dir),
            scanner,
        };
        storage
            .initialize_library_roots(&app_dir, &legacy_root_dir)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        if let Err(error) = storage.migrate_legacy_bindings() {
            log::warn!("迁移旧歌词绑定到新结构失败：{error}");
        }
        storage
            .migrate_track_aliases()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        Ok(storage)
    }
}

fn backup_database(
    connection: &Connection,
    app_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let backup_dir = app_dir.join("database-backups");
    fs::create_dir_all(&backup_dir)?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let backup_path = backup_dir.join(format!("lyrics-plus.sqlite3.{timestamp}.backup"));
    connection.backup(rusqlite::DatabaseName::Main, &backup_path, None)?;
    Ok(())
}

const BASE_SCHEMA_VERSION: i64 = 1;
const COMPATIBILITY_SCHEMA_VERSION: i64 = 2;
const RECORDING_SCHEMA_VERSION: i64 = 3;
const TRACK_OBSERVATION_SCHEMA_VERSION: i64 = 4;
const EXTERNAL_ID_SCHEMA_VERSION: i64 = 5;
const ARTIST_CREDIT_SCHEMA_VERSION: i64 = 6;
const LYRIC_ASSET_SCHEMA_VERSION: i64 = 7;
const LIBRARY_ROOT_SCHEMA_VERSION: i64 = 8;
const SEARCH_RUN_SCHEMA_VERSION: i64 = 9;
const SEARCH_TRACE_SCHEMA_VERSION: i64 = 10;
const V2_COMPATIBILITY_SCHEMA_VERSION: i64 = 11;
const RECORDING_SPLIT_SCHEMA_VERSION: i64 = 13;
const SEARCH_TIMING_SCHEMA_VERSION: i64 = 14;
const STORAGE_SCHEMA_VERSION: i64 = SEARCH_TIMING_SCHEMA_VERSION;

/// Applies the versioned base schema. Column backfills below remain as a
/// compatibility bridge for databases created before `user_version` existed.
fn migrate_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_base_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_compatibility_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_recording_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_track_observation_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_external_id_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_artist_credit_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_lyric_asset_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_library_root_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_search_run_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_search_trace_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_v2_compatibility_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_search_timing_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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

fn migrate_recording_split_schema(connection: &mut Connection) -> rusqlite::Result<()> {
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
