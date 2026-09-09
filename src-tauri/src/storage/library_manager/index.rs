use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use super::super::{
    load_artist_credits, load_observations, normalized_identity_artist, recording_view, Storage,
};
use super::artists::{all_library_artist_summaries, refresh_artist_projection_if_dirty};
use super::lyrics::library_lyric_summary;
use super::models::{
    LibraryArtistSummary, LibraryIndexEntry, LibraryIndexStatus, LibraryLyricStatusCounts,
    LibraryLyricSummary, LibrarySongSummary,
};
use super::pagination::library_page_parameters;
use super::songs::library_song_summary;

static LIBRARY_SEARCH_INDEX_LOCK: Mutex<()> = Mutex::new(());

// 资料库搜索统一折叠繁简、大小写、空格和标点，但不改变保存或展示的原文。
pub(super) fn normalized_library_search_value(value: &str) -> String {
    normalized_identity_artist(value)
}

fn push_library_search_field(fields: &mut Vec<(u8, String)>, priority: u8, value: &str) {
    let value = normalized_library_search_value(value);
    if !value.is_empty() {
        fields.push((priority, value));
    }
}

fn library_song_search_fields(
    connection: &Connection,
    item: &LibrarySongSummary,
) -> Result<Vec<(u8, String)>, String> {
    let mut fields = Vec::new();
    push_library_search_field(&mut fields, 4, &item.title);
    for artist in &item.artists {
        push_library_search_field(&mut fields, 3, artist);
    }
    if let Some(album) = item.album.as_deref() {
        push_library_search_field(&mut fields, 3, album);
    }

    for observation in load_observations(connection, item.recording_id)? {
        push_library_search_field(&mut fields, 2, &observation.raw_title);
        for artist in observation.raw_artists {
            push_library_search_field(&mut fields, 1, &artist);
        }
        if let Some(album) = observation.raw_album.as_deref() {
            push_library_search_field(&mut fields, 1, album);
        }
    }
    for credit in load_artist_credits(connection, item.recording_id)? {
        push_library_search_field(&mut fields, 1, &credit.raw_name);
        for alias in credit.confirmed_aliases {
            push_library_search_field(&mut fields, 1, &alias);
        }
    }
    Ok(fields)
}

fn library_lyric_search_fields(
    connection: &Connection,
    item: &LibraryLyricSummary,
) -> Result<Vec<(u8, String)>, String> {
    let mut fields = Vec::new();
    push_library_search_field(&mut fields, 6, &item.title);
    push_library_search_field(&mut fields, 5, &item.artist);
    push_library_search_field(&mut fields, 4, &item.source_name);

    let mut statement = connection
        .prepare(
            "SELECT recording_id FROM recording_lyric_bindings WHERE asset_id=?1
             UNION
             SELECT recording_id FROM platform_lyric_overrides WHERE asset_id=?1",
        )
        .map_err(|error| format!("准备歌词关联搜索查询失败：{error}"))?;
    let recording_ids = statement
        .query_map(params![item.asset_id], |row| row.get::<_, i64>(0))
        .map_err(|error| format!("读取歌词关联搜索数据失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析歌词关联搜索数据失败：{error}"))?;

    for recording_id in recording_ids {
        let recording = recording_view(connection, recording_id)?;
        push_library_search_field(&mut fields, 3, &recording.title);
        if let Some(album) = recording.album.as_deref() {
            push_library_search_field(&mut fields, 3, album);
        }
        for credit in load_artist_credits(connection, recording_id)? {
            push_library_search_field(&mut fields, 3, &credit.canonical_name);
            push_library_search_field(&mut fields, 2, &credit.raw_name);
            for alias in credit.confirmed_aliases {
                push_library_search_field(&mut fields, 2, &alias);
            }
        }
        for observation in load_observations(connection, recording_id)? {
            push_library_search_field(&mut fields, 2, &observation.raw_title);
            for artist in observation.raw_artists {
                push_library_search_field(&mut fields, 2, &artist);
            }
            if let Some(album) = observation.raw_album.as_deref() {
                push_library_search_field(&mut fields, 2, album);
            }
        }
    }
    Ok(fields)
}

fn library_artist_search_fields(item: &LibraryArtistSummary) -> Vec<(u8, String)> {
    let mut fields = Vec::new();
    push_library_search_field(&mut fields, 3, &item.canonical_name);
    for alias in &item.aliases {
        push_library_search_field(&mut fields, 2, alias);
    }
    for raw_name in &item.raw_names {
        push_library_search_field(&mut fields, 1, raw_name);
    }
    fields
}

pub(super) fn search_trigrams(value: &str) -> Vec<String> {
    let characters = value.chars().collect::<Vec<_>>();
    if characters.len() < 3 {
        return Vec::new();
    }
    let mut grams = characters
        .windows(3)
        .map(|window| window.iter().collect::<String>())
        .collect::<Vec<_>>();
    grams.sort();
    grams.dedup();
    grams
}

fn replace_library_search_terms(
    connection: &Connection,
    entity_kind: &str,
    entity_id: i64,
    fields: Vec<(u8, String)>,
    generation: i64,
) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM library_search_grams WHERE term_id IN (
               SELECT term_id FROM library_search_terms WHERE entity_kind=?1 AND entity_id=?2
             )",
            params![entity_kind, entity_id],
        )
        .map_err(|error| format!("清理资料库搜索分词失败：{error}"))?;
    connection
        .execute(
            "DELETE FROM library_search_terms WHERE entity_kind=?1 AND entity_id=?2",
            params![entity_kind, entity_id],
        )
        .map_err(|error| format!("清理资料库搜索索引失败：{error}"))?;
    let mut fields = fields;
    fields.sort();
    fields.dedup();
    for (priority, normalized_value) in fields {
        connection
            .execute(
                "INSERT OR IGNORE INTO library_search_terms
                   (entity_kind, entity_id, field_priority, normalized_value)
                 VALUES (?1, ?2, ?3, ?4)",
                params![entity_kind, entity_id, priority, normalized_value],
            )
            .map_err(|error| format!("更新资料库搜索索引失败：{error}"))?;
        let term_id = connection
            .query_row(
                "SELECT term_id FROM library_search_terms
                 WHERE entity_kind=?1 AND entity_id=?2
                   AND field_priority=?3 AND normalized_value=?4",
                params![entity_kind, entity_id, priority, normalized_value],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("读取资料库搜索词条失败：{error}"))?;
        for gram in search_trigrams(&normalized_value) {
            connection
                .execute(
                    "INSERT OR IGNORE INTO library_search_grams(term_id, gram) VALUES (?1, ?2)",
                    params![term_id, gram],
                )
                .map_err(|error| format!("更新资料库搜索分词失败：{error}"))?;
        }
    }
    connection
        .execute(
            "INSERT INTO library_search_state(entity_kind, entity_id, generation)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(entity_kind, entity_id) DO UPDATE SET generation=excluded.generation",
            params![entity_kind, entity_id, generation],
        )
        .map_err(|error| format!("更新资料库搜索状态失败：{error}"))?;
    Ok(())
}

fn commit_library_search_terms_if_current(
    connection: &mut Connection,
    entity_kind: &str,
    entity_id: i64,
    fields: Vec<(u8, String)>,
    generation: i64,
) -> Result<bool, String> {
    let transaction = connection
        .transaction()
        .map_err(|error| format!("开始提交资料库搜索索引失败：{error}"))?;
    let current_generation = transaction
        .query_row(
            "SELECT generation FROM library_index_dirty
             WHERE index_kind=?1 AND entity_id=?2",
            params![entity_kind, entity_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("校验资料库搜索索引失败：{error}"))?;
    if current_generation != Some(generation) {
        return Ok(false);
    }
    replace_library_search_terms(&transaction, entity_kind, entity_id, fields, generation)?;
    transaction
        .commit()
        .map_err(|error| format!("提交资料库搜索索引失败：{error}"))?;
    Ok(true)
}

fn pending_search_entities(
    connection: &Connection,
    entity_kind: &str,
) -> Result<Vec<(i64, i64)>, String> {
    let mut statement = connection
        .prepare(
            "SELECT dirty.entity_id, dirty.generation
             FROM library_index_dirty AS dirty
             LEFT JOIN library_search_state AS state
               ON state.entity_kind=dirty.index_kind AND state.entity_id=dirty.entity_id
             WHERE dirty.index_kind=?1
               AND (state.generation IS NULL OR state.generation!=dirty.generation)
             ORDER BY dirty.queued_at, dirty.entity_id",
        )
        .map_err(|error| format!("准备资料库搜索增量索引失败：{error}"))?;
    let pending = statement
        .query_map(params![entity_kind], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("读取资料库搜索增量索引失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析资料库搜索增量索引失败：{error}"))?;
    Ok(pending)
}

pub(super) fn pending_index_generations(
    connection: &Connection,
    index_kind: &str,
) -> Result<Vec<(i64, i64)>, String> {
    let mut statement = connection
        .prepare(
            "SELECT entity_id, generation FROM library_index_dirty
             WHERE index_kind=?1 ORDER BY entity_id",
        )
        .map_err(|error| format!("准备资料库索引水位查询失败：{error}"))?;
    let pending = statement
        .query_map(params![index_kind], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("读取资料库索引水位失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析资料库索引水位失败：{error}"))?;
    Ok(pending)
}

pub(super) fn search_index_page(
    connection: &Connection,
    entity_kind: &str,
    query: &str,
    page: u64,
    page_size: u64,
) -> Result<(Vec<i64>, u64, u64, usize), String> {
    let (page, page_size, offset) = library_page_parameters(page, page_size);
    let base = match entity_kind {
        "song" => (
            "recordings",
            "recording_id",
            "updated_at DESC, recording_id DESC",
        ),
        "lyric" => ("lyric_assets", "asset_id", "updated_at DESC, asset_id DESC"),
        "artist" => (
            "library_artist_projection",
            "artist_id",
            "canonical_name, artist_id",
        ),
        _ => return Err("未知的资料库搜索类型".into()),
    };
    let grams = search_trigrams(query);
    let grams_json = serde_json::to_string(&grams).unwrap_or_else(|_| "[]".into());
    let gram_count = grams.len() as i64;
    let matches = "WITH candidate_terms AS (
       SELECT term_id FROM library_search_terms WHERE ?3=0
       UNION
       SELECT term_id FROM library_search_grams
       WHERE ?3>0 AND gram IN (SELECT value FROM json_each(?4))
       GROUP BY term_id
       HAVING COUNT(DISTINCT gram)=?3
     ), matches AS (
       SELECT entity_id,
              MAX(CASE
                    WHEN normalized_value=?2 THEN 30 + field_priority
                    WHEN substr(normalized_value, 1, length(?2))=?2 THEN 20 + field_priority
                    ELSE 10 + field_priority
                  END) AS rank
       FROM library_search_terms
       JOIN candidate_terms USING(term_id)
       WHERE entity_kind=?1 AND instr(normalized_value, ?2)>0
       GROUP BY entity_id
     )";
    let total_sql = format!("{matches} SELECT COUNT(*) FROM matches");
    let total = connection
        .query_row(
            &total_sql,
            params![entity_kind, query, gram_count, grams_json],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("读取资料库搜索总数失败：{error}"))?
        .max(0) as u64;
    let page_sql = format!(
        "{matches}
         SELECT matched.entity_id FROM matches AS matched
         JOIN {} AS base ON base.{}=matched.entity_id
         ORDER BY matched.rank DESC, base.{} LIMIT ?5 OFFSET ?6",
        base.0, base.1, base.2
    );
    let mut statement = connection
        .prepare(&page_sql)
        .map_err(|error| format!("准备资料库搜索分页失败：{error}"))?;
    let ids = statement
        .query_map(
            params![
                entity_kind,
                query,
                gram_count,
                grams_json,
                page_size as i64,
                offset
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("读取资料库搜索分页失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析资料库搜索分页失败：{error}"))?;
    Ok((ids, total, page, page_size))
}

pub(super) fn search_lyric_index_page(
    connection: &Connection,
    query: &str,
    status: Option<&str>,
    source_kind: Option<&str>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<i64>, u64, u64, usize, LibraryLyricStatusCounts), String> {
    let (page, page_size, offset) = library_page_parameters(page, page_size);
    let grams = search_trigrams(query);
    let grams_json = serde_json::to_string(&grams).unwrap_or_else(|_| "[]".into());
    let gram_count = grams.len() as i64;
    const CTE: &str = "WITH candidate_terms AS (
       SELECT term_id FROM library_search_terms WHERE ?2=0 AND entity_kind='lyric'
       UNION
       SELECT term_id FROM library_search_grams
       WHERE ?2>0 AND gram IN (SELECT value FROM json_each(?3))
       GROUP BY term_id
       HAVING COUNT(DISTINCT gram)=?2
     ), matches AS (
       SELECT entity_id,
              MAX(CASE
                    WHEN normalized_value=?1 THEN 30 + field_priority
                    WHEN substr(normalized_value, 1, length(?1))=?1 THEN 20 + field_priority
                    ELSE 10 + field_priority
                  END) AS rank
       FROM library_search_terms
       JOIN candidate_terms USING(term_id)
       WHERE entity_kind='lyric' AND instr(normalized_value, ?1)>0
       GROUP BY entity_id
     ), binding_stats AS (
       SELECT asset_id, COUNT(*) AS binding_count, COALESCE(SUM(is_default), 0) AS default_count
       FROM recording_lyric_bindings GROUP BY asset_id
     ), override_stats AS (
       SELECT asset_id, COUNT(*) AS override_count FROM platform_lyric_overrides
       WHERE asset_id IS NOT NULL GROUP BY asset_id
     ), filtered AS (
       SELECT asset.asset_id, asset.source_kind, asset.updated_at, matches.rank,
              CASE
                WHEN COALESCE(binding.default_count, 0) + COALESCE(overrides.override_count, 0) > 0 THEN 'inUse'
                WHEN COALESCE(binding.binding_count, 0) > 0 THEN 'candidate'
                ELSE 'unbound'
              END AS status
       FROM matches
       JOIN lyric_assets AS asset ON asset.asset_id=matches.entity_id
       LEFT JOIN binding_stats AS binding ON binding.asset_id=asset.asset_id
       LEFT JOIN override_stats AS overrides ON overrides.asset_id=asset.asset_id
       WHERE (?4 IS NULL OR asset.source_kind=?4)
     ) ";
    let counts_sql = format!(
        "{CTE} SELECT COALESCE(SUM(status='inUse'), 0),
                      COALESCE(SUM(status='candidate'), 0),
                      COALESCE(SUM(status='unbound'), 0) FROM filtered"
    );
    let status_counts = connection
        .query_row(
            &counts_sql,
            params![query, gram_count, grams_json, source_kind],
            |row| {
                Ok(LibraryLyricStatusCounts {
                    in_use: row.get::<_, i64>(0)?.max(0) as u64,
                    candidate: row.get::<_, i64>(1)?.max(0) as u64,
                    unbound: row.get::<_, i64>(2)?.max(0) as u64,
                })
            },
        )
        .map_err(|error| format!("读取歌词搜索状态统计失败：{error}"))?;
    let total = match status {
        Some("inUse") => status_counts.in_use,
        Some("candidate") => status_counts.candidate,
        Some("unbound") => status_counts.unbound,
        _ => status_counts.in_use + status_counts.candidate + status_counts.unbound,
    };
    let page_sql = format!(
        "{CTE} SELECT asset_id FROM filtered WHERE (?5 IS NULL OR status=?5)
         ORDER BY rank DESC, updated_at DESC, asset_id DESC LIMIT ?6 OFFSET ?7"
    );
    let mut statement = connection
        .prepare(&page_sql)
        .map_err(|error| format!("准备歌词搜索分页失败：{error}"))?;
    let ids = statement
        .query_map(
            params![
                query,
                gram_count,
                grams_json,
                source_kind,
                status,
                page_size as i64,
                offset
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("读取歌词搜索分页失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析歌词搜索分页失败：{error}"))?;
    Ok((ids, total, page, page_size, status_counts))
}

impl Storage {
    pub fn library_index_status(&self) -> Result<LibraryIndexStatus, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut statement = connection
            .prepare(
                "SELECT state.index_kind, state.phase, state.processed, state.total,
                        CASE state.index_kind
                          WHEN 'search' THEN (
                            SELECT COUNT(*) FROM library_index_dirty AS dirty
                            LEFT JOIN library_search_state AS indexed
                              ON indexed.entity_kind=dirty.index_kind
                             AND indexed.entity_id=dirty.entity_id
                            WHERE indexed.generation IS NULL
                               OR indexed.generation!=dirty.generation
                          )
                          WHEN 'artist' THEN (
                            SELECT COUNT(*) FROM library_index_dirty AS dirty
                            LEFT JOIN library_artist_projection_state AS indexed
                              ON indexed.artist_id=dirty.entity_id
                            WHERE dirty.index_kind='artist'
                              AND (indexed.generation IS NULL
                                   OR indexed.generation!=dirty.generation)
                          )
                          WHEN 'song_similarity' THEN (
                            SELECT COUNT(*) FROM library_index_dirty AS dirty
                            LEFT JOIN library_similarity_state AS indexed
                              ON indexed.index_kind='song_similarity'
                             AND indexed.entity_id=dirty.entity_id
                            WHERE dirty.index_kind='song'
                              AND (indexed.generation IS NULL
                                   OR indexed.generation!=dirty.generation)
                          )
                          WHEN 'lyric_similarity' THEN (
                            SELECT COUNT(*) FROM library_index_dirty AS dirty
                            LEFT JOIN library_similarity_state AS indexed
                              ON indexed.index_kind='lyric_similarity'
                             AND indexed.entity_id=dirty.entity_id
                            WHERE dirty.index_kind='lyric'
                              AND (indexed.generation IS NULL
                                   OR indexed.generation!=dirty.generation)
                          )
                          ELSE 0
                        END,
                        state.revision, state.last_error
                 FROM library_index_state AS state
                 ORDER BY CASE state.index_kind
                   WHEN 'search' THEN 1 WHEN 'artist' THEN 2
                   WHEN 'song_similarity' THEN 3 ELSE 4 END",
            )
            .map_err(|error| format!("准备资料库索引状态查询失败：{error}"))?;
        let indexes = statement
            .query_map([], |row| {
                Ok(LibraryIndexEntry {
                    index_kind: row.get(0)?,
                    phase: row.get(1)?,
                    processed: row.get::<_, i64>(2)?.max(0) as u64,
                    total: row.get::<_, i64>(3)?.max(0) as u64,
                    pending: row.get::<_, i64>(4)?.max(0) as u64,
                    revision: row.get::<_, i64>(5)?.max(0) as u64,
                    error: row.get(6)?,
                })
            })
            .map_err(|error| format!("读取资料库索引状态失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析资料库索引状态失败：{error}"))?;
        Ok(LibraryIndexStatus { indexes })
    }

    pub fn rebuild_library_search_index(&self) -> Result<(), String> {
        let _index_guard = LIBRARY_SEARCH_INDEX_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut connection = Connection::open(&self.database_path)
            .map_err(|error| format!("打开资料库搜索索引失败：{error}"))?;
        connection
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(|error| format!("初始化资料库搜索索引失败：{error}"))?;
        let started = std::time::Instant::now();
        let song_pending = pending_search_entities(&connection, "song")?;
        let lyric_pending = pending_search_entities(&connection, "lyric")?;
        let artist_pending = pending_search_entities(&connection, "artist")?;
        let total = song_pending.len() + lyric_pending.len() + artist_pending.len();
        if total == 0 {
            return Ok(());
        }
        connection
            .execute(
                "UPDATE library_index_state SET phase='building', processed=0, total=?2,
                        last_error=NULL WHERE index_kind=?1",
                params!["search", total as i64],
            )
            .map_err(|error| format!("更新资料库搜索索引状态失败：{error}"))?;
        let mut processed = 0_i64;
        for (recording_id, generation) in song_pending {
            let fields = if connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM recordings WHERE recording_id=?1)",
                    params![recording_id],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap_or(false)
            {
                let summary = library_song_summary(&connection, recording_id)?;
                library_song_search_fields(&connection, &summary)?
            } else {
                Vec::new()
            };
            if commit_library_search_terms_if_current(
                &mut connection,
                "song",
                recording_id,
                fields,
                generation,
            )? {
                processed += 1;
                if processed % 32 == 0 {
                    connection
                        .execute(
                            "UPDATE library_index_state SET processed=?2
                             WHERE index_kind=?1",
                            params!["search", processed],
                        )
                        .map_err(|error| format!("报告资料库搜索索引进度失败：{error}"))?;
                }
            }
        }
        for (asset_id, generation) in lyric_pending {
            let fields = match library_lyric_summary(&connection, asset_id)? {
                Some(summary) => library_lyric_search_fields(&connection, &summary)?,
                None => Vec::new(),
            };
            if commit_library_search_terms_if_current(
                &mut connection,
                "lyric",
                asset_id,
                fields,
                generation,
            )? {
                processed += 1;
                if processed % 32 == 0 {
                    connection
                        .execute(
                            "UPDATE library_index_state SET processed=?2
                             WHERE index_kind=?1",
                            params!["search", processed],
                        )
                        .map_err(|error| format!("报告资料库搜索索引进度失败：{error}"))?;
                }
            }
        }
        if !artist_pending.is_empty() {
            refresh_artist_projection_if_dirty(&connection)?;
            connection
                .execute(
                    "DELETE FROM library_search_grams WHERE term_id IN (
                       SELECT term_id FROM library_search_terms WHERE entity_kind='artist'
                     )",
                    [],
                )
                .map_err(|error| format!("清理歌手搜索分词失败：{error}"))?;
            connection
                .execute(
                    "DELETE FROM library_search_terms WHERE entity_kind='artist'",
                    [],
                )
                .map_err(|error| format!("清理歌手搜索索引失败：{error}"))?;
            let (summaries, _) = all_library_artist_summaries(&connection)?;
            for summary in summaries {
                let fields = library_artist_search_fields(&summary);
                replace_library_search_terms(&connection, "artist", summary.artist_id, fields, 1)?;
            }
            for (artist_id, generation) in artist_pending {
                connection
                    .execute(
                        "INSERT INTO library_search_state(entity_kind, entity_id, generation)
                         VALUES ('artist', ?1, ?2)
                         ON CONFLICT(entity_kind, entity_id) DO UPDATE SET generation=excluded.generation",
                        params![artist_id, generation],
                    )
                    .map_err(|error| format!("更新歌手搜索状态失败：{error}"))?;
                processed += 1;
            }
        }
        connection
            .execute(
                "UPDATE library_index_state SET phase='ready', processed=?2, total=?2,
                        revision=revision+1, last_error=NULL WHERE index_kind=?1",
                params!["search", processed],
            )
            .map_err(|error| format!("完成资料库搜索索引失败：{error}"))?;
        log::debug!(
            "资料库搜索索引完成：entities={processed} elapsed_ms={}",
            started.elapsed().as_millis()
        );
        Ok(())
    }
}
