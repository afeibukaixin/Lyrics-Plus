use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, Manager};

mod migrations;

use super::library;
use super::{migrate_legacy_files, Storage};
use migrations::migrate_schema;

impl Storage {
    pub fn new(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let app_dir = app.path().app_data_dir()?;
        let library_dir = app.path().home_dir()?.join("Music").join("Lyrics Plus");
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
        if database_exists && stored_schema_version < migrations::STORAGE_SCHEMA_VERSION {
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
