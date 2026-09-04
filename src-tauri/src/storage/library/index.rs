use std::collections::HashSet;
use std::fs;
use std::path::Path;
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
    let raw = read_lyric_text(path)?;
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

pub(super) fn cleanup_missing_files(
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
fn modified_ms(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_millis() as u64)
}
