use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::super::{content_hash, Storage};
use super::discovery::{canonical_directory, collect_lyric_files};
use super::index::{cleanup_missing_files, index_file_if_changed, IndexOutcome, INDEX_BATCH_SIZE};
use super::LIBRARY_DIRECTORY_PREFERENCE;

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

pub(in crate::storage) struct LibraryScanCoordinator {
    generation: AtomicU64,
    status: Mutex<LibraryScanStatus>,
}

impl LibraryScanCoordinator {
    pub(in crate::storage) fn new(library_dir: &Path) -> Self {
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

    pub(super) fn begin(&self, library_dir: &Path) -> LibraryScanStatus {
        let mut current_status = self
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
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
        *current_status = status.clone();
        status
    }

    pub(super) fn is_current(&self, scan_id: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == scan_id
    }

    pub(in crate::storage) fn cancel_if_matches(&self, library_dir: &Path) -> bool {
        let mut status = self
            .status
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let matches_directory = Path::new(&status.library_dir) == library_dir;
        let is_running = matches!(
            status.phase,
            LibraryScanPhase::Discovering | LibraryScanPhase::Indexing
        );
        if !matches_directory || !is_running {
            return false;
        }
        self.generation.fetch_add(1, Ordering::SeqCst);
        status.phase = LibraryScanPhase::Idle;
        status.total = None;
        status.error = None;
        true
    }

    pub(super) fn update(
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

    pub(in crate::storage) fn snapshot(&self) -> LibraryScanStatus {
        self.status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

impl Storage {
    pub fn set_library_directory(&self, path: &str) -> Result<PathBuf, String> {
        let path = canonical_directory(path)?;
        let metadata =
            fs::metadata(&path).map_err(|error| format!("无法读取歌词文件夹信息：{error}"))?;
        if metadata.permissions().readonly() {
            return Err("所选歌词文件夹不可写".into());
        }

        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let managed_root_id = super::super::MANAGED_ROOT_ID;
        let previous_path = connection
            .query_row(
                "SELECT path FROM library_roots WHERE root_id=?1 AND root_kind='managed'",
                params![managed_root_id],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .map(PathBuf::from);
        let enabled_roots = {
            let mut statement = connection
                .prepare(
                    "SELECT root_id, path FROM library_roots
                     WHERE enabled=1 AND root_id!=?1",
                )
                .map_err(|error| format!("读取歌词根目录失败：{error}"))?;
            let roots = statement
                .query_map(params![managed_root_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        PathBuf::from(row.get::<_, String>(1)?),
                    ))
                })
                .map_err(|error| format!("解析歌词根目录失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析歌词根目录失败：{error}"))?;
            roots
        };
        if let Some((root_id, _)) = enabled_roots.iter().find(|(_, existing_path)| {
            path.starts_with(existing_path) || existing_path.starts_with(&path)
        }) {
            return Err(format!("应用下载目录与已有根目录重叠：{root_id}"));
        }

        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始更新应用下载根目录失败：{error}"))?;
        if let Some(previous_path) = previous_path.filter(|old_path| old_path != &path) {
            let already_registered = enabled_roots
                .iter()
                .any(|(_, existing_path)| existing_path == &previous_path);
            if !already_registered {
                let legacy_root_id =
                    format!("legacy_{}", content_hash(&previous_path.to_string_lossy()));
                let root_id_conflict = transaction
                    .query_row(
                        "SELECT path FROM library_roots WHERE root_id=?1",
                        params![legacy_root_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()
                    .map_err(|error| format!("检查历史歌词根目录失败：{error}"))?;
                if root_id_conflict
                    .is_some_and(|registered_path| PathBuf::from(registered_path) != previous_path)
                {
                    return Err("历史歌词根目录标识冲突，请重新选择下载目录".into());
                }
                transaction
                    .execute(
                        "INSERT INTO library_roots
                           (root_id, root_kind, display_name, path, read_only, enabled, updated_at)
                         VALUES (?1, 'legacy', '历史下载目录', ?2, 1, 1, unixepoch())
                         ON CONFLICT(path) DO NOTHING",
                        params![legacy_root_id, previous_path.to_string_lossy()],
                    )
                    .map_err(|error| format!("保留历史歌词根目录失败：{error}"))?;
            }
        }
        transaction
            .execute(
                "INSERT INTO app_preferences (key, value, updated_at)
                     VALUES (?1, ?2, unixepoch())
                     ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=unixepoch()",
                params![LIBRARY_DIRECTORY_PREFERENCE, path.to_string_lossy()],
            )
            .map_err(|error| format!("保存歌词目录失败：{error}"))?;
        transaction
            .execute(
                "UPDATE library_roots
                     SET path=?1, updated_at=unixepoch()
                     WHERE root_id=?2 AND root_kind='managed'",
                params![path.to_string_lossy(), managed_root_id],
            )
            .map_err(|error| format!("更新应用下载根目录失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交应用下载根目录更新失败：{error}"))?;

        *self
            .library_dir
            .write()
            .unwrap_or_else(|error| error.into_inner()) = path;
        Ok(self.library_directory())
    }

    pub fn begin_library_scan(&self) -> LibraryScanStatus {
        self.scanner.begin(&self.library_directory())
    }

    pub fn begin_library_root_scan(&self, root_id: &str) -> Result<LibraryScanStatus, String> {
        let root = self.library_root_path(root_id)?;
        Ok(self.scanner.begin(&root))
    }

    pub fn library_scan_status(&self) -> LibraryScanStatus {
        self.scanner.snapshot()
    }

    pub fn run_library_scan(
        &self,
        scan_id: u64,
        publish: impl FnMut(&LibraryScanStatus),
    ) -> Result<bool, String> {
        self.run_library_root_scan(scan_id, super::super::MANAGED_ROOT_ID, publish)
    }

    pub fn run_library_root_scan(
        &self,
        scan_id: u64,
        root_id: &str,
        mut publish: impl FnMut(&LibraryScanStatus),
    ) -> Result<bool, String> {
        let snapshot = self.library_scan_status();
        if snapshot.scan_id != scan_id {
            return Ok(false);
        }
        let root = self.library_root_path(root_id)?;
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
                match index_file_if_changed(&transaction, path, root_id) {
                    Ok(IndexOutcome::Added) => batch_added += 1,
                    Ok(IndexOutcome::Updated) => batch_updated += 1,
                    Ok(IndexOutcome::Unchanged) => batch_unchanged += 1,
                    Err(error) => {
                        batch_failed += 1;
                        if first_failure.is_none() {
                            let display_path = path
                                .strip_prefix(&root)
                                .map(|relative| relative.display().to_string())
                                .unwrap_or_else(|_| "<library-file>".into());
                            first_failure = Some(format!("{}：{error}", display_path));
                        }
                        transaction
                            .execute(
                                "UPDATE lyric_files SET available=0, updated_at=unixepoch()
                                 WHERE content_path=?1",
                                params![path_string],
                            )
                            .map_err(|error| format!("标记不可用歌词索引失败：{error}"))?;
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
            let removed = cleanup_missing_files(&mut connection, &root, &seen, root_id)?;
            let Some(status) = self
                .scanner
                .update(scan_id, |status| status.removed = removed)
            else {
                return Ok(false);
            };
            publish(&status);
        }
        self.update_library_root_stats(root_id)?;
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

    fn library_root_path(&self, root_id: &str) -> Result<PathBuf, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .query_row(
                "SELECT path FROM library_roots WHERE root_id=?1 AND enabled=1",
                params![root_id],
                |row| row.get::<_, String>(0),
            )
            .map(PathBuf::from)
            .map_err(|error| format!("读取歌词根目录失败：{error}"))
    }

    fn update_library_root_stats(&self, root_id: &str) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let path = connection
            .query_row(
                "SELECT path FROM library_roots WHERE root_id=?1",
                params![root_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| format!("读取歌词根目录统计失败：{error}"))?;
        let prefix = if path.ends_with(std::path::MAIN_SEPARATOR) {
            path.clone()
        } else {
            format!("{path}{}", std::path::MAIN_SEPARATOR)
        };
        connection
            .execute(
                "UPDATE library_roots SET
                   file_count=(SELECT COUNT(*) FROM lyric_files
                               WHERE content_path=?1 OR substr(content_path, 1, length(?2))=?2),
                   unavailable_count=(SELECT COUNT(*) FROM lyric_files
                                      WHERE available=0 AND
                                        (content_path=?1 OR substr(content_path, 1, length(?2))=?2)),
                   last_scan_at=unixepoch(), last_error=NULL, updated_at=unixepoch()
                 WHERE root_id=?3",
                params![path, prefix, root_id],
            )
            .map_err(|error| format!("更新歌词根目录统计失败：{error}"))?;
        Ok(())
    }
}
