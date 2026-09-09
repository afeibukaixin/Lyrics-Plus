use std::collections::HashSet;

use rusqlite::{params, Connection};

use super::super::{
    artist_alias_ids_for_removal, confirmed_artist_alias_groups, equivalent_artist_names,
    normalized_artist_alias, normalized_identity_artist, Storage,
};
use super::index::{normalized_library_search_value, search_index_page};
use super::models::{LibraryArtistDetail, LibraryArtistSummary, LibraryPage};
use super::pagination::library_page_parameters;
use super::songs::library_song_summaries;

#[derive(Clone)]
struct LibraryArtistRow {
    artist_id: i64,
    canonical_name: String,
}

fn all_artist_rows(connection: &Connection) -> Result<Vec<LibraryArtistRow>, String> {
    let mut statement = connection
        .prepare("SELECT artist_id, canonical_name FROM artists ORDER BY artist_id")
        .map_err(|error| format!("准备歌手资料库查询失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(LibraryArtistRow {
                artist_id: row.get(0)?,
                canonical_name: row.get(1)?,
            })
        })
        .map_err(|error| format!("读取歌手资料库失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析歌手资料库失败：{error}"))?;
    Ok(rows)
}

fn artist_group_member_ids(
    connection: &Connection,
    rows: &[LibraryArtistRow],
    artist_id: i64,
) -> Result<(i64, Vec<i64>, Vec<String>), String> {
    let seed = rows
        .iter()
        .find(|row| row.artist_id == artist_id)
        .ok_or_else(|| "歌手不存在".to_string())?;
    let mut names =
        equivalent_artist_names(connection, std::slice::from_ref(&seed.canonical_name))?;
    names.push(seed.canonical_name.clone());
    let keys = names
        .iter()
        .map(|name| normalized_identity_artist(name))
        .collect::<HashSet<_>>();
    let mut member_ids = rows
        .iter()
        .filter(|row| keys.contains(&normalized_identity_artist(&row.canonical_name)))
        .map(|row| row.artist_id)
        .collect::<Vec<_>>();
    member_ids.sort_unstable();
    member_ids.dedup();
    names.sort_by_key(|name| normalized_artist_alias(name));
    names.dedup_by(|left, right| normalized_artist_alias(left) == normalized_artist_alias(right));
    let anchor = member_ids.first().copied().unwrap_or(artist_id);
    Ok((anchor, member_ids, names))
}

fn library_artist_summary(
    connection: &Connection,
    rows: &[LibraryArtistRow],
    artist_id: i64,
) -> Result<LibraryArtistSummary, String> {
    let (anchor, member_ids, names) = artist_group_member_ids(connection, rows, artist_id)?;
    let canonical_name = rows
        .iter()
        .find(|row| row.artist_id == anchor)
        .map(|row| row.canonical_name.clone())
        .ok_or_else(|| "歌手不存在".to_string())?;
    let mut aliases = names
        .into_iter()
        .filter(|name| normalized_artist_alias(name) != normalized_artist_alias(&canonical_name))
        .collect::<Vec<_>>();
    let mut song_ids = HashSet::new();
    let mut raw_names = Vec::new();
    for member_id in &member_ids {
        let mut statement = connection
            .prepare(
                "SELECT recording_id, raw_name FROM recording_artist_credits WHERE artist_id=?1",
            )
            .map_err(|error| format!("准备歌手歌曲查询失败：{error}"))?;
        let values = statement
            .query_map(params![member_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("读取歌手歌曲失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析歌手歌曲失败：{error}"))?;
        for (recording_id, raw_name) in values {
            song_ids.insert(recording_id);
            raw_names.push(raw_name);
        }
    }
    raw_names.sort_by_key(|name| normalized_artist_alias(name));
    raw_names
        .dedup_by(|left, right| normalized_artist_alias(left) == normalized_artist_alias(right));
    aliases.sort_by_key(|name| normalized_artist_alias(name));
    aliases.dedup_by(|left, right| normalized_artist_alias(left) == normalized_artist_alias(right));
    Ok(LibraryArtistSummary {
        artist_id: anchor,
        canonical_name,
        aliases,
        song_count: song_ids.len() as u64,
        raw_names,
    })
}

pub(super) fn all_library_artist_summaries(
    connection: &Connection,
) -> Result<
    (
        Vec<LibraryArtistSummary>,
        std::collections::HashMap<i64, i64>,
    ),
    String,
> {
    let rows = all_artist_rows(connection)?;
    let mut ids_by_name = std::collections::HashMap::<String, Vec<i64>>::new();
    for row in &rows {
        ids_by_name
            .entry(normalized_identity_artist(&row.canonical_name))
            .or_default()
            .push(row.artist_id);
    }
    let alias_groups = confirmed_artist_alias_groups(connection)?;
    let mut adjacency = std::collections::HashMap::<i64, Vec<i64>>::new();
    for ids in ids_by_name.values() {
        if let Some(first) = ids.first().copied() {
            for id in ids.iter().copied().skip(1) {
                adjacency.entry(first).or_default().push(id);
                adjacency.entry(id).or_default().push(first);
            }
        }
    }
    for names in &alias_groups {
        let ids = names
            .iter()
            .flat_map(|name| {
                ids_by_name
                    .get(&normalized_identity_artist(name))
                    .into_iter()
                    .flatten()
                    .copied()
            })
            .collect::<Vec<_>>();
        if let Some(first) = ids.first().copied() {
            for id in ids.iter().copied().skip(1) {
                adjacency.entry(first).or_default().push(id);
                adjacency.entry(id).or_default().push(first);
            }
        }
    }
    let mut anchor_by_id = std::collections::HashMap::<i64, i64>::new();
    let mut visited = HashSet::new();
    for row in &rows {
        if !visited.insert(row.artist_id) {
            continue;
        }
        let mut pending = vec![row.artist_id];
        let mut members = Vec::new();
        while let Some(current) = pending.pop() {
            members.push(current);
            for neighbor in adjacency.get(&current).into_iter().flatten().copied() {
                if visited.insert(neighbor) {
                    pending.push(neighbor);
                }
            }
        }
        let anchor = members.iter().copied().min().unwrap_or(row.artist_id);
        for member in members {
            anchor_by_id.insert(member, anchor);
        }
    }
    let mut names_by_anchor = std::collections::HashMap::<i64, Vec<String>>::new();
    for row in &rows {
        let anchor = anchor_by_id[&row.artist_id];
        names_by_anchor
            .entry(anchor)
            .or_default()
            .push(row.canonical_name.clone());
    }
    for names in alias_groups {
        let anchor = names.iter().find_map(|name| {
            ids_by_name
                .get(&normalized_identity_artist(name))
                .and_then(|ids| ids.first())
                .and_then(|id| anchor_by_id.get(id))
                .copied()
        });
        if let Some(anchor) = anchor {
            names_by_anchor.entry(anchor).or_default().extend(names);
        }
    }

    let mut songs_by_anchor = std::collections::HashMap::<i64, HashSet<i64>>::new();
    let mut raw_names_by_anchor = std::collections::HashMap::<i64, Vec<String>>::new();
    let mut statement = connection
        .prepare(
            "SELECT artist_id, recording_id, raw_name FROM recording_artist_credits
             WHERE artist_id IS NOT NULL ORDER BY artist_id, recording_id",
        )
        .map_err(|error| format!("准备歌手聚合查询失败：{error}"))?;
    let credits = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("读取歌手聚合数据失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析歌手聚合数据失败：{error}"))?;
    for (artist_id, recording_id, raw_name) in credits {
        let anchor = anchor_by_id.get(&artist_id).copied().unwrap_or(artist_id);
        songs_by_anchor
            .entry(anchor)
            .or_default()
            .insert(recording_id);
        raw_names_by_anchor
            .entry(anchor)
            .or_default()
            .push(raw_name);
    }

    let row_by_id = rows
        .iter()
        .map(|row| (row.artist_id, row))
        .collect::<std::collections::HashMap<_, _>>();
    let mut anchors = anchor_by_id.values().copied().collect::<Vec<_>>();
    anchors.sort_unstable();
    anchors.dedup();
    let mut summaries = Vec::with_capacity(anchors.len());
    for anchor in anchors {
        let Some(row) = row_by_id.get(&anchor) else {
            continue;
        };
        let canonical_key = normalized_artist_alias(&row.canonical_name);
        let mut aliases = names_by_anchor.remove(&anchor).unwrap_or_default();
        aliases.retain(|name| normalized_artist_alias(name) != canonical_key);
        aliases.sort_by_key(|name| normalized_artist_alias(name));
        aliases.dedup_by(|left, right| {
            normalized_artist_alias(left) == normalized_artist_alias(right)
        });
        let mut raw_names = raw_names_by_anchor.remove(&anchor).unwrap_or_default();
        raw_names.sort_by_key(|name| normalized_artist_alias(name));
        raw_names.dedup_by(|left, right| {
            normalized_artist_alias(left) == normalized_artist_alias(right)
        });
        summaries.push(LibraryArtistSummary {
            artist_id: anchor,
            canonical_name: row.canonical_name.clone(),
            aliases,
            song_count: songs_by_anchor
                .get(&anchor)
                .map_or(0, |ids| ids.len() as u64),
            raw_names,
        });
    }
    Ok((summaries, anchor_by_id))
}

pub(super) fn refresh_artist_projection_if_dirty(connection: &Connection) -> Result<(), String> {
    let dirty = connection
        .query_row(
            "SELECT EXISTS(
                       SELECT 1 FROM library_index_dirty AS dirty
                       LEFT JOIN library_artist_projection_state AS state
                         ON state.artist_id=dirty.entity_id
                       WHERE dirty.index_kind='artist'
                         AND (state.generation IS NULL OR state.generation!=dirty.generation)
                     )
                    OR NOT EXISTS(SELECT 1 FROM library_artist_projection)",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| format!("读取歌手索引状态失败：{error}"))?;
    if !dirty {
        return Ok(());
    }
    let pending = {
        let mut statement = connection
            .prepare(
                "SELECT dirty.entity_id, dirty.generation
                 FROM library_index_dirty AS dirty
                 LEFT JOIN library_artist_projection_state AS state
                   ON state.artist_id=dirty.entity_id
                 WHERE dirty.index_kind='artist'
                   AND (state.generation IS NULL OR state.generation!=dirty.generation)",
            )
            .map_err(|error| format!("准备歌手查询索引失败：{error}"))?;
        let pending = statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .map_err(|error| format!("读取歌手查询索引失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析歌手查询索引失败：{error}"))?;
        pending
    };
    let (summaries, _) = all_library_artist_summaries(connection)?;
    connection
        .execute("DELETE FROM library_artist_projection", [])
        .map_err(|error| format!("清理歌手查询索引失败：{error}"))?;
    let mut statement = connection
        .prepare(
            "INSERT INTO library_artist_projection
               (artist_id, canonical_name, aliases_json, raw_names_json, song_count, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, unixepoch())",
        )
        .map_err(|error| format!("准备歌手查询索引失败：{error}"))?;
    for summary in summaries {
        statement
            .execute(params![
                summary.artist_id,
                summary.canonical_name,
                serde_json::to_string(&summary.aliases).unwrap_or_else(|_| "[]".into()),
                serde_json::to_string(&summary.raw_names).unwrap_or_else(|_| "[]".into()),
                summary.song_count as i64,
            ])
            .map_err(|error| format!("更新歌手查询索引失败：{error}"))?;
    }
    drop(statement);
    for (artist_id, generation) in pending {
        connection
            .execute(
                "INSERT INTO library_artist_projection_state(artist_id, generation)
                 VALUES (?1, ?2)
                 ON CONFLICT(artist_id) DO UPDATE SET generation=excluded.generation",
                params![artist_id, generation],
            )
            .map_err(|error| format!("完成歌手查询索引失败：{error}"))?;
    }
    connection
        .execute(
            "UPDATE library_index_state SET phase='ready', revision=revision+1,
                    processed=total, last_error=NULL WHERE index_kind='artist'",
            [],
        )
        .map_err(|error| format!("更新歌手索引状态失败：{error}"))?;
    Ok(())
}

fn projected_artist_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryArtistSummary> {
    Ok(LibraryArtistSummary {
        artist_id: row.get(0)?,
        canonical_name: row.get(1)?,
        aliases: serde_json::from_str(&row.get::<_, String>(2)?).unwrap_or_default(),
        raw_names: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
        song_count: row.get::<_, i64>(4)?.max(0) as u64,
    })
}

impl Storage {
    pub fn list_library_artists(
        &self,
        query: &str,
        page: u64,
        page_size: u64,
    ) -> Result<LibraryPage<LibraryArtistSummary>, String> {
        let query = normalized_library_search_value(query.trim());
        if !query.is_empty() {
            self.rebuild_library_search_index()?;
        }
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        refresh_artist_projection_if_dirty(&connection)?;
        if query.is_empty() {
            let (page, page_size, offset) = library_page_parameters(page, page_size);
            let total = connection
                .query_row(
                    "SELECT COUNT(*) FROM library_artist_projection",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|error| format!("读取歌手资料库总数失败：{error}"))?
                .max(0) as u64;
            let mut statement = connection
                .prepare(
                    "SELECT artist_id, canonical_name, aliases_json, raw_names_json, song_count
                     FROM library_artist_projection
                     ORDER BY canonical_name, artist_id LIMIT ?1 OFFSET ?2",
                )
                .map_err(|error| format!("准备歌手资料库分页查询失败：{error}"))?;
            let items = statement
                .query_map(params![page_size as i64, offset], projected_artist_summary)
                .map_err(|error| format!("读取歌手资料库失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析歌手资料库失败：{error}"))?;
            return Ok(LibraryPage {
                items,
                total,
                page,
                page_size: page_size as u64,
            });
        }
        let (ids, total, page, page_size) =
            search_index_page(&connection, "artist", &query, page, page_size)?;
        let mut items = Vec::with_capacity(ids.len());
        for artist_id in ids {
            let item = connection
                .query_row(
                    "SELECT artist_id, canonical_name, aliases_json, raw_names_json, song_count
                     FROM library_artist_projection WHERE artist_id=?1",
                    params![artist_id],
                    projected_artist_summary,
                )
                .map_err(|error| format!("读取歌手资料库搜索结果失败：{error}"))?;
            items.push(item);
        }
        Ok(LibraryPage {
            items,
            total,
            page,
            page_size: page_size as u64,
        })
    }

    pub fn library_artist_detail(&self, artist_id: i64) -> Result<LibraryArtistDetail, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let rows = all_artist_rows(&connection)?;
        let summary = library_artist_summary(&connection, &rows, artist_id)?;
        let (_, members, _) = artist_group_member_ids(&connection, &rows, artist_id)?;
        let member_ids_json = serde_json::to_string(&members)
            .map_err(|error| format!("编码歌手成员失败：{error}"))?;
        let mut statement = connection
            .prepare(
                "SELECT DISTINCT credit.recording_id
                 FROM recording_artist_credits AS credit
                 JOIN json_each(?1) AS member
                   ON credit.artist_id=CAST(member.value AS INTEGER)",
            )
            .map_err(|error| format!("准备歌手关联歌曲集合查询失败：{error}"))?;
        let song_ids = statement
            .query_map(params![member_ids_json], |row| row.get::<_, i64>(0))
            .map_err(|error| format!("读取歌手关联歌曲失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析歌手关联歌曲失败：{error}"))?;
        drop(statement);
        let mut songs = library_song_summaries(&connection, &song_ids)?;
        songs.sort_by(|left, right| left.title.cmp(&right.title));
        Ok(LibraryArtistDetail { summary, songs })
    }

    pub fn update_library_artist_name(
        &self,
        artist_id: i64,
        name: &str,
    ) -> Result<LibraryArtistDetail, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("歌手名称不能为空".into());
        }
        let normalized = normalized_identity_artist(name);
        if normalized.is_empty() {
            return Err("歌手名称不包含有效字符".into());
        }
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let rows = all_artist_rows(&connection)?;
        let (anchor, members, _) = artist_group_member_ids(&connection, &rows, artist_id)?;
        let canonical_conflict = rows.iter().any(|row| {
            !members.contains(&row.artist_id)
                && normalized_identity_artist(&row.canonical_name) == normalized
        });
        let alias_conflict = {
            let mut statement = connection
                .prepare("SELECT artist_id, alias FROM artist_aliases WHERE confirmed=1")
                .map_err(|error| format!("准备歌手别名冲突查询失败：{error}"))?;
            let conflict = statement
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|error| format!("读取歌手别名冲突失败：{error}"))?
                .filter_map(Result::ok)
                .any(|(owner_id, alias)| {
                    !members.contains(&owner_id) && normalized_identity_artist(&alias) == normalized
                });
            conflict
        };
        let conflict = canonical_conflict || alias_conflict;
        if conflict {
            return Err("该名称已属于另一个歌手，请改为添加别名".into());
        }
        connection
            .execute(
                "UPDATE artists SET canonical_name=?2, updated_at=unixepoch() WHERE artist_id=?1",
                params![anchor, name],
            )
            .map_err(|error| format!("更新歌手名称失败：{error}"))?;
        drop(connection);
        self.library_artist_detail(anchor)
    }

    pub fn set_library_artist_alias(
        &self,
        artist_id: i64,
        alias: &str,
        confirmed: bool,
    ) -> Result<LibraryArtistDetail, String> {
        let alias = alias.trim();
        if alias.is_empty() {
            return Err("歌手别名不能为空".into());
        }
        let normalized_alias = normalized_artist_alias(alias);
        if normalized_alias.is_empty() {
            return Err("歌手别名不包含有效字符".into());
        }
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let rows = all_artist_rows(&connection)?;
        let (anchor, _, names) = artist_group_member_ids(&connection, &rows, artist_id)?;
        let canonical_name = rows
            .iter()
            .find(|row| row.artist_id == anchor)
            .map(|row| row.canonical_name.as_str())
            .unwrap_or_default();
        if confirmed && normalized_artist_alias(canonical_name) == normalized_alias {
            return Err("歌手别名与规范歌手名称相同".into());
        }
        if confirmed
            && names
                .iter()
                .any(|name| normalized_artist_alias(name) == normalized_alias)
        {
            return Err("该歌手等价名称已存在".into());
        }
        if confirmed {
            connection
                .execute(
                    "INSERT INTO artist_aliases (artist_id, alias, normalized_alias, confirmed)
                 VALUES (?1, ?2, ?3, 1)
                 ON CONFLICT(artist_id, normalized_alias) DO UPDATE SET
                   alias=excluded.alias, confirmed=1, updated_at=unixepoch()",
                    params![anchor, alias, normalized_alias],
                )
                .map_err(|error| format!("保存歌手别名失败：{error}"))?;
        } else {
            let ids = artist_alias_ids_for_removal(&connection, anchor, canonical_name, alias)?;
            for id in ids {
                connection.execute("UPDATE artist_aliases SET confirmed=0, updated_at=unixepoch() WHERE alias_id=?1", params![id])
                    .map_err(|error| format!("移除歌手别名失败：{error}"))?;
            }
        }
        drop(connection);
        self.library_artist_detail(anchor)
    }
}
