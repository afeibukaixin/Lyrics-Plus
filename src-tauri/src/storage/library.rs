use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;

use super::{content_hash, ensure_column, Storage};
use crate::lyrics::parse_lrc_with_options;

const MAX_LYRIC_FILE_SIZE: u64 = 5 * 1024 * 1024;
const MAX_LYRIC_FILES: usize = 50_000;
const INDEX_BATCH_SIZE: usize = 200;
pub(super) const LIBRARY_DIRECTORY_PREFERENCE: &str = "lyrics.library_directory";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LibraryScanPhase {
    Idle,
    Discovering,
    Indexing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryScanStatus {
    pub scan_id: u64,
    pub library_dir: String,
    pub phase: LibraryScanPhase,
    pub discovered: u64,
    pub processed: u64,
    pub total: Option<u64>,
    pub skipped: u64,
    pub added: u64,
    pub updated: u64,
    pub unchanged: u64,
    pub removed: u64,
    pub failed: u64,
    pub first_failure: Option<String>,
    pub error: Option<String>,
}

pub(super) struct LibraryScanCoordinator {
    generation: AtomicU64,
    status: Mutex<LibraryScanStatus>,
}

impl LibraryScanCoordinator {
    pub(super) fn new(library_dir: &Path) -> Self {
        Self {
            generation: AtomicU64::new(0),
            status: Mutex::new(LibraryScanStatus {
                scan_id: 0,
                library_dir: library_dir.to_string_lossy().into_owned(),
                phase: LibraryScanPhase::Idle,
                discovered: 0,
                processed: 0,
                total: None,
                skipped: 0,
                added: 0,
                updated: 0,
                unchanged: 0,
                removed: 0,
                failed: 0,
                first_failure: None,
                error: None,
            }),
        }
    }

    fn begin(&self, library_dir: &Path) -> LibraryScanStatus {
        let scan_id = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let status = LibraryScanStatus {
            scan_id,
            library_dir: library_dir.to_string_lossy().into_owned(),
            phase: LibraryScanPhase::Discovering,
            discovered: 0,
            processed: 0,
            total: None,
            skipped: 0,
            added: 0,
            updated: 0,
            unchanged: 0,
            removed: 0,
            failed: 0,
            first_failure: None,
            error: None,
        };
        *self
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = status.clone();
        status
    }

    fn is_current(&self, scan_id: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == scan_id
    }

    fn update(
        &self,
        scan_id: u64,
        update: impl FnOnce(&mut LibraryScanStatus),
    ) -> Option<LibraryScanStatus> {
        if !self.is_current(scan_id) {
            return None;
        }
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !self.is_current(scan_id) || status.scan_id != scan_id {
            return None;
        }
        update(&mut status);
        Some(status.clone())
    }

    fn snapshot(&self) -> LibraryScanStatus {
        self.status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

pub(super) fn initialize_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute(
        "CREATE INDEX IF NOT EXISTS lyric_files_content_hash ON lyric_files(content_hash)",
        [],
    )?;
    connection.execute(
        "CREATE INDEX IF NOT EXISTS lyric_associations_content_path
         ON lyric_associations(content_path)",
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
    let added_app_owned = ensure_column(
        connection,
        "lyric_files",
        "app_owned",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    if added_app_owned {
        connection.execute(
            "UPDATE lyric_files SET app_owned=0 WHERE source IN ('本地文件', '本地导入', '手动导入')",
            [],
        )?;
    } else {
        connection.execute(
            "UPDATE lyric_files SET app_owned=0
             WHERE source IN ('本地文件', '本地导入', '手动导入') AND app_owned!=0",
            [],
        )?;
    }
    Ok(())
}

pub(super) fn normalize_collision_metadata(connection: &Connection) -> rusqlite::Result<()> {
    let rows = {
        let mut statement = connection.prepare(
            "SELECT content_path, title, artist
             FROM lyric_files
             WHERE app_owned=1 AND source NOT IN ('本地文件', '本地导入', '手动导入')",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    PathBuf::from(row.get::<_, String>(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    for (path, title, artist) in rows {
        let Some((base_path, filename_artist, filename_title)) = collision_filename_parts(&path)
        else {
            continue;
        };
        if !base_path.is_file() || lrc_tag(&read_lyric(&path).unwrap_or_default(), "ti").is_some() {
            continue;
        }
        if title != filename_title || artist != filename_artist {
            continue;
        }
        connection.execute(
            "UPDATE lyric_files SET title=?2, artist=?3, updated_at=unixepoch()
             WHERE content_path=?1 AND app_owned=1",
            params![path.to_string_lossy(), filename_title, filename_artist],
        )?;
        connection.execute(
            "UPDATE lyric_associations SET title=?2, artist=?3
             WHERE content_path=?1",
            params![path.to_string_lossy(), filename_title, filename_artist],
        )?;
    }
    Ok(())
}

impl Storage {
    pub fn set_library_directory(&self, path: &str) -> Result<PathBuf, String> {
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
        Ok(self.library_directory())
    }

    pub fn begin_library_scan(&self) -> LibraryScanStatus {
        self.scanner.begin(&self.library_directory())
    }

    pub fn library_scan_status(&self) -> LibraryScanStatus {
        self.scanner.snapshot()
    }

    pub fn run_library_scan(
        &self,
        scan_id: u64,
        mut publish: impl FnMut(&LibraryScanStatus),
    ) -> Result<bool, String> {
        let snapshot = self.library_scan_status();
        if snapshot.scan_id != scan_id {
            return Ok(false);
        }
        let root = PathBuf::from(snapshot.library_dir);
        let mut connection = Connection::open(&self.database_path)
            .map_err(|error| format!("打开歌词索引数据库失败：{error}"))?;
        connection
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(|error| format!("初始化歌词索引连接失败：{error}"))?;

        let mut files = Vec::new();
        let mut discovery_skipped = 0_u64;
        collect_lyric_files(
            &root,
            &mut files,
            &mut discovery_skipped,
            &self.scanner,
            scan_id,
            &mut publish,
        )?;
        let Some(status) = self.scanner.update(scan_id, |status| {
            status.phase = LibraryScanPhase::Indexing;
            status.discovered = files.len() as u64;
            status.total = Some(files.len() as u64);
            status.skipped = discovery_skipped;
        }) else {
            return Ok(false);
        };
        publish(&status);

        let mut seen = HashSet::with_capacity(files.len());
        for batch in files.chunks(INDEX_BATCH_SIZE) {
            if !self.scanner.is_current(scan_id) {
                return Ok(false);
            }
            let transaction = connection
                .transaction()
                .map_err(|error| format!("开始歌词索引事务失败：{error}"))?;
            let mut batch_added = 0_u64;
            let mut batch_updated = 0_u64;
            let mut batch_unchanged = 0_u64;
            let mut batch_failed = 0_u64;
            let mut first_failure = None;
            for path in batch {
                let path_string = path.to_string_lossy().into_owned();
                seen.insert(path_string.clone());
                match index_file_if_changed(&transaction, path) {
                    Ok(IndexOutcome::Added) => batch_added += 1,
                    Ok(IndexOutcome::Updated) => batch_updated += 1,
                    Ok(IndexOutcome::Unchanged) => batch_unchanged += 1,
                    Err(error) => {
                        batch_failed += 1;
                        if first_failure.is_none() {
                            first_failure = Some(format!("{}：{error}", path.display()));
                        }
                        transaction
                            .execute(
                                "DELETE FROM lyric_files WHERE content_path=?1",
                                params![path_string],
                            )
                            .map_err(|error| format!("清理不可用歌词索引失败：{error}"))?;
                    }
                }
            }
            transaction
                .commit()
                .map_err(|error| format!("提交歌词索引失败：{error}"))?;
            let Some(status) = self.scanner.update(scan_id, |status| {
                status.processed += batch.len() as u64;
                status.added += batch_added;
                status.updated += batch_updated;
                status.unchanged += batch_unchanged;
                status.failed += batch_failed;
                if status.first_failure.is_none() {
                    status.first_failure = first_failure;
                }
            }) else {
                return Ok(false);
            };
            publish(&status);
        }

        if !self.scanner.is_current(scan_id) {
            return Ok(false);
        }
        if discovery_skipped == 0 {
            let removed = cleanup_missing_files(&mut connection, &root, &seen)?;
            let Some(status) = self
                .scanner
                .update(scan_id, |status| status.removed = removed)
            else {
                return Ok(false);
            };
            publish(&status);
        }
        self.cleanup_orphan_app_owned_files();
        let Some(status) = self.scanner.update(scan_id, |status| {
            status.phase = LibraryScanPhase::Completed;
            status.processed = status.total.unwrap_or(status.processed);
        }) else {
            return Ok(false);
        };
        publish(&status);
        Ok(true)
    }

    pub fn fail_library_scan(&self, scan_id: u64, error: String) -> Option<LibraryScanStatus> {
        self.scanner.update(scan_id, |status| {
            status.phase = LibraryScanPhase::Failed;
            status.error = Some(error);
        })
    }

    pub fn library_directory(&self) -> PathBuf {
        self.library_dir
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

fn collect_lyric_files(
    root: &Path,
    output: &mut Vec<PathBuf>,
    skipped: &mut u64,
    coordinator: &LibraryScanCoordinator,
    scan_id: u64,
    publish: &mut impl FnMut(&LibraryScanStatus),
) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|error| format!("无法读取歌词文件夹：{error}"))?;
    for entry in entries {
        if !coordinator.is_current(scan_id) {
            return Ok(());
        }
        let Ok(entry) = entry else {
            *skipped += 1;
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            *skipped += 1;
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if let Err(error) = collect_lyric_files(
                &entry.path(),
                output,
                skipped,
                coordinator,
                scan_id,
                publish,
            ) {
                if error.contains("最多索引 50000 个文件") {
                    return Err(error);
                }
                *skipped += 1;
            }
        } else if file_type.is_file() && has_supported_extension(&entry.path()) {
            push_discovered_file(output, entry.path())?;
            if output.len() % 250 == 0 {
                if let Some(status) = coordinator.update(scan_id, |status| {
                    status.discovered = output.len() as u64;
                    status.skipped = *skipped;
                }) {
                    publish(&status);
                }
            }
        }
    }
    Ok(())
}

fn push_discovered_file(output: &mut Vec<PathBuf>, path: PathBuf) -> Result<(), String> {
    if output.len() >= MAX_LYRIC_FILES {
        return Err("单个歌词文件夹最多索引 50000 个文件".into());
    }
    output.push(path);
    Ok(())
}

enum IndexOutcome {
    Added,
    Updated,
    Unchanged,
}

fn index_file_if_changed(
    connection: &Transaction<'_>,
    path: &Path,
) -> Result<IndexOutcome, String> {
    let file_metadata = fs::metadata(path).map_err(|error| format!("读取歌词文件失败：{error}"))?;
    if file_metadata.len() > MAX_LYRIC_FILE_SIZE {
        return Err("歌词文件超过 5 MB，已跳过".into());
    }
    let modified_at_ms = modified_ms(&file_metadata).map(|value| value as i64);
    let existing = connection
        .query_row(
            "SELECT file_size, modified_at_ms, app_owned, title, artist
             FROM lyric_files WHERE content_path=?1",
            params![path.to_string_lossy()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, i64>(2)? != 0,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("读取歌词索引状态失败：{error}"))?;
    if modified_at_ms.is_some()
        && existing.as_ref().is_some_and(|(size, modified, _, _, _)| {
            (*size, *modified) == (file_metadata.len() as i64, modified_at_ms)
        })
    {
        return Ok(IndexOutcome::Unchanged);
    }
    let raw = read_lyric(path)?;
    let metadata = lyric_metadata(path, &raw, "本地文件");
    let (indexed_title, indexed_artist) = existing
        .as_ref()
        .filter(|(_, _, app_owned, _, _)| *app_owned)
        .and_then(|(_, _, _, title, artist)| {
            collision_filename_parts(path).map(|_| (title.clone(), artist.clone()))
        })
        .unwrap_or_else(|| (metadata.title.clone(), metadata.artist.clone()));
    let hash = content_hash(&raw);
    let fingerprint = content_hash(&format!(
        "{}|{}|{}|{}",
        indexed_title,
        indexed_artist,
        metadata.duration_ms.unwrap_or(0),
        file_metadata.len()
    ));
    connection
        .execute(
            "INSERT INTO lyric_files
               (content_path, title, artist, source, original_format, manual_selected,
                content_hash, folder_id, managed, file_size, modified_at_ms, duration_ms,
                file_fingerprint, has_translation, has_word_timing, has_romanization,
                app_owned, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, NULL, 1, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, unixepoch())
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
                indexed_title,
                indexed_artist,
                metadata.source,
                metadata.format,
                hash,
                file_metadata.len() as i64,
                modified_at_ms,
                metadata.duration_ms.map(|value| value as i64),
                fingerprint,
                metadata.has_translation,
                metadata.has_word_timing,
                metadata.has_romanization,
            ],
        )
        .map_err(|error| format!("更新歌词索引失败：{error}"))?;
    Ok(if existing.is_some() {
        IndexOutcome::Updated
    } else {
        IndexOutcome::Added
    })
}

fn collision_filename_parts(path: &Path) -> Option<(PathBuf, String, String)> {
    let stem = path.file_stem()?.to_str()?;
    let suffix_start = stem.rfind(" (")?;
    let suffix = stem.get(suffix_start + 2..stem.len().checked_sub(1)?)?;
    if suffix.is_empty() || !suffix.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    let base_stem = stem.get(..suffix_start)?.trim_end();
    let (artist, title) = base_stem.split_once(" - ")?;
    let extension = path.extension()?.to_str()?;
    Some((
        path.with_file_name(format!("{base_stem}.{extension}")),
        artist.to_owned(),
        title.to_owned(),
    ))
}

fn cleanup_missing_files(
    connection: &mut Connection,
    root: &Path,
    seen: &HashSet<String>,
) -> Result<u64, String> {
    let prefix = directory_prefix(root);
    let mut statement = connection
        .prepare(
            "SELECT content_path FROM lyric_files
             WHERE managed=1 AND folder_id IS NULL
               AND substr(content_path, 1, length(?1))=?1",
        )
        .map_err(|error| format!("读取已有索引失败：{error}"))?;
    let missing = statement
        .query_map(params![prefix], |row| row.get::<_, String>(0))
        .map_err(|error| format!("查询已有索引失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析已有索引失败：{error}"))?
        .into_iter()
        .filter(|path| !seen.contains(path))
        .collect::<Vec<_>>();
    drop(statement);
    let transaction = connection
        .transaction()
        .map_err(|error| format!("开始清理索引事务失败：{error}"))?;
    let removed = missing.len() as u64;
    for path in missing {
        transaction
            .execute(
                "DELETE FROM lyric_associations WHERE content_path=?1",
                params![path],
            )
            .map_err(|error| format!("清理失效关联失败：{error}"))?;
        transaction
            .execute(
                "DELETE FROM lyric_files WHERE content_path=?1",
                params![path],
            )
            .map_err(|error| format!("清理失效索引失败：{error}"))?;
    }
    transaction
        .commit()
        .map_err(|error| format!("提交索引清理失败：{error}"))?;
    Ok(removed)
}

fn directory_prefix(root: &Path) -> String {
    let mut prefix = root.to_string_lossy().into_owned();
    if !prefix.ends_with(std::path::MAIN_SEPARATOR) {
        prefix.push(std::path::MAIN_SEPARATOR);
    }
    prefix
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

    fn scan(storage: &Storage) -> LibraryScanStatus {
        let status = storage.begin_library_scan();
        assert!(storage.run_library_scan(status.scan_id, |_| {}).unwrap());
        storage.library_scan_status()
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
            reopened.library_directory(),
            new_dir.canonicalize().unwrap()
        );
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn newer_scan_cancels_previous_generation() {
        let (app_dir, library_dir) = test_dirs("scan-cancellation");
        fs::create_dir_all(&library_dir).unwrap();
        let storage = Storage::open(app_dir.clone(), library_dir).unwrap();
        let old = storage.begin_library_scan();
        let current = storage.begin_library_scan();
        assert!(!storage.run_library_scan(old.scan_id, |_| {}).unwrap());
        assert!(storage.run_library_scan(current.scan_id, |_| {}).unwrap());
        assert_eq!(storage.library_scan_status().scan_id, current.scan_id);
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn scan_commits_and_reports_in_two_hundred_file_batches() {
        let (app_dir, library_dir) = test_dirs("scan-batches");
        fs::create_dir_all(&library_dir).unwrap();
        for index in 0..201 {
            fs::write(
                library_dir.join(format!("Artist - Song {index}.lrc")),
                format!("[00:01]Line {index}"),
            )
            .unwrap();
        }
        let storage = Storage::open(app_dir.clone(), library_dir).unwrap();
        let status = storage.begin_library_scan();
        let mut processed = Vec::new();
        assert!(storage
            .run_library_scan(status.scan_id, |status| {
                if status.phase == LibraryScanPhase::Indexing && status.processed > 0 {
                    processed.push(status.processed);
                }
            })
            .unwrap());
        assert_eq!(processed, vec![200, 201]);
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn discovered_file_limit_rejects_the_next_file() {
        let mut files = vec![PathBuf::new(); MAX_LYRIC_FILES];
        assert!(push_discovered_file(&mut files, PathBuf::new()).is_err());
        assert_eq!(files.len(), MAX_LYRIC_FILES);
    }
}
