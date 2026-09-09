use std::path::{Path, PathBuf};

use crate::lyrics::parse_lrc_with_options;
use rusqlite::{params, Connection, OptionalExtension};

use super::super::{
    load_artist_credits, load_asset, lyric_asset_content_paths, LyricAsset, Storage,
    EXTERNAL_ROOT_ID,
};
use super::index::{normalized_library_search_value, search_lyric_index_page};
use super::models::{
    LibraryLyricDetail, LibraryLyricPage, LibraryLyricRecording, LibraryLyricSource,
    LibraryLyricStatusCounts, LibraryLyricSummary,
};
use super::pagination::library_page_parameters;

impl Storage {
    pub fn list_library_lyrics(
        &self,
        query: &str,
        status: Option<&str>,
        source_kind: Option<&str>,
        page: u64,
        page_size: u64,
    ) -> Result<LibraryLyricPage, String> {
        let query = normalized_library_search_value(query.trim());
        if !query.is_empty() {
            self.rebuild_library_search_index()?;
        }
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if query.is_empty() {
            let (page, page_size, offset) = library_page_parameters(page, page_size);
            const LYRIC_STATUS_CTE: &str =
                "WITH binding_stats AS (
                   SELECT asset_id, COUNT(*) AS binding_count,
                          COALESCE(SUM(is_default), 0) AS default_count
                   FROM recording_lyric_bindings GROUP BY asset_id
                 ), override_stats AS (
                   SELECT asset_id, COUNT(*) AS override_count
                   FROM platform_lyric_overrides WHERE asset_id IS NOT NULL GROUP BY asset_id
                 ), assets_with_status AS (
                   SELECT asset.asset_id, asset.source_kind, asset.updated_at,
                          CASE
                            WHEN COALESCE(binding.default_count, 0) + COALESCE(overrides.override_count, 0) > 0 THEN 'inUse'
                            WHEN COALESCE(binding.binding_count, 0) > 0 THEN 'candidate'
                            ELSE 'unbound'
                          END AS status
                   FROM lyric_assets AS asset
                   LEFT JOIN binding_stats AS binding ON binding.asset_id=asset.asset_id
                   LEFT JOIN override_stats AS overrides ON overrides.asset_id=asset.asset_id
                 ) ";
            let counts_sql = format!(
                "{LYRIC_STATUS_CTE}
                 SELECT COALESCE(SUM(status='inUse'), 0),
                        COALESCE(SUM(status='candidate'), 0),
                        COALESCE(SUM(status='unbound'), 0)
                 FROM assets_with_status WHERE (?1 IS NULL OR source_kind=?1)"
            );
            let status_counts = connection
                .query_row(&counts_sql, params![source_kind], |row| {
                    Ok(LibraryLyricStatusCounts {
                        in_use: row.get::<_, i64>(0)?.max(0) as u64,
                        candidate: row.get::<_, i64>(1)?.max(0) as u64,
                        unbound: row.get::<_, i64>(2)?.max(0) as u64,
                    })
                })
                .map_err(|error| format!("读取歌词状态统计失败：{error}"))?;
            let page_sql = format!(
                "{LYRIC_STATUS_CTE}
                 SELECT asset_id FROM assets_with_status
                 WHERE (?1 IS NULL OR source_kind=?1) AND (?2 IS NULL OR status=?2)
                 ORDER BY updated_at DESC, asset_id DESC LIMIT ?3 OFFSET ?4"
            );
            let mut statement = connection
                .prepare(&page_sql)
                .map_err(|error| format!("准备歌词资料库分页查询失败：{error}"))?;
            let ids = statement
                .query_map(
                    params![source_kind, status, page_size as i64, offset],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|error| format!("读取歌词资料库失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析歌词资料库失败：{error}"))?;
            let items = library_lyric_summaries(&connection, &ids)?;
            let total = match status {
                Some("inUse") => status_counts.in_use,
                Some("candidate") => status_counts.candidate,
                Some("unbound") => status_counts.unbound,
                _ => status_counts.in_use + status_counts.candidate + status_counts.unbound,
            };
            return Ok(LibraryLyricPage {
                items,
                total,
                page,
                page_size: page_size as u64,
                status_counts,
            });
        }
        let (ids, total, page, page_size, status_counts) =
            search_lyric_index_page(&connection, &query, status, source_kind, page, page_size)?;
        let items = library_lyric_summaries(&connection, &ids)?;
        Ok(LibraryLyricPage {
            items,
            total,
            page,
            page_size: page_size as u64,
            status_counts,
        })
    }

    pub fn library_lyric_detail(&self, asset_id: i64) -> Result<LibraryLyricDetail, String> {
        let (asset, summary, sources, recordings, content_paths) = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let asset =
                load_asset(&connection, asset_id)?.ok_or_else(|| "歌词资源不存在".to_string())?;
            let summary = library_lyric_summary(&connection, asset_id)?
                .ok_or_else(|| "歌词资源不存在".to_string())?;
            let sources = library_lyric_sources(&connection, &asset)?;
            let recordings = library_lyric_recordings(&connection, asset_id)?;
            let content_paths = lyric_asset_content_paths(&connection, asset_id)
                .map(|(_, paths)| paths)
                .unwrap_or_default();
            (asset, summary, sources, recordings, content_paths)
        };
        let document = content_paths.into_iter().find_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|raw| parse_lrc_with_options(&raw, &asset.source_name, false).ok())
        });
        Ok(LibraryLyricDetail {
            summary,
            asset,
            sources,
            recordings,
            document,
        })
    }
}

impl Storage {
    pub fn delete_library_lyric_source(&self, asset_id: i64, source_id: i64) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let asset =
            load_asset(&connection, asset_id)?.ok_or_else(|| "歌词资源不存在".to_string())?;
        let sources = library_lyric_sources(&connection, &asset)?;
        let source = sources
            .iter()
            .find(|source| source.source_id == Some(source_id))
            .ok_or_else(|| "歌词来源不存在".to_string())?;
        if !source.writable {
            return Err("该歌词来源是只读文件".into());
        }
        let primary_was_deleted =
            asset.root_id == source.root_id && asset.relative_path == source.relative_path;
        let available_replacements = sources
            .iter()
            .filter(|candidate| candidate.source_id != Some(source_id) && candidate.available)
            .count();
        if (source.available || primary_was_deleted) && available_replacements == 0 {
            return Err("必须至少保留一个可用歌词来源".into());
        }
        let root_path = source.root_id.as_deref().and_then(|root_id| {
            connection
                .query_row(
                    "SELECT path FROM library_roots WHERE root_id=?1",
                    params![root_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .ok()
                .flatten()
        });
        let path = source_absolute_path(
            source.root_id.as_deref(),
            root_path.as_deref(),
            source.relative_path.as_deref(),
        )
        .ok_or_else(|| "歌词来源路径无效".to_string())?;
        if path.exists() {
            let canonical_root = root_path
                .as_deref()
                .and_then(|value| std::fs::canonicalize(value).ok())
                .ok_or_else(|| "歌词来源目录不可用".to_string())?;
            let canonical_path = std::fs::canonicalize(&path)
                .map_err(|error| format!("读取歌词来源路径失败：{error}"))?;
            if !canonical_path.starts_with(canonical_root) {
                return Err("歌词来源不在可写目录内".into());
            }
        }
        drop(connection);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("删除歌词来源文件失败：{error}")),
        }
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始清理歌词来源索引失败：{error}"))?;
        transaction
            .execute(
                "DELETE FROM lyric_files WHERE content_path=?1",
                params![path.to_string_lossy()],
            )
            .map_err(|error| format!("清理歌词文件索引失败：{error}"))?;
        transaction
            .execute(
                "DELETE FROM lyric_asset_sources WHERE source_id=?1 AND asset_id=?2",
                params![source_id, asset_id],
            )
            .map_err(|error| format!("清理歌词来源索引失败：{error}"))?;
        if primary_was_deleted {
            let replacement = transaction.query_row(
                "SELECT source_kind, source_name, provider_id, provider_item_id, root_dir_id, relative_path
                 FROM lyric_asset_sources WHERE asset_id=?1 AND available=1 ORDER BY source_id LIMIT 1",
                params![asset_id],
                |row| Ok((
                    row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, Option<String>>(5)?,
                )),
            ).optional().map_err(|error| format!("读取替代歌词来源失败：{error}"))?
                .ok_or_else(|| "必须至少保留一个可用歌词来源".to_string())?;
            transaction
                .execute(
                    "UPDATE lyric_assets SET source_kind=?2, source_name=?3, provider_id=?4,
                   provider_item_id=?5, root_dir_id=?6, relative_path=?7, available=1,
                   updated_at=unixepoch() WHERE asset_id=?1",
                    params![
                        asset_id,
                        replacement.0,
                        replacement.1,
                        replacement.2,
                        replacement.3,
                        replacement.4,
                        replacement.5
                    ],
                )
                .map_err(|error| format!("切换歌词主来源失败：{error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("提交歌词来源清理失败：{error}"))
    }
}

pub(super) fn library_lyric_summary(
    connection: &Connection,
    asset_id: i64,
) -> Result<Option<LibraryLyricSummary>, String> {
    let Some(asset) = load_asset(connection, asset_id)? else {
        return Ok(None);
    };
    let metadata = connection
        .query_row(
            "SELECT title, artist, file_size FROM lyric_files
             WHERE content_hash=?1
             ORDER BY available DESC, updated_at DESC LIMIT 1",
            params![asset.content_fingerprint],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("读取歌词文件元数据失败：{error}"))?;
    let recording_metadata = connection
        .query_row(
            "SELECT recording.title,
                    COALESCE((SELECT group_concat(raw_name, ' / ')
                              FROM recording_artist_credits
                              WHERE recording_id=recording.recording_id), '')
             FROM recording_lyric_bindings AS binding
             JOIN recordings AS recording ON recording.recording_id=binding.recording_id
             WHERE binding.asset_id=?1 ORDER BY binding.is_default DESC, binding.updated_at DESC
             LIMIT 1",
            params![asset_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| format!("读取歌词关联歌曲元数据失败：{error}"))?;
    let (title, artist, fallback_size) = metadata.unwrap_or_else(|| {
        let (title, artist) =
            recording_metadata.unwrap_or_else(|| ("未知歌曲".into(), "未知歌手".into()));
        (title, artist, 0)
    });
    let (binding_count, default_count) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(is_default), 0)
             FROM recording_lyric_bindings WHERE asset_id=?1",
            params![asset_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|error| format!("读取歌词绑定统计失败：{error}"))?;
    let override_count = connection
        .query_row(
            "SELECT COUNT(*) FROM platform_lyric_overrides WHERE asset_id=?1",
            params![asset_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("读取歌词平台使用统计失败：{error}"))?;
    let source_count = connection
        .query_row(
            "SELECT COUNT(*) FROM lyric_asset_sources WHERE asset_id=?1",
            params![asset_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("读取歌词来源数量失败：{error}"))?;
    let file_size = connection
        .query_row(
            "SELECT COALESCE(SUM(file_size), ?2) FROM lyric_files WHERE content_hash=?1 AND available=1",
            params![asset.content_fingerprint, fallback_size],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(fallback_size)
        .max(0) as u64;
    let active_count = default_count.max(0) + override_count.max(0);
    let status = if active_count > 0 {
        "inUse"
    } else if binding_count > 0 {
        "candidate"
    } else {
        "unbound"
    };
    Ok(Some(LibraryLyricSummary {
        asset_id,
        title,
        artist,
        source_name: asset.source_name.clone(),
        source_kind: asset.source_kind.clone(),
        original_format: asset.original_format.clone(),
        language: asset.language.clone(),
        has_word_timing: asset.has_word_timing,
        has_translation: asset.has_translation,
        has_romanization: asset.has_romanization,
        available: asset.available,
        active_count: active_count as u64,
        binding_count: binding_count.max(0) as u64,
        source_count: source_count.max(if asset.relative_path.is_some() { 1 } else { 0 }) as u64,
        file_size,
        status: status.into(),
        content_fingerprint: asset.content_fingerprint,
    }))
}

pub(super) fn library_lyric_summaries(
    connection: &Connection,
    asset_ids: &[i64],
) -> Result<Vec<LibraryLyricSummary>, String> {
    if asset_ids.is_empty() {
        return Ok(Vec::new());
    }
    let ids_json = serde_json::to_string(asset_ids)
        .map_err(|error| format!("编码歌词资料库分页失败：{error}"))?;
    let mut statement = connection
        .prepare(
            "WITH page_ids AS (
               SELECT CAST(value AS INTEGER) AS asset_id, key AS ordinal FROM json_each(?1)
             ), binding_stats AS (
               SELECT binding.asset_id, COUNT(*) AS binding_count,
                      COALESCE(SUM(binding.is_default), 0) AS default_count
               FROM recording_lyric_bindings AS binding
               JOIN page_ids USING(asset_id) GROUP BY binding.asset_id
             ), override_stats AS (
               SELECT overrides.asset_id, COUNT(*) AS override_count
               FROM platform_lyric_overrides AS overrides
               JOIN page_ids USING(asset_id) GROUP BY overrides.asset_id
             ), source_stats AS (
               SELECT source.asset_id, COUNT(*) AS source_count
               FROM lyric_asset_sources AS source
               JOIN page_ids USING(asset_id) GROUP BY source.asset_id
             )
             SELECT asset.asset_id,
                    COALESCE((SELECT title FROM lyric_files
                              WHERE content_hash=asset.content_fingerprint
                              ORDER BY available DESC, updated_at DESC LIMIT 1),
                             (SELECT recording.title
                              FROM recording_lyric_bindings AS binding
                              JOIN recordings AS recording USING(recording_id)
                              WHERE binding.asset_id=asset.asset_id
                              ORDER BY binding.is_default DESC, binding.updated_at DESC LIMIT 1),
                             '未知歌曲'),
                    COALESCE((SELECT artist FROM lyric_files
                              WHERE content_hash=asset.content_fingerprint
                              ORDER BY available DESC, updated_at DESC LIMIT 1),
                             (SELECT COALESCE(group_concat(raw_name, ' / '), '')
                              FROM recording_artist_credits
                              WHERE recording_id=(
                                SELECT recording_id FROM recording_lyric_bindings
                                WHERE asset_id=asset.asset_id
                                ORDER BY is_default DESC, updated_at DESC LIMIT 1
                              )), '未知歌手'),
                    asset.source_name, asset.source_kind, asset.original_format, asset.language,
                    asset.has_word_timing, asset.has_translation, asset.has_romanization,
                    asset.available, COALESCE(binding.default_count, 0),
                    COALESCE(overrides.override_count, 0),
                    COALESCE(binding.binding_count, 0),
                    MAX(COALESCE(sources.source_count, 0),
                        CASE WHEN asset.relative_path IS NULL THEN 0 ELSE 1 END),
                    COALESCE((SELECT SUM(file_size) FROM lyric_files
                              WHERE content_hash=asset.content_fingerprint AND available=1),
                             (SELECT file_size FROM lyric_files
                              WHERE content_hash=asset.content_fingerprint
                              ORDER BY available DESC, updated_at DESC LIMIT 1), 0),
                    asset.content_fingerprint
             FROM page_ids
             JOIN lyric_assets AS asset USING(asset_id)
             LEFT JOIN binding_stats AS binding USING(asset_id)
             LEFT JOIN override_stats AS overrides USING(asset_id)
             LEFT JOIN source_stats AS sources USING(asset_id)
             ORDER BY page_ids.ordinal",
        )
        .map_err(|error| format!("准备歌词资料库集合摘要失败：{error}"))?;
    let summaries = statement
        .query_map(params![ids_json], |row| {
            let default_count = row.get::<_, i64>(11)?.max(0);
            let override_count = row.get::<_, i64>(12)?.max(0);
            let binding_count = row.get::<_, i64>(13)?.max(0);
            let active_count = default_count + override_count;
            Ok(LibraryLyricSummary {
                asset_id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                source_name: row.get(3)?,
                source_kind: row.get(4)?,
                original_format: row.get(5)?,
                language: row.get(6)?,
                has_word_timing: row.get::<_, i64>(7)? != 0,
                has_translation: row.get::<_, i64>(8)? != 0,
                has_romanization: row.get::<_, i64>(9)? != 0,
                available: row.get::<_, i64>(10)? != 0,
                active_count: active_count as u64,
                binding_count: binding_count as u64,
                source_count: row.get::<_, i64>(14)?.max(0) as u64,
                file_size: row.get::<_, i64>(15)?.max(0) as u64,
                status: if active_count > 0 {
                    "inUse"
                } else if binding_count > 0 {
                    "candidate"
                } else {
                    "unbound"
                }
                .into(),
                content_fingerprint: row.get(16)?,
            })
        })
        .map_err(|error| format!("读取歌词资料库集合摘要失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析歌词资料库集合摘要失败：{error}"))?;
    Ok(summaries)
}

pub(super) fn library_lyric_sources(
    connection: &Connection,
    asset: &LyricAsset,
) -> Result<Vec<LibraryLyricSource>, String> {
    let mut statement = connection
        .prepare(
            "SELECT source.source_id, source.source_kind, source.source_name,
                    source.root_dir_id, root.display_name, source.relative_path,
                    source.available, COALESCE(root.read_only, 1), root.path, root.root_kind
             FROM lyric_asset_sources AS source
             LEFT JOIN library_roots AS root ON root.root_id=source.root_dir_id
             WHERE source.asset_id=?1 ORDER BY source.available DESC, source.source_id",
        )
        .map_err(|error| format!("准备歌词来源查询失败：{error}"))?;
    let sources = statement
        .query_map(params![asset.asset_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)? != 0,
                row.get::<_, i64>(7)? != 0,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        })
        .map_err(|error| format!("读取歌词来源失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析歌词来源失败：{error}"))?;
    let mut result = Vec::new();
    for (
        source_id,
        source_kind,
        source_name,
        root_id,
        root_name,
        relative_path,
        available,
        read_only,
        root_path,
        root_kind,
    ) in sources
    {
        let path = source_absolute_path(
            root_id.as_deref(),
            root_path.as_deref(),
            relative_path.as_deref(),
        );
        let file_size = path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        let writable = !read_only
            && matches!(source_kind.as_str(), "managed" | "cache")
            && root_kind
                .as_deref()
                .is_some_and(|kind| matches!(kind, "managed" | "cache"));
        result.push(LibraryLyricSource {
            source_id: Some(source_id),
            source_kind,
            source_name,
            root_id,
            root_name,
            relative_path,
            available,
            writable,
            file_size,
        });
    }
    if result.is_empty() {
        let root = asset.root_id.as_deref().and_then(|root_id| {
            connection.query_row(
                "SELECT display_name, path, read_only, root_kind FROM library_roots WHERE root_id=?1",
                params![root_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)? != 0, row.get::<_, String>(3)?)),
            ).optional().ok().flatten()
        });
        let path = source_absolute_path(
            asset.root_id.as_deref(),
            root.as_ref().map(|item| item.1.as_str()),
            asset.relative_path.as_deref(),
        );
        result.push(LibraryLyricSource {
            source_id: None,
            source_kind: asset.source_kind.clone(),
            source_name: asset.source_name.clone(),
            root_id: asset.root_id.clone(),
            root_name: root.as_ref().map(|item| item.0.clone()),
            relative_path: asset.relative_path.clone(),
            available: asset.available,
            writable: root
                .as_ref()
                .is_some_and(|item| !item.2 && matches!(item.3.as_str(), "managed" | "cache"))
                && matches!(asset.source_kind.as_str(), "managed" | "cache"),
            file_size: path
                .as_deref()
                .and_then(|value| std::fs::metadata(value).ok())
                .map(|value| value.len())
                .unwrap_or_default(),
        });
    }
    Ok(result)
}

pub(super) fn source_absolute_path(
    root_id: Option<&str>,
    root_path: Option<&str>,
    relative_path: Option<&str>,
) -> Option<PathBuf> {
    let relative = relative_path?;
    if root_id == Some(EXTERNAL_ROOT_ID) {
        return Some(PathBuf::from(relative));
    }
    let root = PathBuf::from(root_path?);
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative.components().any(|component| {
            !matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return None;
    }
    let path = root.join(relative);
    path.starts_with(&root).then_some(path)
}

fn library_lyric_recordings(
    connection: &Connection,
    asset_id: i64,
) -> Result<Vec<LibraryLyricRecording>, String> {
    let mut statement = connection
        .prepare(
            "SELECT binding.recording_id, recording.title, binding.is_default, binding.offset_ms
         FROM recording_lyric_bindings AS binding
         JOIN recordings AS recording ON recording.recording_id=binding.recording_id
         WHERE binding.asset_id=?1 ORDER BY binding.is_default DESC, recording.title",
        )
        .map_err(|error| format!("准备歌词歌曲关系查询失败：{error}"))?;
    let rows = statement
        .query_map(params![asset_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|error| format!("读取歌词歌曲关系失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析歌词歌曲关系失败：{error}"))?;
    rows.into_iter()
        .map(|(recording_id, title, is_default, offset_ms)| {
            let artists = load_artist_credits(connection, recording_id)?
                .into_iter()
                .map(|credit| credit.canonical_name)
                .collect();
            Ok(LibraryLyricRecording {
                recording_id,
                title,
                artists,
                is_default,
                offset_ms,
            })
        })
        .collect()
}
