use std::collections::{hash_map::DefaultHasher, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use rusqlite::{params, Connection};

use super::super::{load_asset, Storage};
use super::lyrics::{library_lyric_sources, library_lyric_summaries, source_absolute_path};
use super::models::{CleanupItemResult, UnboundCleanupPreview, UnboundCleanupResult};
use super::pagination::library_page_parameters;

fn asset_is_unbound(connection: &Connection, asset_id: i64) -> bool {
    connection
        .query_row(
            "SELECT NOT EXISTS(SELECT 1 FROM recording_lyric_bindings WHERE asset_id=?1)
                AND NOT EXISTS(SELECT 1 FROM platform_lyric_overrides WHERE asset_id=?1)",
            params![asset_id],
            |row| row.get::<_, bool>(0),
        )
        .unwrap_or(false)
}

fn unbound_cleanup_revision(connection: &Connection) -> String {
    let values = connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM lyric_assets),
               (SELECT COALESCE(MAX(updated_at), 0) FROM lyric_assets),
               (SELECT COUNT(*) FROM recording_lyric_bindings),
               (SELECT COALESCE(MAX(updated_at), 0) FROM recording_lyric_bindings),
               (SELECT COUNT(*) FROM platform_lyric_overrides),
               (SELECT COALESCE(MAX(updated_at), 0) FROM platform_lyric_overrides),
               (SELECT COUNT(*) FROM lyric_asset_sources),
               (SELECT COALESCE(MAX(updated_at), 0) FROM lyric_asset_sources)",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            },
        )
        .unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    values.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn deletable_asset_sources(
    connection: &Connection,
    asset_id: i64,
) -> Result<Vec<(Option<i64>, PathBuf, u64)>, String> {
    let asset = load_asset(connection, asset_id)?.ok_or_else(|| "歌词资源不存在".to_string())?;
    let sources = library_lyric_sources(connection, &asset)?;
    let mut statement = connection
        .prepare("SELECT root_id, path FROM library_roots")
        .map_err(|error| format!("读取歌词目录失败：{error}"))?;
    let roots = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .and_then(|rows| rows.collect::<rusqlite::Result<std::collections::HashMap<_, _>>>())
        .map_err(|error| format!("读取歌词目录失败：{error}"))?;
    Ok(sources
        .into_iter()
        .filter(|source| source.writable && source.available)
        .filter_map(|source| {
            let root_path = source.root_id.as_ref().and_then(|id| roots.get(id))?;
            let path = source_absolute_path(
                source.root_id.as_deref(),
                Some(root_path.as_str()),
                source.relative_path.as_deref(),
            )?;
            if path.exists() {
                let canonical_root = std::fs::canonicalize(root_path).ok()?;
                let canonical_path = std::fs::canonicalize(&path).ok()?;
                if !canonical_path.starts_with(canonical_root) {
                    return None;
                }
            }
            Some((source.source_id, path, source.file_size))
        })
        .collect())
}

pub(super) fn asset_can_cleanup(connection: &Connection, asset_id: i64) -> bool {
    asset_is_unbound(connection, asset_id)
        && deletable_asset_sources(connection, asset_id).is_ok_and(|sources| !sources.is_empty())
}

fn unbound_cleanup_candidates(connection: &Connection) -> Result<(Vec<i64>, u64, u64), String> {
    let mut statement = connection
        .prepare(
            "SELECT asset.asset_id, source.root_dir_id, source.relative_path, root.path
             FROM lyric_assets AS asset
             JOIN lyric_asset_sources AS source ON source.asset_id=asset.asset_id
             JOIN library_roots AS root ON root.root_id=source.root_dir_id
             WHERE source.available=1 AND root.read_only=0
               AND source.source_kind IN ('managed', 'cache')
               AND root.root_kind IN ('managed', 'cache')
               AND NOT EXISTS(SELECT 1 FROM recording_lyric_bindings AS binding
                              WHERE binding.asset_id=asset.asset_id)
               AND NOT EXISTS(SELECT 1 FROM platform_lyric_overrides AS platform_override
                              WHERE platform_override.asset_id=asset.asset_id)
             UNION ALL
             SELECT asset.asset_id, asset.root_dir_id, asset.relative_path, root.path
             FROM lyric_assets AS asset
             JOIN library_roots AS root ON root.root_id=asset.root_dir_id
             WHERE asset.available=1 AND asset.relative_path IS NOT NULL
               AND root.read_only=0 AND asset.source_kind IN ('managed', 'cache')
               AND root.root_kind IN ('managed', 'cache')
               AND NOT EXISTS(SELECT 1 FROM lyric_asset_sources AS source
                              WHERE source.asset_id=asset.asset_id)
               AND NOT EXISTS(SELECT 1 FROM recording_lyric_bindings AS binding
                              WHERE binding.asset_id=asset.asset_id)
               AND NOT EXISTS(SELECT 1 FROM platform_lyric_overrides AS platform_override
                              WHERE platform_override.asset_id=asset.asset_id)
             ORDER BY asset_id",
        )
        .map_err(|error| format!("准备未绑定歌词候选集合查询失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|error| format!("读取未绑定歌词候选失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析未绑定歌词候选失败：{error}"))?;
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    let mut file_count = 0_u64;
    let mut total_size = 0_u64;
    for (asset_id, root_id, relative_path, root_path) in rows {
        let Some(path) = source_absolute_path(
            root_id.as_deref(),
            root_path.as_deref(),
            relative_path.as_deref(),
        ) else {
            continue;
        };
        if path.exists() {
            let Some(canonical_root) = root_path
                .as_deref()
                .and_then(|root| std::fs::canonicalize(root).ok())
            else {
                continue;
            };
            let Some(canonical_path) = std::fs::canonicalize(&path).ok() else {
                continue;
            };
            if !canonical_path.starts_with(canonical_root) {
                continue;
            }
        }
        if seen.insert(asset_id) {
            ids.push(asset_id);
        }
        file_count += 1;
        total_size += std::fs::metadata(path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
    }
    Ok((ids, file_count, total_size))
}

impl Storage {
    pub fn preview_unbound_lyrics_cleanup(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<UnboundCleanupPreview, String> {
        // 预览可能需要检查较多文件，使用独立 WAL 连接，避免占用主连接互斥锁。
        let connection = Connection::open(&self.database_path)
            .map_err(|error| format!("打开未绑定歌词预览连接失败：{error}"))?;
        connection
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(|error| format!("初始化未绑定歌词预览连接失败：{error}"))?;
        let (candidate_ids, file_count, total_size) = unbound_cleanup_candidates(&connection)?;
        let (page, page_size, offset) = library_page_parameters(page, page_size);
        let total = candidate_ids.len() as u64;
        let page_ids = candidate_ids
            .into_iter()
            .skip(offset as usize)
            .take(page_size)
            .collect::<Vec<_>>();
        let items = library_lyric_summaries(&connection, &page_ids)?;
        Ok(UnboundCleanupPreview {
            items,
            total,
            page,
            page_size: page_size as u64,
            file_count,
            total_size,
            revision: unbound_cleanup_revision(&connection),
        })
    }

    pub fn cleanup_unbound_lyrics(
        &self,
        selection_mode: &str,
        asset_ids: &[i64],
        excluded_asset_ids: &[i64],
        revision: &str,
    ) -> Result<UnboundCleanupResult, String> {
        {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if unbound_cleanup_revision(&connection) != revision {
                return Err("清理预览已过期，请刷新后重新确认".into());
            }
        }
        let selected_ids = match selection_mode {
            "selected" => asset_ids.to_vec(),
            "allExcept" => {
                let connection = Connection::open(&self.database_path)
                    .map_err(|error| format!("打开未绑定歌词清理连接失败：{error}"))?;
                connection
                    .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
                    .map_err(|error| format!("初始化未绑定歌词清理连接失败：{error}"))?;
                let excluded = excluded_asset_ids.iter().copied().collect::<HashSet<_>>();
                let (ids, _, _) = unbound_cleanup_candidates(&connection)?;
                ids.into_iter()
                    .filter(|id| !excluded.contains(id))
                    .collect()
            }
            _ => return Err("未知的清理选择模式".into()),
        };
        let mut results = Vec::new();
        for asset_id in &selected_ids {
            results.push(self.cleanup_unbound_lyric(*asset_id));
        }
        let deleted_files = results.iter().map(|item| item.deleted_files).sum();
        let released_bytes = results.iter().map(|item| item.released_bytes).sum();
        results.retain(|item| item.error.is_some());
        Ok(UnboundCleanupResult {
            items: results,
            deleted_files,
            released_bytes,
        })
    }

    pub fn cleanup_selected_unbound_lyrics(
        &self,
        asset_ids: &[i64],
    ) -> Result<UnboundCleanupResult, String> {
        let mut seen = HashSet::new();
        let mut results = asset_ids
            .iter()
            .copied()
            .filter(|asset_id| seen.insert(*asset_id))
            .map(|asset_id| self.cleanup_unbound_lyric(asset_id))
            .collect::<Vec<_>>();
        let deleted_files = results.iter().map(|item| item.deleted_files).sum();
        let released_bytes = results.iter().map(|item| item.released_bytes).sum();
        results.retain(|item| item.error.is_some());
        Ok(UnboundCleanupResult {
            items: results,
            deleted_files,
            released_bytes,
        })
    }

    fn cleanup_unbound_lyric(&self, asset_id: i64) -> CleanupItemResult {
        let sources = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if !asset_is_unbound(&connection, asset_id) {
                return CleanupItemResult {
                    asset_id,
                    deleted_files: 0,
                    released_bytes: 0,
                    error: Some("歌词仍有关联，未执行删除".into()),
                };
            }
            match deletable_asset_sources(&connection, asset_id) {
                Ok(sources) if !sources.is_empty() => sources,
                Ok(_) => {
                    return CleanupItemResult {
                        asset_id,
                        deleted_files: 0,
                        released_bytes: 0,
                        error: Some("歌词没有可安全删除的托管文件".into()),
                    }
                }
                Err(error) => {
                    return CleanupItemResult {
                        asset_id,
                        deleted_files: 0,
                        released_bytes: 0,
                        error: Some(error),
                    }
                }
            }
        };
        let mut deleted = Vec::new();
        let mut first_error = None;
        for (source_id, path, size) in sources {
            match std::fs::remove_file(&path) {
                Ok(()) => deleted.push((source_id, path, size)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    deleted.push((source_id, path, size))
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(format!("删除 {} 失败：{error}", path.display()));
                    }
                }
            }
        }
        let deleted_files = deleted.len() as u64;
        let released_bytes = deleted.iter().map(|item| item.2).sum();
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = match connection.transaction() {
            Ok(value) => value,
            Err(error) => {
                return CleanupItemResult {
                    asset_id,
                    deleted_files,
                    released_bytes,
                    error: Some(format!("开始清理歌词索引失败：{error}")),
                }
            }
        };
        for (source_id, path, _) in &deleted {
            if let Err(error) = transaction.execute(
                "DELETE FROM lyric_files WHERE content_path=?1",
                params![path.to_string_lossy()],
            ) {
                first_error.get_or_insert_with(|| format!("清理歌词文件索引失败：{error}"));
            }
            if let Some(source_id) = source_id {
                if let Err(error) = transaction.execute(
                    "DELETE FROM lyric_asset_sources WHERE source_id=?1",
                    params![source_id],
                ) {
                    first_error.get_or_insert_with(|| format!("清理歌词来源索引失败：{error}"));
                }
            }
        }
        if transaction
            .query_row(
                "SELECT NOT EXISTS(SELECT 1 FROM lyric_asset_sources WHERE asset_id=?1)",
                params![asset_id],
                |row| row.get::<_, bool>(0),
            )
            .unwrap_or(false)
        {
            if let Err(error) = transaction.execute(
                "DELETE FROM lyric_assets WHERE asset_id=?1",
                params![asset_id],
            ) {
                first_error.get_or_insert_with(|| format!("清理歌词资源失败：{error}"));
            }
        } else if let Err(error) = transaction.execute(
            "UPDATE lyric_assets SET available=EXISTS(
                 SELECT 1 FROM lyric_asset_sources WHERE asset_id=?1 AND available=1
             ), updated_at=unixepoch() WHERE asset_id=?1",
            params![asset_id],
        ) {
            first_error.get_or_insert_with(|| format!("更新歌词资源状态失败：{error}"));
        }
        if let Err(error) = transaction.commit() {
            first_error.get_or_insert_with(|| format!("提交歌词索引清理失败：{error}"));
        }
        CleanupItemResult {
            asset_id,
            deleted_files,
            released_bytes,
            error: first_error,
        }
    }
}
