use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, Manager};

use crate::lyrics::provider::{KUGOU_DISPLAY_NAME, NETEASE_DISPLAY_NAME, QQMUSIC_DISPLAY_NAME};
use crate::lyrics::{parse_lrc_with_options, LyricsDocument};

pub mod library;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveKind {
    Automatic,
    ManualSelection,
    Import,
}

pub struct SaveRequest<'a> {
    pub track_key: &'a str,
    pub title: &'a str,
    pub artist: &'a str,
    pub source: &'a str,
    pub raw: &'a str,
    pub provider_id: Option<&'a str>,
    pub provider_item_id: Option<&'a str>,
    pub kind: SaveKind,
}

impl SaveKind {
    fn is_manual(self) -> bool {
        matches!(self, Self::ManualSelection | Self::Import)
    }
}

pub struct Storage {
    connection: Mutex<Connection>,
    library_dir: RwLock<PathBuf>,
}

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
        let connection = Connection::open(app_dir.join("lyrics-plus.sqlite3"))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
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
             CREATE INDEX IF NOT EXISTS lyric_history_used_at
               ON lyric_history(used_at DESC);
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
        let readme = library_dir.join("README.txt");
        if !readme.exists() {
            fs::write(
                &readme,
                "Lyrics Plus 歌词库\n\n这里的歌词文件归你所有，可直接查看、编辑和备份。\n应用自动下载或手动导入的歌词会使用“歌手 - 歌名.lrc”格式保存。\n外部歌词文件夹默认仅建立只读索引。\n",
            )?;
        }
        library::scan_managed_library(&connection, &library_dir).map_err(std::io::Error::other)?;
        Ok(Self {
            connection: Mutex::new(connection),
            library_dir: RwLock::new(library_dir),
        })
    }

    pub fn save(&self, request: SaveRequest<'_>) -> Result<LyricsDocument, String> {
        let SaveRequest {
            track_key,
            title,
            artist,
            source,
            raw,
            provider_id,
            provider_item_id,
            kind,
        } = request;
        let existing = self.association(track_key)?;
        if kind == SaveKind::Automatic
            && existing
                .as_ref()
                .is_some_and(|association| association.manual_selected)
        {
            return self
                .load(track_key)?
                .ok_or_else(|| "受保护的歌词关联无法读取".into());
        }

        let mut document = parse_lrc_with_options(raw, source, kind.is_manual())?;
        if document.metadata.title.is_none() {
            document.metadata.title = Some(title.into());
        }
        if document.metadata.artist.is_none() {
            document.metadata.artist = Some(artist.into());
        }
        let library_dir = self
            .library_dir
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let path = existing
            .as_ref()
            .map(|association| association.path.clone())
            .filter(|path| path.starts_with(&library_dir))
            .unwrap_or_else(|| available_path(&library_dir, title, artist, raw));
        fs::write(&path, raw).map_err(|error| format!("保存歌词文件失败：{error}"))?;
        let content_hash = content_hash(raw);
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute(
                "INSERT INTO lyric_associations
                   (track_key, title, artist, source, content_path, offset_ms, original_format,
                    manual_selected, provider_id, provider_item_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'lrc', ?7, ?8, ?9, unixepoch())
                 ON CONFLICT(track_key) DO UPDATE SET
                   title=excluded.title, artist=excluded.artist, source=excluded.source,
                   content_path=excluded.content_path, original_format=excluded.original_format,
                   manual_selected=excluded.manual_selected, provider_id=excluded.provider_id,
                   provider_item_id=excluded.provider_item_id, updated_at=unixepoch()",
                params![
                    track_key,
                    title,
                    artist,
                    source,
                    path.to_string_lossy(),
                    document.offset_ms,
                    kind.is_manual(),
                    provider_id,
                    provider_item_id
                ],
            )
            .map_err(|error| format!("保存歌词关联失败：{error}"))?;
        upsert_file_index(
            &connection,
            &path,
            title,
            artist,
            source,
            kind.is_manual(),
            &content_hash,
        )?;
        connection
            .execute(
                "INSERT INTO lyric_history (track_key, title, artist, source, used_at)
                 VALUES (?1, ?2, ?3, ?4, unixepoch())",
                params![track_key, title, artist, source],
            )
            .map_err(|error| format!("保存歌词使用记录失败：{error}"))?;
        connection
            .execute(
                "DELETE FROM lyric_history WHERE id NOT IN (
                   SELECT id FROM lyric_history ORDER BY used_at DESC, id DESC LIMIT 100
                 )",
                [],
            )
            .map_err(|error| format!("整理歌词使用记录失败：{error}"))?;
        drop(connection);
        self.load(track_key)?
            .ok_or_else(|| "歌词保存后无法读取".into())
    }

    pub fn load(&self, track_key: &str) -> Result<Option<LyricsDocument>, String> {
        let Some(association) = self.association(track_key)? else {
            return Ok(None);
        };
        let raw = fs::read_to_string(&association.path)
            .map_err(|error| format!("读取歌词文件失败：{error}"))?;
        let mut document =
            parse_lrc_with_options(&raw, &association.source, association.manual_selected)?;
        document.metadata.title = Some(association.title);
        document.metadata.artist = Some(association.artist);
        document.metadata.original_format = association.original_format;
        document.offset_ms = association.offset_ms;
        Ok(Some(document))
    }

    fn association(&self, track_key: &str) -> Result<Option<Association>, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .query_row(
                "SELECT title, artist, source, content_path, offset_ms, original_format, manual_selected
                 FROM lyric_associations WHERE track_key=?1",
                params![track_key],
                |row| {
                    Ok(Association {
                        title: row.get(0)?,
                        artist: row.get(1)?,
                        source: row.get(2)?,
                        path: PathBuf::from(row.get::<_, String>(3)?),
                        offset_ms: row.get(4)?,
                        original_format: row.get(5)?,
                        manual_selected: row.get::<_, i64>(6)? != 0,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("读取歌词关联失败：{error}"))
    }

    pub fn set_offset(&self, track_key: &str, offset_ms: i64) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let changed = connection
            .execute(
                "UPDATE lyric_associations SET offset_ms=?2, updated_at=unixepoch() WHERE track_key=?1",
                params![track_key, offset_ms],
            )
            .map_err(|error| format!("保存歌词偏移失败：{error}"))?;
        if changed == 0 {
            Err("当前歌曲尚未关联歌词".into())
        } else {
            Ok(())
        }
    }

    pub fn remove(&self, track_key: &str) -> Result<(), String> {
        let association = self.association(track_key)?;
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute(
                "DELETE FROM lyric_associations WHERE track_key=?1",
                params![track_key],
            )
            .map_err(|error| format!("解除歌词关联失败：{error}"))?;
        let path = association.map(|association| association.path);
        let references = path.as_ref().map_or(0, |path| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM lyric_associations WHERE content_path=?1",
                    params![path.to_string_lossy()],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(1)
        });
        if let Some(path) = path.filter(|_| references == 0) {
            connection
                .execute(
                    "DELETE FROM lyric_files WHERE content_path=?1",
                    params![path.to_string_lossy()],
                )
                .map_err(|error| format!("删除歌词索引失败：{error}"))?;
            drop(connection);
            if path.exists() {
                fs::remove_file(path).map_err(|error| format!("删除歌词文件失败：{error}"))?;
            }
        }
        Ok(())
    }

    pub fn get_preference(&self, key: &str) -> Result<Option<String>, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .query_row(
                "SELECT value FROM app_preferences WHERE key=?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("读取应用偏好失败：{error}"))
    }

    pub fn set_preference(&self, key: &str, value: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute(
                "INSERT INTO app_preferences (key, value, updated_at)
                 VALUES (?1, ?2, unixepoch())
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=unixepoch()",
                params![key, value],
            )
            .map_err(|error| format!("保存应用偏好失败：{error}"))?;
        Ok(())
    }

    pub fn remove_preference(&self, key: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute("DELETE FROM app_preferences WHERE key=?1", params![key])
            .map_err(|error| format!("重置应用偏好失败：{error}"))?;
        Ok(())
    }

    pub fn remove_preferences_with_prefix(&self, prefix: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute(
                "DELETE FROM app_preferences WHERE substr(key, 1, length(?1))=?1",
                params![prefix],
            )
            .map_err(|error| format!("重置应用偏好失败：{error}"))?;
        Ok(())
    }
}

fn migrate_provider_source_names(connection: &Connection) -> rusqlite::Result<()> {
    const SOURCE_TABLES: [&str; 3] = ["lyric_associations", "lyric_files", "lyric_history"];
    let aliases = [
        ("网易云音乐", NETEASE_DISPLAY_NAME),
        ("QQ 音乐", QQMUSIC_DISPLAY_NAME),
        ("QQ音乐", QQMUSIC_DISPLAY_NAME),
        ("酷狗音乐", KUGOU_DISPLAY_NAME),
    ];

    for table in SOURCE_TABLES {
        let statement = format!("UPDATE {table} SET source=?1 WHERE source=?2");
        for (legacy_name, display_name) in aliases {
            connection.execute(&statement, params![display_name, legacy_name])?;
        }
    }
    Ok(())
}

struct Association {
    title: String,
    artist: String,
    source: String,
    path: PathBuf,
    offset_ms: i64,
    original_format: String,
    manual_selected: bool,
}

pub(super) fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|existing| existing == column) {
        connection.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
        return Ok(true);
    }
    Ok(false)
}

pub(super) fn safe_component(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|character| {
            if character.is_control() || matches!(character, '/' | '\\' | ':') {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned
        .trim_matches(['.', ' '])
        .chars()
        .take(96)
        .collect::<String>();
    if cleaned.is_empty() {
        "未知".into()
    } else {
        cleaned
    }
}

pub(super) fn available_path(library_dir: &Path, title: &str, artist: &str, raw: &str) -> PathBuf {
    let stem = format!("{} - {}", safe_component(artist), safe_component(title));
    let initial = library_dir.join(format!("{stem}.lrc"));
    if !initial.exists() || fs::read_to_string(&initial).ok().as_deref() == Some(raw) {
        return initial;
    }
    for suffix in 2..10_000 {
        let candidate = library_dir.join(format!("{stem} ({suffix}).lrc"));
        if !candidate.exists() || fs::read_to_string(&candidate).ok().as_deref() == Some(raw) {
            return candidate;
        }
    }
    library_dir.join(format!("{stem}-{}.lrc", content_hash(raw)))
}

pub(super) fn content_hash(raw: &str) -> String {
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(super) fn upsert_file_index(
    connection: &Connection,
    path: &Path,
    title: &str,
    artist: &str,
    source: &str,
    manual_selected: bool,
    hash: &str,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO lyric_files
               (content_path, title, artist, source, original_format, manual_selected, content_hash, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'lrc', ?5, ?6, unixepoch())
             ON CONFLICT(content_path) DO UPDATE SET
               title=excluded.title, artist=excluded.artist, source=excluded.source,
               original_format=excluded.original_format, manual_selected=excluded.manual_selected,
               content_hash=excluded.content_hash, updated_at=unixepoch()",
            params![
                path.to_string_lossy(),
                title,
                artist,
                source,
                manual_selected,
                hash
            ],
        )
        .map_err(|error| format!("保存歌词文件索引失败：{error}"))?;
    Ok(())
}

fn migrate_legacy_files(
    connection: &Connection,
    legacy_dir: &Path,
    library_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut statement = connection.prepare(
        "SELECT track_key, title, artist, source, content_path, manual_selected
         FROM lyric_associations",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                PathBuf::from(row.get::<_, String>(4)?),
                row.get::<_, i64>(5)? != 0,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    for (track_key, title, artist, source, old_path, manual_selected) in rows {
        if !old_path.exists() {
            continue;
        }
        let raw = fs::read_to_string(&old_path)?;
        let path = if old_path.starts_with(library_dir) {
            old_path.clone()
        } else if old_path.starts_with(legacy_dir) {
            let destination = available_path(library_dir, &title, &artist, &raw);
            if destination != old_path && !destination.exists() {
                fs::copy(&old_path, &destination)?;
            }
            connection.execute(
                "UPDATE lyric_associations SET content_path=?2, updated_at=unixepoch()
                 WHERE track_key=?1",
                params![track_key, destination.to_string_lossy()],
            )?;
            if destination.exists() && old_path != destination {
                let _ = fs::remove_file(&old_path);
            }
            destination
        } else {
            old_path.clone()
        };
        upsert_file_index(
            connection,
            &path,
            &title,
            &artist,
            &source,
            manual_selected,
            &content_hash(&raw),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    pub(super) fn test_dirs(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "lyrics-plus-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        (root.join("app"), root.join("Music").join("Lyrics Plus"))
    }

    #[test]
    fn preferences_and_lyrics_survive_reopen() {
        let (app_dir, library_dir) = test_dirs("storage");
        {
            let storage =
                Storage::open(app_dir.clone(), library_dir.clone()).expect("open storage");
            storage
                .set_preference("player.selection", "spotify")
                .expect("save preference");
            storage
                .save(SaveRequest {
                    track_key: "spotify:test",
                    title: "测试歌曲",
                    artist: "测试歌手",
                    source: "test",
                    raw: "[00:01.00]第一行\n[00:02.00]第二行",
                    provider_id: Some("test"),
                    provider_item_id: Some("version-1"),
                    kind: SaveKind::ManualSelection,
                })
                .expect("save lyrics");
            storage
                .set_offset("spotify:test", 300)
                .expect("save offset");
        }

        let reopened = Storage::open(app_dir.clone(), library_dir.clone()).expect("reopen storage");
        assert_eq!(
            reopened
                .get_preference("player.selection")
                .expect("load preference")
                .as_deref(),
            Some("spotify")
        );
        let lyrics = reopened
            .load("spotify:test")
            .expect("load lyrics")
            .expect("lyrics exist");
        assert_eq!(lyrics.tracks.original.lines.len(), 2);
        assert_eq!(lyrics.offset_ms, 300);
        assert!(lyrics.metadata.manual_selected);
        assert!(library_dir.join("测试歌手 - 测试歌曲.lrc").exists());
        let selected_version: (String, String) = reopened
            .connection
            .lock()
            .expect("lock database")
            .query_row(
                "SELECT provider_id, provider_item_id FROM lyric_associations WHERE track_key='spotify:test'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("load provider version");
        assert_eq!(selected_version, ("test".into(), "version-1".into()));
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn migrates_legacy_provider_names_across_stored_metadata() {
        let (app_dir, library_dir) = test_dirs("provider-names");
        let legacy_names = [
            ("netease", "网易云音乐", NETEASE_DISPLAY_NAME),
            ("qqmusic-spaced", "QQ 音乐", QQMUSIC_DISPLAY_NAME),
            ("qqmusic", "QQ音乐", QQMUSIC_DISPLAY_NAME),
            ("kugou", "酷狗音乐", KUGOU_DISPLAY_NAME),
        ];

        {
            let storage = Storage::open(app_dir.clone(), library_dir.clone()).unwrap();
            for (track_key, legacy_name, _) in legacy_names {
                storage
                    .save(SaveRequest {
                        track_key,
                        title: track_key,
                        artist: "Artist",
                        source: legacy_name,
                        raw: "[00:01]Legacy source",
                        provider_id: Some(track_key),
                        provider_item_id: Some("1"),
                        kind: SaveKind::ManualSelection,
                    })
                    .unwrap();
            }
        }

        let reopened = Storage::open(app_dir.clone(), library_dir.clone()).unwrap();
        for (track_key, _, expected_name) in legacy_names {
            let document = reopened.load(track_key).unwrap().unwrap();
            assert_eq!(document.metadata.source, expected_name);
        }

        let library_sources = reopened
            .library_overview()
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| entry.source)
            .collect::<HashSet<_>>();
        assert!(library_sources.contains(NETEASE_DISPLAY_NAME));
        assert!(library_sources.contains(QQMUSIC_DISPLAY_NAME));
        assert!(library_sources.contains(KUGOU_DISPLAY_NAME));

        let connection = reopened.connection.lock().unwrap();
        let legacy_history_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM lyric_history
                 WHERE source IN ('网易云音乐', 'QQ 音乐', 'QQ音乐', '酷狗音乐')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy_history_count, 0);
        drop(connection);

        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn automatic_search_does_not_overwrite_manual_lyrics() {
        let (app_dir, library_dir) = test_dirs("priority");
        let storage = Storage::open(app_dir.clone(), library_dir).expect("open storage");
        storage
            .save(SaveRequest {
                track_key: "track",
                title: "Song",
                artist: "Artist",
                source: "本地导入",
                raw: "[00:01]Manual",
                provider_id: None,
                provider_item_id: None,
                kind: SaveKind::Import,
            })
            .unwrap();
        let result = storage
            .save(SaveRequest {
                track_key: "track",
                title: "Song",
                artist: "Artist",
                source: "LRCLIB",
                raw: "[00:01]Network",
                provider_id: Some("lrclib"),
                provider_item_id: Some("1"),
                kind: SaveKind::Automatic,
            })
            .unwrap();
        assert_eq!(result.tracks.original.lines[0].text, "Manual");
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn resetting_overlay_preferences_clears_overlay_only() {
        let (app_dir, library_dir) = test_dirs("reset-preferences");
        let storage = Storage::open(app_dir.clone(), library_dir).expect("open storage");
        storage
            .set_preference("overlay.style.display-a", "custom")
            .unwrap();
        storage
            .set_preference("overlay.position.display-a", "bounds")
            .unwrap();
        storage
            .set_preference("overlay.last_monitor", "display-a")
            .unwrap();
        storage.set_preference("overlay.visible", "true").unwrap();
        storage.set_preference("overlay.locked", "true").unwrap();
        storage
            .set_preference("overlay.passthrough", "true")
            .unwrap();
        storage
            .set_preference("player.selection", "spotify")
            .unwrap();
        storage
            .save(SaveRequest {
                track_key: "track",
                title: "Song",
                artist: "Artist",
                source: "本地导入",
                raw: "[00:01]Manual",
                provider_id: None,
                provider_item_id: None,
                kind: SaveKind::Import,
            })
            .unwrap();

        storage
            .remove_preferences_with_prefix("overlay.style.")
            .unwrap();
        storage
            .remove_preferences_with_prefix("overlay.position.")
            .unwrap();
        storage.remove_preference("overlay.last_monitor").unwrap();
        storage.remove_preference("overlay.visible").unwrap();
        storage.remove_preference("overlay.locked").unwrap();
        storage.remove_preference("overlay.passthrough").unwrap();

        assert_eq!(
            storage.get_preference("overlay.style.display-a").unwrap(),
            None
        );
        for key in [
            "overlay.position.display-a",
            "overlay.last_monitor",
            "overlay.visible",
            "overlay.locked",
            "overlay.passthrough",
        ] {
            assert_eq!(storage.get_preference(key).unwrap(), None);
        }
        assert_eq!(
            storage
                .get_preference("player.selection")
                .unwrap()
                .as_deref(),
            Some("spotify")
        );
        assert_eq!(
            storage
                .load("track")
                .unwrap()
                .unwrap()
                .tracks
                .original
                .lines[0]
                .text,
            "Manual"
        );
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn migrates_hashed_cache_to_readable_library_file() {
        let (app_dir, library_dir) = test_dirs("migration");
        fs::create_dir_all(app_dir.join("lyrics")).unwrap();
        let legacy = app_dir.join("lyrics/0123456789abcdef.lrc");
        fs::write(&legacy, "[00:01]Legacy").unwrap();
        let connection = Connection::open(app_dir.join("lyrics-plus.sqlite3")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE lyric_associations (
               track_key TEXT PRIMARY KEY, title TEXT NOT NULL, artist TEXT NOT NULL,
               source TEXT NOT NULL, content_path TEXT NOT NULL,
               offset_ms INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0
             );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO lyric_associations VALUES (?1, ?2, ?3, ?4, ?5, 240, 0)",
                params![
                    "track",
                    "Legacy Song",
                    "Legacy Artist",
                    "LRCLIB",
                    legacy.to_string_lossy()
                ],
            )
            .unwrap();
        drop(connection);

        let storage = Storage::open(app_dir.clone(), library_dir.clone()).unwrap();
        let lyrics = storage.load("track").unwrap().unwrap();
        assert_eq!(lyrics.offset_ms, 240);
        assert!(library_dir.join("Legacy Artist - Legacy Song.lrc").exists());
        assert!(!legacy.exists());
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }
}
