use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use super::super::{content_hash, read_lyric_text};
use super::metadata::{collision_filename_parts, lyric_metadata};

pub(super) const MAX_LYRIC_FILE_SIZE: u64 = 5 * 1024 * 1024;
pub(super) const INDEX_BATCH_SIZE: usize = 200;

pub(super) enum IndexOutcome {
    Added,
    Updated,
    Unchanged,
}

pub(super) fn index_file_if_changed(
    connection: &Transaction<'_>,
    path: &Path,
    root_id: &str,
) -> Result<IndexOutcome, String> {
    let file_metadata = fs::metadata(path).map_err(|error| format!("读取歌词文件失败：{error}"))?;
    if file_metadata.len() > MAX_LYRIC_FILE_SIZE {
        return Err("歌词文件超过 5 MB，已跳过".into());
    }
    let modified_at_ms = modified_ms(&file_metadata).map(|value| value as i64);
    let existing = connection
        .query_row(
            "SELECT file_size, modified_at_ms, app_owned, title, artist, available, content_hash
             FROM lyric_files WHERE content_path=?1",
            params![path.to_string_lossy()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, i64>(2)? != 0,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)? != 0,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("读取歌词索引状态失败：{error}"))?;
    if modified_at_ms.is_some()
        && existing
            .as_ref()
            .is_some_and(|(size, modified, _, _, _, _, _)| {
                (*size, *modified) == (file_metadata.len() as i64, modified_at_ms)
            })
    {
        connection
            .execute(
                "UPDATE lyric_files SET available=1, updated_at=unixepoch()
                     WHERE content_path=?1",
                params![path.to_string_lossy()],
            )
            .map_err(|error| format!("恢复歌词索引可用状态失败：{error}"))?;
        if let Some((_, _, _, _, _, _, content_hash)) = existing.as_ref() {
            restore_v2_asset_references(connection, content_hash, path, root_id)?;
        }
        return Ok(IndexOutcome::Unchanged);
    }
    let raw = read_lyric_text(path)?;
    let metadata = lyric_metadata(path, &raw, "本地文件");
    let (indexed_title, indexed_artist) = existing
        .as_ref()
        .filter(|(_, _, app_owned, _, _, _, _)| *app_owned)
        .and_then(|(_, _, _, title, artist, _, _)| {
            collision_filename_parts(path).map(|_| (title.clone(), artist.clone()))
        })
        .unwrap_or_else(|| (metadata.title.clone(), metadata.artist.clone()));
    let hash = content_hash(&raw);
    restore_fingerprint_references(connection, &hash, path, root_id)?;
    let fingerprint = content_hash(&format!(
        "{}|{}|{}|{}",
        indexed_title,
        indexed_artist,
        metadata.duration_ms.unwrap_or(0),
        file_metadata.len()
    ));
    if let Some((_, _, _, _, _, _, previous_hash)) = existing.as_ref() {
        if previous_hash != &hash {
            mark_v2_asset_unavailable(connection, previous_hash, path, root_id)?;
        }
    }
    connection
        .execute(
            "INSERT INTO lyric_files
               (content_path, title, artist, source, original_format, manual_selected,
                content_hash, folder_id, managed, file_size, modified_at_ms, duration_ms,
                file_fingerprint, has_translation, has_word_timing, has_romanization,
               app_owned, available, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, NULL, 1, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, 1, unixepoch())
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
               has_romanization=excluded.has_romanization, available=1, updated_at=unixepoch()",
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
    restore_v2_asset_references(connection, &hash, path, root_id)?;
    Ok(if existing.is_some() {
        IndexOutcome::Updated
    } else {
        IndexOutcome::Added
    })
}

fn restore_fingerprint_references(
    connection: &Transaction<'_>,
    hash: &str,
    path: &Path,
    root_id: &str,
) -> Result<(), String> {
    let path_string = path.to_string_lossy().into_owned();
    let root = root_path(connection, root_id)?;
    let prefix = directory_prefix(&root);
    let previous_paths = connection
        .prepare(
            "SELECT content_path FROM lyric_files
             WHERE content_hash=?1 AND content_path!=?2
               AND substr(content_path, 1, length(?3))=?3
             ORDER BY available ASC, updated_at DESC",
        )
        .and_then(|mut statement| {
            statement
                .query_map(params![hash, path_string, prefix], |row| {
                    row.get::<_, String>(0)
                })
                .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
        })
        .map_err(|error| format!("查询歌词改名关联失败：{error}"))?;
    let Some(previous_path) = previous_paths
        .into_iter()
        .find(|previous_path| !Path::new(previous_path).is_file())
    else {
        return Ok(());
    };
    connection
        .execute(
            "UPDATE lyric_associations SET content_path=?2
             WHERE content_path=?1",
            params![previous_path, path_string],
        )
        .map_err(|error| format!("恢复歌词改名关联失败：{error}"))?;
    connection
        .execute(
            "UPDATE lyric_assets SET
               root_dir_id=COALESCE(?3, root_dir_id),
               relative_path=COALESCE((
                 SELECT substr(?2, length(path)+2) FROM library_roots
                 WHERE root_id=?3 AND enabled=1
               ), relative_path),
               available=1, updated_at=unixepoch()
             WHERE content_fingerprint=?1",
            params![hash, path_string, root_id],
        )
        .map_err(|error| format!("恢复歌词资源改名关联失败：{error}"))?;
    let old_relative_path = Path::new(&previous_path)
        .strip_prefix(&root)
        .ok()
        .map(|value| value.to_string_lossy().into_owned());
    let new_relative_path = Path::new(&path_string)
        .strip_prefix(&root)
        .ok()
        .map(|value| value.to_string_lossy().into_owned());
    if let (Some(old_relative_path), Some(new_relative_path)) =
        (old_relative_path, new_relative_path)
    {
        connection
            .execute(
                "UPDATE lyric_asset_sources SET relative_path=?1, available=1, updated_at=unixepoch()
                 WHERE root_dir_id=?2 AND relative_path=?3 AND asset_id IN (
                   SELECT asset_id FROM lyric_assets WHERE content_fingerprint=?4
                 )",
                params![new_relative_path, root_id, old_relative_path, hash],
            )
            .map_err(|error| format!("恢复歌词来源改名关联失败：{error}"))?;
    }
    Ok(())
}

fn restore_v2_asset_references(
    connection: &Transaction<'_>,
    content_hash: &str,
    path: &Path,
    root_id: &str,
) -> Result<(), String> {
    let relative_path = path
        .strip_prefix(root_path(connection, root_id)?)
        .map_err(|_| "歌词文件不在登记根目录内".to_string())?
        .to_string_lossy()
        .into_owned();
    connection
        .execute(
            "UPDATE lyric_asset_sources SET available=1, updated_at=unixepoch()
             WHERE root_dir_id=?1 AND relative_path=?2 AND asset_id IN (
               SELECT asset_id FROM lyric_assets WHERE content_fingerprint=?3
             )",
            params![root_id, relative_path, content_hash],
        )
        .map_err(|error| format!("恢复歌词来源状态失败：{error}"))?;
    connection
        .execute(
            "UPDATE lyric_assets SET available=1, updated_at=unixepoch()
             WHERE content_fingerprint=?1 AND EXISTS (
               SELECT 1 FROM lyric_asset_sources
               WHERE lyric_asset_sources.asset_id=lyric_assets.asset_id
                 AND lyric_asset_sources.available=1
             )",
            params![content_hash],
        )
        .map_err(|error| format!("恢复歌词资源状态失败：{error}"))?;
    Ok(())
}

fn mark_v2_asset_unavailable(
    connection: &Transaction<'_>,
    content_hash: &str,
    path: &Path,
    root_id: &str,
) -> Result<(), String> {
    let relative_path = path
        .strip_prefix(root_path(connection, root_id)?)
        .map_err(|_| "歌词文件不在登记根目录内".to_string())?
        .to_string_lossy()
        .into_owned();
    connection
        .execute(
            "UPDATE lyric_asset_sources SET available=0, updated_at=unixepoch()
             WHERE root_dir_id=?1 AND relative_path=?2 AND asset_id IN (
               SELECT asset_id FROM lyric_assets WHERE content_fingerprint=?3
             )",
            params![root_id, relative_path, content_hash],
        )
        .map_err(|error| format!("标记旧歌词来源不可用失败：{error}"))?;
    connection
        .execute(
            "UPDATE lyric_assets SET available=0, updated_at=unixepoch()
             WHERE content_fingerprint=?1 AND NOT EXISTS (
               SELECT 1 FROM lyric_asset_sources
               WHERE lyric_asset_sources.asset_id=lyric_assets.asset_id
                 AND lyric_asset_sources.available=1
             )",
            params![content_hash],
        )
        .map_err(|error| format!("标记旧歌词资源不可用失败：{error}"))?;
    Ok(())
}

fn root_path(connection: &Transaction<'_>, root_id: &str) -> Result<PathBuf, String> {
    connection
        .query_row(
            "SELECT path FROM library_roots WHERE root_id=?1",
            params![root_id],
            |row| row.get::<_, String>(0),
        )
        .map(PathBuf::from)
        .map_err(|error| format!("读取歌词根目录路径失败：{error}"))
}

pub(super) fn cleanup_missing_files(
    connection: &mut Connection,
    root: &Path,
    seen: &HashSet<String>,
    root_id: &str,
) -> Result<u64, String> {
    let prefix = directory_prefix(root);
    let mut statement = connection
        .prepare(
            "SELECT content_path FROM lyric_files
             WHERE managed=1 AND folder_id IS NULL AND available=1
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
                "UPDATE lyric_files SET available=0, updated_at=unixepoch()
                 WHERE content_path=?1",
                params![path],
            )
            .map_err(|error| format!("标记失效索引失败：{error}"))?;
        if let Ok(relative_path) = Path::new(&path).strip_prefix(root) {
            let relative_path = relative_path.to_string_lossy();
            transaction
                .execute(
                    "UPDATE lyric_asset_sources SET available=0, updated_at=unixepoch()
                     WHERE root_dir_id=?2
                       AND relative_path=?1",
                    params![relative_path.as_ref(), root_id],
                )
                .map_err(|error| format!("标记失效歌词来源失败：{error}"))?;
            transaction
                .execute(
                    "UPDATE lyric_assets SET available=0, updated_at=unixepoch()
                     WHERE root_dir_id=?2
                       AND relative_path=?1
                       AND NOT EXISTS (
                         SELECT 1 FROM lyric_asset_sources
                         WHERE lyric_asset_sources.asset_id=lyric_assets.asset_id
                           AND lyric_asset_sources.available=1
                       )",
                    params![relative_path.as_ref(), root_id],
                )
                .map_err(|error| format!("标记失效歌词资源失败：{error}"))?;
        }
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
fn modified_ms(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_millis() as u64)
}
