impl Storage {
    pub fn new(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let app_dir = app.path().app_data_dir()?;
        let library_dir = app.path().audio_dir()?.join("Lyrics Plus");
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
        let connection = Connection::open(&database_path)?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=5000;
             CREATE TABLE IF NOT EXISTS lyric_associations (
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
               updated_at INTEGER NOT NULL DEFAULT (unixepoch())
             );
             CREATE INDEX IF NOT EXISTS lyric_history_used_at
               ON lyric_history(used_at DESC);
             CREATE INDEX IF NOT EXISTS lyric_track_aliases_canonical
               ON lyric_track_aliases(canonical_track_key);
             CREATE INDEX IF NOT EXISTS lyric_track_aliases_identity
               ON lyric_track_aliases(title_norm, artist_norm);
             CREATE INDEX IF NOT EXISTS lyric_files_title_artist
               ON lyric_files(title, artist);",
        )?;
        ensure_column(
            &connection,
            "lyric_associations",
            "original_format",
            "TEXT NOT NULL DEFAULT 'lrc'",
        )?;
        library::initialize_schema(&connection)?;
        let library_dir = connection
            .query_row(
                "SELECT value FROM app_preferences WHERE key=?1",
                params![library::LIBRARY_DIRECTORY_PREFERENCE],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
            .unwrap_or(library_dir);
        fs::create_dir_all(&library_dir)?;
        let library_dir = library_dir.canonicalize().unwrap_or(library_dir);
        let added_manual_selected = ensure_column(
            &connection,
            "lyric_associations",
            "manual_selected",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(&connection, "lyric_associations", "provider_id", "TEXT")?;
        ensure_column(
            &connection,
            "lyric_associations",
            "provider_item_id",
            "TEXT",
        )?;
        if added_manual_selected {
            connection.execute("UPDATE lyric_associations SET manual_selected=1", [])?;
        } else {
            connection.execute(
                "UPDATE lyric_associations SET manual_selected=1
                 WHERE source IN ('本地导入', '手动导入')",
                [],
            )?;
        }
        migrate_provider_source_names(&connection)?;
        migrate_legacy_files(&connection, &legacy_lyrics_dir, &library_dir)?;
        library::normalize_collision_metadata(&connection)?;
        let readme = library_dir.join("README.txt");
        if !readme.exists() {
            fs::write(
                &readme,
                "Lyrics Plus 歌词库\n\n这里的歌词文件归你所有，可直接查看、编辑和备份。\n应用自动下载或手动导入的歌词会使用“歌手 - 歌名.lrc”格式保存。\n外部歌词文件夹默认仅建立只读索引。\n",
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
            .migrate_track_aliases()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        storage.cleanup_orphan_app_owned_files();
        Ok(storage)
    }
}
