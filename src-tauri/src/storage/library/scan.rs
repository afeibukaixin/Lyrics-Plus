use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::Serialize;

use super::super::Storage;
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

    pub(super) fn is_current(&self, scan_id: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == scan_id
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

    pub(super) fn snapshot(&self) -> LibraryScanStatus {
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
