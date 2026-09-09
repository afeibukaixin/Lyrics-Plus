use rusqlite::{params, Connection, OptionalExtension};

use super::super::{
    load_artist_credits, load_asset, load_bindings, load_external_identifiers, load_observations,
    recording_view, PlatformLyricOverride, RecordingIdentity, Storage,
};
use super::index::{normalized_library_search_value, search_index_page};
use super::models::{LibraryPage, LibrarySongDetail, LibrarySongSource, LibrarySongSummary};
use super::pagination::library_page_parameters;

impl Storage {
    pub fn list_library_songs(
        &self,
        query: &str,
        page: u64,
        page_size: u64,
    ) -> Result<LibraryPage<LibrarySongSummary>, String> {
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
            let total = connection
                .query_row(
                    "SELECT COUNT(*) FROM recordings AS recording
                     WHERE EXISTS (SELECT 1 FROM track_observations AS observation
                                   WHERE observation.recording_id=recording.recording_id)",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|error| format!("读取歌曲资料库总数失败：{error}"))?
                .max(0) as u64;
            let mut statement = connection
                .prepare(
                    "SELECT recording_id FROM recordings AS recording
                     WHERE EXISTS (SELECT 1 FROM track_observations AS observation
                                   WHERE observation.recording_id=recording.recording_id)
                     ORDER BY updated_at DESC, recording_id DESC LIMIT ?1 OFFSET ?2",
                )
                .map_err(|error| format!("准备歌曲资料库分页查询失败：{error}"))?;
            let ids = statement
                .query_map(params![page_size as i64, offset], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(|error| format!("读取歌曲资料库失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析歌曲资料库失败：{error}"))?;
            let items = library_song_summaries(&connection, &ids)?;
            return Ok(LibraryPage {
                items,
                total,
                page,
                page_size: page_size as u64,
            });
        }
        let (ids, total, page, page_size) =
            search_index_page(&connection, "song", &query, page, page_size)?;
        let items = library_song_summaries(&connection, &ids)?;
        Ok(LibraryPage {
            items,
            total,
            page,
            page_size: page_size as u64,
        })
    }

    pub fn library_song_detail(&self, recording_id: i64) -> Result<LibrarySongDetail, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let view = recording_view(&connection, recording_id)?;
        let artist_credits = load_artist_credits(&connection, recording_id)?;
        let external_identifiers = load_external_identifiers(&connection, recording_id)?;
        let observations = load_observations(&connection, recording_id)?;
        let bindings = load_bindings(&connection, recording_id)?;
        let platform_overrides = load_all_platform_overrides(&connection, recording_id)?;
        Ok(LibrarySongDetail {
            recording: RecordingIdentity {
                recording_id,
                title: view.title,
                album: view.album,
                duration_ms: view.duration_ms,
                version_tags: view.version_tags,
                artist_credits,
                external_identifiers,
                observations,
            },
            bindings,
            platform_overrides,
        })
    }
}

pub(super) fn library_song_summary(
    connection: &Connection,
    recording_id: i64,
) -> Result<LibrarySongSummary, String> {
    let view = recording_view(connection, recording_id)?;
    let credits = load_artist_credits(connection, recording_id)?;
    let mut source_map = library_song_sources(connection, &[recording_id])?;
    let sources = source_map.remove(&recording_id).unwrap_or_default();
    let lyric_count = connection
        .query_row(
            "SELECT COUNT(*) FROM recording_lyric_bindings WHERE recording_id=?1",
            params![recording_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("读取歌曲歌词数量失败：{error}"))?
        .max(0) as u64;
    let default_lyric = connection
        .query_row(
            "SELECT asset.source_name FROM recording_lyric_bindings AS binding
             JOIN lyric_assets AS asset ON asset.asset_id=binding.asset_id
             WHERE binding.recording_id=?1 AND binding.is_default=1 LIMIT 1",
            params![recording_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("读取歌曲默认歌词失败：{error}"))?;
    Ok(LibrarySongSummary {
        recording_id,
        title: view.title,
        artists: credits
            .into_iter()
            .map(|credit| credit.canonical_name)
            .collect(),
        album: view.album,
        duration_ms: view.duration_ms,
        version_tags: view.version_tags,
        source_count: sources.len() as u64,
        sources,
        lyric_count,
        default_lyric,
    })
}

pub(super) fn library_song_summaries(
    connection: &Connection,
    recording_ids: &[i64],
) -> Result<Vec<LibrarySongSummary>, String> {
    if recording_ids.is_empty() {
        return Ok(Vec::new());
    }
    let ids_json = serde_json::to_string(recording_ids)
        .map_err(|error| format!("编码歌曲资料库分页失败：{error}"))?;
    let mut statement = connection
        .prepare(
            "WITH page_ids AS (
               SELECT CAST(value AS INTEGER) AS recording_id, key AS ordinal
               FROM json_each(?1)
             )
             SELECT recording.recording_id, recording.title, recording.album,
                    recording.duration_ms, recording.version_tags_json,
                    COALESCE((
                      SELECT json_group_array(canonical_name) FROM (
                        SELECT artist.canonical_name
                        FROM recording_artist_credits AS credit
                        JOIN artists AS artist ON artist.artist_id=credit.artist_id
                        WHERE credit.recording_id=recording.recording_id
                        ORDER BY credit.credit_order
                      )
                    ), '[]'),
                    (SELECT COUNT(*) FROM recording_lyric_bindings
                     WHERE recording_id=recording.recording_id),
                    (SELECT asset.source_name
                     FROM recording_lyric_bindings AS binding
                     JOIN lyric_assets AS asset ON asset.asset_id=binding.asset_id
                     WHERE binding.recording_id=recording.recording_id
                       AND binding.is_default=1 LIMIT 1)
             FROM page_ids
             JOIN recordings AS recording USING(recording_id)
             ORDER BY page_ids.ordinal",
        )
        .map_err(|error| format!("准备歌曲资料库集合摘要失败：{error}"))?;
    let mut summaries = statement
        .query_map(params![ids_json], |row| {
            Ok(LibrarySongSummary {
                recording_id: row.get(0)?,
                title: row.get(1)?,
                album: row.get(2)?,
                duration_ms: row
                    .get::<_, Option<i64>>(3)?
                    .and_then(|value| u64::try_from(value).ok()),
                version_tags: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                artists: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                source_count: 0,
                sources: Vec::new(),
                lyric_count: row.get::<_, i64>(6)?.max(0) as u64,
                default_lyric: row.get(7)?,
            })
        })
        .map_err(|error| format!("读取歌曲资料库集合摘要失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析歌曲资料库集合摘要失败：{error}"))?;
    let mut source_map = library_song_sources(connection, recording_ids)?;
    for summary in &mut summaries {
        summary.sources = source_map.remove(&summary.recording_id).unwrap_or_default();
        summary.source_count = summary.sources.len() as u64;
    }
    Ok(summaries)
}

fn library_song_sources(
    connection: &Connection,
    recording_ids: &[i64],
) -> Result<std::collections::HashMap<i64, Vec<LibrarySongSource>>, String> {
    if recording_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let ids_json = serde_json::to_string(recording_ids)
        .map_err(|error| format!("编码歌曲来源集合失败：{error}"))?;
    let mut statement = connection
        .prepare(
            "SELECT recording_id, platform, source_app_bundle_id, source_app_name
             FROM track_observations
             WHERE recording_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?1))
             ORDER BY observed_at DESC, observation_id DESC",
        )
        .map_err(|error| format!("准备歌曲来源查询失败：{error}"))?;
    let rows = statement
        .query_map(params![ids_json], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|error| format!("读取歌曲来源失败：{error}"))?;
    let mut grouped = std::collections::HashMap::<
        i64,
        std::collections::BTreeMap<String, LibrarySongSource>,
    >::new();
    for row in rows {
        let (recording_id, platform, bundle_id, app_name) =
            row.map_err(|error| format!("解析歌曲来源失败：{error}"))?;
        let platform_key = normalized_source_component(&platform);
        let (key, source_app_bundle_id, source_app_name) =
            if platform.eq_ignore_ascii_case("system") {
                let bundle_id = bundle_id.filter(|value| !value.trim().is_empty());
                let app_name = app_name.filter(|value| !value.trim().is_empty());
                let key = if let Some(value) = bundle_id.as_deref() {
                    format!("system:bundle:{}", normalized_source_component(value))
                } else if let Some(value) = app_name.as_deref() {
                    format!("system:name:{}", normalized_source_component(value))
                } else {
                    "system:unknown".to_owned()
                };
                (key, bundle_id, app_name)
            } else {
                (format!("platform:{platform_key}"), None, None)
            };
        let sources = grouped.entry(recording_id).or_default();
        sources
            .entry(key)
            .and_modify(|source| {
                source.observation_count += 1;
                if source.source_app_bundle_id.is_none() {
                    source.source_app_bundle_id = source_app_bundle_id.clone();
                }
                if source.source_app_name.is_none() {
                    source.source_app_name = source_app_name.clone();
                }
            })
            .or_insert(LibrarySongSource {
                platform,
                source_app_bundle_id,
                source_app_name,
                observation_count: 1,
            });
    }
    Ok(grouped
        .into_iter()
        .map(|(recording_id, sources)| (recording_id, sources.into_values().collect()))
        .collect())
}

fn normalized_source_component(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn load_all_platform_overrides(
    connection: &Connection,
    recording_id: i64,
) -> Result<Vec<PlatformLyricOverride>, String> {
    let mut statement = connection
        .prepare(
            "SELECT override_id, recording_id, platform, asset_id, offset_ms
             FROM platform_lyric_overrides WHERE recording_id=?1 ORDER BY platform",
        )
        .map_err(|error| format!("准备平台歌词覆盖查询失败：{error}"))?;
    let rows = statement
        .query_map(params![recording_id], |row| {
            Ok(PlatformLyricOverride {
                override_id: row.get(0)?,
                recording_id: row.get(1)?,
                platform: row.get(2)?,
                asset_id: row.get(3)?,
                offset_ms: row.get(4)?,
                asset: None,
            })
        })
        .map_err(|error| format!("读取平台歌词覆盖失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析平台歌词覆盖失败：{error}"))?;
    rows.into_iter()
        .map(|mut item| {
            item.asset = item
                .asset_id
                .and_then(|id| load_asset(connection, id).ok().flatten());
            Ok(item)
        })
        .collect()
}
