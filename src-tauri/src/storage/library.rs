use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rusqlite::{params, Connection};
use serde::Serialize;

use super::{content_hash, ensure_column, Storage};
use crate::lyrics::{parse_lrc_with_options, LyricsDocument};

const MAX_LYRIC_FILE_SIZE: u64 = 5 * 1024 * 1024;
pub(super) const LIBRARY_DIRECTORY_PREFERENCE: &str = "lyrics.library_directory";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    pub path: String,
    pub file_name: String,
    pub title: String,
    pub artist: String,
    pub source: String,
    pub format: String,
    pub duration_ms: Option<u64>,
    pub file_size: u64,
    pub modified_at_ms: Option<u64>,
    pub duplicate_count: u64,
    pub association_count: u64,
    pub has_translation: bool,
    pub has_word_timing: bool,
    pub has_romanization: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryOverview {
    pub library_dir: String,
    pub entries: Vec<LibraryEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPreview {
    pub entry: LibraryEntry,
    pub raw: String,
    pub document: Option<LyricsDocument>,
}

pub(super) fn initialize_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute(
        "CREATE INDEX IF NOT EXISTS lyric_files_content_hash ON lyric_files(content_hash)",
        [],
    )?;
    ensure_column(connection, "lyric_files", "folder_id", "INTEGER")?;
    ensure_column(
        connection,
        "lyric_files",
        "managed",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    ensure_column(
        connection,
        "lyric_files",
        "file_size",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(connection, "lyric_files", "modified_at_ms", "INTEGER")?;
    ensure_column(connection, "lyric_files", "duration_ms", "INTEGER")?;
    ensure_column(
        connection,
        "lyric_files",
        "file_fingerprint",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "lyric_files",
        "has_translation",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        connection,
        "lyric_files",
        "has_word_timing",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        connection,
        "lyric_files",
        "has_romanization",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    Ok(())
}

pub(super) fn scan_managed_library(
    connection: &Connection,
    library_dir: &Path,
) -> Result<(), String> {
    scan_folder(connection, library_dir)
}

impl Storage {
    pub fn library_overview(&self) -> Result<LibraryOverview, String> {
        let library_dir = self.library_directory();
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut entries = list_entries(&connection, &library_dir)?;
        entries.sort_by(|left, right| {
            right
                .modified_at_ms
                .cmp(&left.modified_at_ms)
                .then_with(|| left.title.cmp(&right.title))
        });
        Ok(LibraryOverview {
            library_dir: library_dir.to_string_lossy().into_owned(),
            entries,
        })
    }

    pub fn set_library_directory(&self, path: &str) -> Result<LibraryOverview, String> {
        let path = canonical_directory(path)?;
        let metadata =
            fs::metadata(&path).map_err(|error| format!("无法读取歌词文件夹信息：{error}"))?;
        if metadata.permissions().readonly() {
            return Err("所选歌词文件夹不可写".into());
        }

        {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            scan_managed_library(&connection, &path)?;
            connection
                .execute(
                    "INSERT INTO app_preferences (key, value, updated_at)
                     VALUES (?1, ?2, unixepoch())
                     ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=unixepoch()",
                    params![LIBRARY_DIRECTORY_PREFERENCE, path.to_string_lossy()],
                )
                .map_err(|error| format!("保存歌词目录失败：{error}"))?;
        }

        *self
            .library_dir
            .write()
            .unwrap_or_else(|error| error.into_inner()) = path;
        self.library_overview()
    }

    pub fn rescan_library(&self) -> Result<LibraryOverview, String> {
        let library_dir = self.library_directory();
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        scan_managed_library(&connection, &library_dir)?;
        drop(connection);
        self.library_overview()
    }

    pub fn preview_library_entry(&self, path: &str) -> Result<LibraryPreview, String> {
        let entry = self
            .library_overview()?
            .entries
            .into_iter()
            .find(|entry| same_path(&entry.path, path))
            .ok_or_else(|| "歌词文件未被当前目录索引".to_string())?;
        let raw = read_lyric(Path::new(&entry.path))?;
        let document = parse_lrc_with_options(&raw, &entry.source, true).ok();
        Ok(LibraryPreview {
            entry,
            raw,
            document,
        })
    }

    fn library_directory(&self) -> PathBuf {
        self.library_dir
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

fn list_entries(connection: &Connection, library_dir: &Path) -> Result<Vec<LibraryEntry>, String> {
    let mut statement = connection
        .prepare(
            "SELECT content_path, title, artist, source, original_format,
                    file_size, modified_at_ms, duration_ms, content_hash,
                    has_translation, has_word_timing, has_romanization,
                    (SELECT COUNT(*) FROM lyric_associations associations
                     WHERE associations.content_path=lyric_files.content_path)
             FROM lyric_files WHERE managed=1 AND folder_id IS NULL",
        )
        .map_err(|error| format!("读取歌词库失败：{error}"))?;
    let indexed = statement
        .query_map([], |row| {
            let path: String = row.get(0)?;
            let entry = LibraryEntry {
                file_name: Path::new(&path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_string(),
                path,
                title: row.get(1)?,
                artist: row.get(2)?,
                source: row.get(3)?,
                format: row.get(4)?,
                file_size: row.get::<_, i64>(5)?.max(0) as u64,
                modified_at_ms: row
                    .get::<_, Option<i64>>(6)?
                    .map(|value| value.max(0) as u64),
                duration_ms: row
                    .get::<_, Option<i64>>(7)?
                    .map(|value| value.max(0) as u64),
                duplicate_count: 1,
                association_count: row.get::<_, i64>(12)?.max(0) as u64,
                has_translation: row.get::<_, i64>(9)? != 0,
                has_word_timing: row.get::<_, i64>(10)? != 0,
                has_romanization: row.get::<_, i64>(11)? != 0,
            };
            Ok((entry, row.get::<_, String>(8)?))
        })
        .map_err(|error| format!("查询歌词库失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析歌词库失败：{error}"))?;

    let mut counts = HashMap::<String, u64>::new();
    for (entry, hash) in &indexed {
        if Path::new(&entry.path).starts_with(library_dir) {
            *counts.entry(hash.clone()).or_insert(0) += 1;
        }
    }
    Ok(indexed
        .into_iter()
        .filter_map(|(mut entry, hash)| {
            if !Path::new(&entry.path).starts_with(library_dir) {
                return None;
            }
            entry.duplicate_count = counts.get(hash.as_str()).copied().unwrap_or(1);
            Some(entry)
        })
        .collect())
}

fn scan_folder(connection: &Connection, root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err("歌词文件夹不存在或不可访问".into());
    }
    let mut files = Vec::new();
    collect_lyric_files(root, &mut files)?;
    let mut seen = HashSet::new();
    for path in files {
        seen.insert(path.to_string_lossy().into_owned());
        index_file(connection, &path)?;
    }

    let mut statement = connection
        .prepare("SELECT content_path FROM lyric_files WHERE managed=1 AND folder_id IS NULL")
        .map_err(|error| format!("读取已有索引失败：{error}"))?;
    let existing = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("查询已有索引失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析已有索引失败：{error}"))?;
    drop(statement);

    for path in existing
        .into_iter()
        .filter(|path| Path::new(path).starts_with(root) && !seen.contains(path))
    {
        connection
            .execute(
                "DELETE FROM lyric_associations WHERE content_path=?1",
                params![path],
            )
            .map_err(|error| format!("清理失效关联失败：{error}"))?;
        connection
            .execute(
                "DELETE FROM lyric_files WHERE content_path=?1",
                params![path],
            )
            .map_err(|error| format!("清理失效索引失败：{error}"))?;
    }
    Ok(())
}

fn collect_lyric_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|error| format!("无法读取歌词文件夹：{error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("读取文件夹项目失败：{error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("读取文件类型失败：{error}"))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_lyric_files(&entry.path(), output)?;
        } else if file_type.is_file() && has_supported_extension(&entry.path()) {
            output.push(entry.path());
            if output.len() >= 50_000 {
                return Err("单个歌词文件夹最多索引 50000 个文件".into());
            }
        }
    }
    Ok(())
}

fn index_file(connection: &Connection, path: &Path) -> Result<(), String> {
    let file_metadata = fs::metadata(path).map_err(|error| format!("读取歌词文件失败：{error}"))?;
    if file_metadata.len() > MAX_LYRIC_FILE_SIZE {
        return Err("歌词文件超过 5 MB，已跳过".into());
    }
    let raw = read_lyric(path)?;
    let metadata = lyric_metadata(path, &raw, "本地文件");
    let hash = content_hash(&raw);
    let fingerprint = content_hash(&format!(
        "{}|{}|{}|{}",
        metadata.title,
        metadata.artist,
        metadata.duration_ms.unwrap_or(0),
        file_metadata.len()
    ));
    connection
        .execute(
            "INSERT INTO lyric_files
               (content_path, title, artist, source, original_format, manual_selected,
                content_hash, folder_id, managed, file_size, modified_at_ms, duration_ms,
                file_fingerprint, has_translation, has_word_timing, has_romanization, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, NULL, 1, ?7, ?8, ?9, ?10, ?11, ?12, ?13, unixepoch())
             ON CONFLICT(content_path) DO UPDATE SET
               title=excluded.title, artist=excluded.artist,
               source=CASE WHEN lyric_files.managed=1 AND lyric_files.source!='本地文件'
                           THEN lyric_files.source ELSE excluded.source END,
               original_format=excluded.original_format,
               content_hash=excluded.content_hash, folder_id=NULL, managed=1,
               file_size=excluded.file_size, modified_at_ms=excluded.modified_at_ms,
               duration_ms=excluded.duration_ms, file_fingerprint=excluded.file_fingerprint,
               has_translation=excluded.has_translation,
               has_word_timing=excluded.has_word_timing,
               has_romanization=excluded.has_romanization, updated_at=unixepoch()",
            params![
                path.to_string_lossy(),
                metadata.title,
                metadata.artist,
                metadata.source,
                metadata.format,
                hash,
                file_metadata.len() as i64,
                modified_ms(&file_metadata).map(|value| value as i64),
                metadata.duration_ms.map(|value| value as i64),
                fingerprint,
                metadata.has_translation,
                metadata.has_word_timing,
                metadata.has_romanization,
            ],
        )
        .map_err(|error| format!("更新歌词索引失败：{error}"))?;
    Ok(())
}

struct ParsedMetadata {
    title: String,
    artist: String,
    source: String,
    format: String,
    duration_ms: Option<u64>,
    has_translation: bool,
    has_word_timing: bool,
    has_romanization: bool,
}

fn lyric_metadata(path: &Path, raw: &str, source: &str) -> ParsedMetadata {
    let format = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("lrc")
        .to_ascii_lowercase();
    let document = parse_lrc_with_options(raw, source, false).ok();
    let (filename_artist, filename_title) = path
        .file_stem()
        .and_then(|value| value.to_str())
        .and_then(|value| value.split_once(" - "))
        .map(|(artist, title)| (Some(artist.to_string()), Some(title.to_string())))
        .unwrap_or((None, None));
    let title = document
        .as_ref()
        .and_then(|value| value.metadata.title.clone())
        .or_else(|| lrc_tag(raw, "ti"))
        .or(filename_title)
        .unwrap_or_else(|| "未知歌曲".into());
    let artist = document
        .as_ref()
        .and_then(|value| value.metadata.artist.clone())
        .or_else(|| lrc_tag(raw, "ar"))
        .or(filename_artist)
        .unwrap_or_else(|| "未知歌手".into());
    let duration_ms = document.as_ref().and_then(|value| {
        value
            .tracks
            .original
            .lines
            .last()
            .map(|line| line.end_ms.unwrap_or(line.start_ms))
    });
    let has_translation = document
        .as_ref()
        .is_some_and(|value| value.tracks.translation.is_some());
    let has_word_timing = document.as_ref().is_some_and(|value| {
        value
            .tracks
            .original
            .lines
            .iter()
            .any(|line| line.words.as_ref().is_some_and(|words| !words.is_empty()))
    });
    let has_romanization = document
        .as_ref()
        .is_some_and(|value| value.tracks.romanization.is_some());
    ParsedMetadata {
        title,
        artist,
        source: source.into(),
        format,
        duration_ms,
        has_translation,
        has_word_timing,
        has_romanization,
    }
}

fn lrc_tag(raw: &str, key: &str) -> Option<String> {
    let prefix = format!("[{key}:");
    raw.lines().find_map(|line| {
        line.strip_prefix(&prefix)
            .and_then(|value| value.strip_suffix(']'))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn read_lyric(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("读取歌词文件失败：{error}"))?;
    if bytes.len() as u64 > MAX_LYRIC_FILE_SIZE {
        return Err("歌词文件超过 5 MB".into());
    }
    Ok(String::from_utf8_lossy(&bytes)
        .trim_start_matches('\u{feff}')
        .to_string())
}

fn has_supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "lrc" | "yrc" | "qrc" | "ttml" | "lrcx"
            )
        })
}

fn canonical_directory(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        return Err("请选择歌词文件夹".into());
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("无法访问歌词文件夹：{error}"))?;
    if !canonical.is_dir() {
        return Err("所选路径不是文件夹".into());
    }
    Ok(canonical)
}

fn same_path(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    let left = Path::new(left).canonicalize();
    let right = Path::new(right).canonicalize();
    matches!((left, right), (Ok(left), Ok(right)) if left == right)
}

fn modified_ms(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::tests::test_dirs;
    use crate::storage::{SaveKind, SaveRequest};

    #[test]
    fn scan_detects_duplicates_and_parses_readable_names() {
        let (app_dir, library_dir) = test_dirs("duplicates");
        fs::create_dir_all(&library_dir).unwrap();
        let raw = "[00:01]Line";
        fs::write(library_dir.join("Artist - Song.lrc"), raw).unwrap();
        fs::write(library_dir.join("Artist - Song copy.lrc"), raw).unwrap();
        let storage = Storage::open(app_dir.clone(), library_dir).unwrap();
        let overview = storage.library_overview().unwrap();
        assert_eq!(overview.entries.len(), 2);
        assert!(overview
            .entries
            .iter()
            .all(|entry| entry.duplicate_count == 2));
        assert!(overview.entries.iter().any(|entry| entry.title == "Song"));
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn changing_directory_preserves_old_files_and_associations() {
        let (app_dir, old_dir) = test_dirs("switch-library");
        let new_dir = app_dir.parent().unwrap().join("New Library");
        fs::create_dir_all(&new_dir).unwrap();
        let storage = Storage::open(app_dir.clone(), old_dir.clone()).unwrap();
        storage
            .save(SaveRequest {
                track_key: "spotify:old",
                title: "Old Song",
                artist: "Artist",
                source: "Test",
                raw: "[00:01]Old line",
                provider_id: None,
                provider_item_id: None,
                kind: SaveKind::Automatic,
            })
            .unwrap();
        let old_path = storage.library_overview().unwrap().entries[0].path.clone();

        let overview = storage
            .set_library_directory(&new_dir.to_string_lossy())
            .unwrap();
        assert!(overview.entries.is_empty());
        assert!(Path::new(&old_path).exists());
        assert!(storage.load("spotify:old").unwrap().is_some());
        storage.rescan_library().unwrap();
        assert!(storage.load("spotify:old").unwrap().is_some());

        storage
            .save(SaveRequest {
                track_key: "spotify:new",
                title: "New Song",
                artist: "Artist",
                source: "Test",
                raw: "[00:01]New line",
                provider_id: None,
                provider_item_id: None,
                kind: SaveKind::Automatic,
            })
            .unwrap();
        let canonical_new_dir = new_dir.canonicalize().unwrap();
        assert!(storage.library_overview().unwrap().entries[0]
            .path
            .starts_with(&canonical_new_dir.to_string_lossy().to_string()));
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn selected_directory_persists_and_invalid_path_is_rejected() {
        let (app_dir, default_dir) = test_dirs("persist-library");
        let new_dir = app_dir.parent().unwrap().join("Selected Library");
        fs::create_dir_all(&new_dir).unwrap();
        let storage = Storage::open(app_dir.clone(), default_dir.clone()).unwrap();
        storage
            .set_library_directory(&new_dir.to_string_lossy())
            .unwrap();
        assert!(storage
            .set_library_directory(&app_dir.join("missing").to_string_lossy())
            .is_err());
        drop(storage);

        let reopened = Storage::open(app_dir.clone(), default_dir).unwrap();
        assert_eq!(
            reopened.library_overview().unwrap().library_dir,
            new_dir.canonicalize().unwrap().to_string_lossy()
        );
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }
}
