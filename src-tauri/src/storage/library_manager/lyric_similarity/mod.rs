use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension};

use super::super::{normalize_lyric_content, Storage};
use super::index::pending_index_generations;
use super::lyrics::{library_lyric_summaries, source_absolute_path};
use super::models::{
    LibraryLyricSimilarityPage, LibraryLyricSummary, LibraryPage, LyricSimilarityGroup,
};
use super::pagination::{library_page, library_page_parameters};

const MIN_LYRIC_SIMILARITY: f64 = 0.85;
const HIGH_LYRIC_SIMILARITY: f64 = 0.95;

fn normalized_library_label(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn lyric_qgram_edit_lower_bound(left: &str, right: &str) -> usize {
    const Q: usize = 3;
    let mut counts = std::collections::HashMap::<u64, i32>::new();
    for (value, delta) in [(left, 1_i32), (right, -1_i32)] {
        let characters = value.chars().collect::<Vec<_>>();
        for window in characters.windows(Q) {
            let mut hasher = DefaultHasher::new();
            window.hash(&mut hasher);
            *counts.entry(hasher.finish()).or_default() += delta;
        }
    }
    let symmetric_difference = counts
        .values()
        .map(|value| value.unsigned_abs() as usize)
        .sum::<usize>();
    symmetric_difference.div_ceil(2 * Q)
}

struct LyricContentLru {
    capacity: usize,
    order: std::collections::VecDeque<i64>,
    values: std::collections::HashMap<i64, std::sync::Arc<String>>,
}

struct LyricTitleBkNode {
    title: String,
    asset_ids: Vec<i64>,
    children: std::collections::HashMap<usize, Box<LyricTitleBkNode>>,
}

impl LyricTitleBkNode {
    fn new(title: String, asset_id: i64) -> Self {
        Self {
            title,
            asset_ids: vec![asset_id],
            children: std::collections::HashMap::new(),
        }
    }

    fn insert(&mut self, title: String, asset_id: i64) {
        let distance = strsim::levenshtein(&self.title, &title);
        if distance == 0 {
            self.asset_ids.push(asset_id);
            return;
        }
        match self.children.get_mut(&distance) {
            Some(child) => child.insert(title, asset_id),
            None => {
                self.children
                    .insert(distance, Box::new(Self::new(title, asset_id)));
            }
        }
    }

    fn find_within(&self, title: &str, radius: usize, output: &mut Vec<i64>) {
        let distance = strsim::levenshtein(&self.title, title);
        if distance <= radius {
            output.extend(self.asset_ids.iter().copied());
        }
        let minimum = distance.saturating_sub(radius);
        let maximum = distance.saturating_add(radius);
        for (edge, child) in &self.children {
            if *edge >= minimum && *edge <= maximum {
                child.find_within(title, radius, output);
            }
        }
    }
}

fn lyric_similarity_candidate_pairs(
    summaries: &std::collections::HashMap<i64, LibraryLyricSummary>,
    recordings: &std::collections::HashMap<i64, HashSet<i64>>,
) -> HashSet<(i64, i64)> {
    let mut pairs = HashSet::new();
    let mut assets_by_recording = std::collections::HashMap::<i64, Vec<i64>>::new();
    for (asset_id, recording_ids) in recordings {
        for recording_id in recording_ids {
            assets_by_recording
                .entry(*recording_id)
                .or_default()
                .push(*asset_id);
        }
    }
    for asset_ids in assets_by_recording.values_mut() {
        asset_ids.sort_unstable();
        asset_ids.dedup();
        for index in 0..asset_ids.len() {
            for right in asset_ids.iter().copied().skip(index + 1) {
                pairs.insert((asset_ids[index], right));
            }
        }
    }
    let mut tree: Option<LyricTitleBkNode> = None;
    let mut ids = summaries.keys().copied().collect::<Vec<_>>();
    ids.sort_unstable();
    for asset_id in ids {
        let title = normalized_library_label(&summaries[&asset_id].title);
        if title.is_empty() {
            continue;
        }
        let title_length = title.chars().count();
        // 对任意可达到 70% 的另一标题，编辑距离不会超过当前长度的 3/7。
        let radius = title_length.saturating_mul(3) / 7;
        if let Some(root) = tree.as_ref() {
            let mut matches = Vec::new();
            root.find_within(&title, radius, &mut matches);
            for other_id in matches {
                let other_title = normalized_library_label(&summaries[&other_id].title);
                if strsim::normalized_levenshtein(&title, &other_title) >= 0.70 {
                    pairs.insert((other_id.min(asset_id), other_id.max(asset_id)));
                }
            }
        }
        match tree.as_mut() {
            Some(root) => root.insert(title, asset_id),
            None => tree = Some(LyricTitleBkNode::new(title, asset_id)),
        }
    }
    pairs
}

impl LyricContentLru {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: std::collections::VecDeque::new(),
            values: std::collections::HashMap::new(),
        }
    }

    fn get(
        &mut self,
        asset_id: i64,
        paths: &std::collections::HashMap<i64, Vec<PathBuf>>,
    ) -> Option<std::sync::Arc<String>> {
        if let Some(value) = self.values.get(&asset_id).cloned() {
            self.order.retain(|id| *id != asset_id);
            self.order.push_back(asset_id);
            return Some(value);
        }
        let normalized = paths.get(&asset_id)?.iter().find_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .map(|raw| normalize_lyric_content(&raw))
                .filter(|value| !value.is_empty())
        })?;
        let value = std::sync::Arc::new(normalized);
        self.values.insert(asset_id, value.clone());
        self.order.push_back(asset_id);
        while self.values.len() > self.capacity {
            if let Some(evicted) = self.order.pop_front() {
                self.values.remove(&evicted);
            }
        }
        Some(value)
    }
}

fn lyric_quality(summary: &LibraryLyricSummary) -> (u64, bool, bool, bool, bool) {
    (
        summary.active_count,
        summary.has_word_timing,
        summary.has_translation,
        summary.has_romanization,
        summary.available,
    )
}

impl Storage {
    pub fn list_library_lyric_similarity(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<LibraryLyricSimilarityPage, String> {
        let cached_page = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let ready = connection
                .query_row(
                    "SELECT phase='ready' AND NOT EXISTS(
                       SELECT 1 FROM library_index_dirty AS dirty
                       LEFT JOIN library_similarity_state AS indexed
                         ON indexed.index_kind='lyric_similarity'
                        AND indexed.entity_id=dirty.entity_id
                       WHERE dirty.index_kind='lyric'
                         AND (indexed.generation IS NULL
                              OR indexed.generation!=dirty.generation)
                     ) FROM library_index_state WHERE index_kind='lyric_similarity'",
                    [],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap_or(false);
            if ready {
                let (page, page_size, offset) = library_page_parameters(page, page_size);
                let total = connection
                    .query_row("SELECT COUNT(*) FROM lyric_similarity_groups", [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .map_err(|error| format!("读取相似歌词总数失败：{error}"))?
                    .max(0) as u64;
                let mut statement = connection
                    .prepare(
                        "SELECT group_json FROM lyric_similarity_groups
                         ORDER BY score DESC, group_id LIMIT ?1 OFFSET ?2",
                    )
                    .map_err(|error| format!("准备相似歌词分页失败：{error}"))?;
                let items = statement
                    .query_map(params![page_size as i64, offset], |row| {
                        row.get::<_, String>(0)
                    })
                    .map_err(|error| format!("读取相似歌词分页失败：{error}"))?
                    .map(|value| {
                        let value =
                            value.map_err(|error| format!("解析相似歌词分页失败：{error}"))?;
                        serde_json::from_str(&value)
                            .map_err(|error| format!("解析相似歌词分页失败：{error}"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Some(LibraryPage {
                    items,
                    total,
                    page,
                    page_size: page_size as u64,
                })
            } else {
                None
            }
        };
        let result = match cached_page {
            Some(result) => result,
            None => library_page(self.analyze_library_lyric_similarity()?, page, page_size),
        };
        let mut previews = std::collections::HashMap::new();
        for asset_id in result
            .items
            .iter()
            .flat_map(|group| group.items.iter().map(|item| item.asset_id))
        {
            let document = self.library_lyric_detail(asset_id)?.document;
            previews.insert(asset_id, document);
        }
        Ok(LibraryLyricSimilarityPage {
            items: result.items,
            total: result.total,
            page: result.page,
            page_size: result.page_size,
            previews,
        })
    }

    pub fn analyze_library_lyric_similarity(&self) -> Result<Vec<LyricSimilarityGroup>, String> {
        let started = std::time::Instant::now();
        let (
            summaries,
            paths,
            recordings,
            duration_by_asset,
            version_by_asset,
            ignored,
            dirty_snapshot,
            changed_asset_ids,
            cached_edges,
            previous_ready,
        ) = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let previous_ready = connection
                .query_row(
                    "SELECT phase='ready' FROM library_index_state
                     WHERE index_kind='lyric_similarity'",
                    [],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap_or(false);
            let changed_asset_ids = {
                let mut statement = connection
                    .prepare(
                        "SELECT dirty.entity_id FROM library_index_dirty AS dirty
                         LEFT JOIN library_similarity_state AS indexed
                           ON indexed.index_kind='lyric_similarity'
                          AND indexed.entity_id=dirty.entity_id
                         WHERE dirty.index_kind='lyric'
                           AND (indexed.generation IS NULL
                                OR indexed.generation!=dirty.generation)",
                    )
                    .map_err(|error| format!("准备相似歌词增量范围失败：{error}"))?;
                let changed_asset_ids = statement
                    .query_map([], |row| row.get::<_, i64>(0))
                    .map_err(|error| format!("读取相似歌词增量范围失败：{error}"))?
                    .collect::<rusqlite::Result<HashSet<_>>>()
                    .map_err(|error| format!("解析相似歌词增量范围失败：{error}"))?;
                changed_asset_ids
            };
            let cached_edges = if previous_ready && !changed_asset_ids.is_empty() {
                let mut statement = connection
                    .prepare(
                        "SELECT left_asset_id, right_asset_id, score
                         FROM lyric_similarity_edges",
                    )
                    .map_err(|error| format!("准备复用相似歌词边失败：{error}"))?;
                let cached_edges = statement
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, f64>(2)?,
                        ))
                    })
                    .map_err(|error| format!("读取可复用相似歌词边失败：{error}"))?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(|error| format!("解析可复用相似歌词边失败：{error}"))?;
                cached_edges
            } else {
                Vec::new()
            };
            connection
                .execute(
                    "UPDATE library_index_state SET phase='building', processed=0,
                            total=(SELECT COUNT(*) FROM lyric_assets), last_error=NULL
                     WHERE index_kind='lyric_similarity'",
                    [],
                )
                .map_err(|error| format!("更新相似歌词索引状态失败：{error}"))?;
            let mut statement = connection
                .prepare("SELECT asset_id FROM lyric_assets WHERE available=1 ORDER BY asset_id")
                .map_err(|error| format!("准备相似歌词分析失败：{error}"))?;
            let ids = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(|error| format!("读取相似歌词资源失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析相似歌词资源失败：{error}"))?;
            drop(statement);
            let mut summaries = library_lyric_summaries(&connection, &ids)?
                .into_iter()
                .map(|summary| (summary.asset_id, summary))
                .collect::<std::collections::HashMap<_, _>>();
            let mut paths = std::collections::HashMap::<i64, Vec<PathBuf>>::new();
            let mut path_statement = connection
                .prepare(
                    "SELECT asset.asset_id, asset.root_dir_id, asset.relative_path, root.path
                     FROM lyric_assets AS asset
                     LEFT JOIN library_roots AS root ON root.root_id=asset.root_dir_id
                     WHERE asset.available=1 AND asset.relative_path IS NOT NULL
                     UNION ALL
                     SELECT source.asset_id, source.root_dir_id, source.relative_path, root.path
                     FROM lyric_asset_sources AS source
                     LEFT JOIN library_roots AS root ON root.root_id=source.root_dir_id
                     WHERE source.available=1",
                )
                .map_err(|error| format!("准备相似歌词内容路径集合查询失败：{error}"))?;
            for row in path_statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })
                .map_err(|error| format!("读取相似歌词内容路径失败：{error}"))?
            {
                let (asset_id, root_id, relative_path, root_path) =
                    row.map_err(|error| format!("解析相似歌词内容路径失败：{error}"))?;
                if let Some(path) = source_absolute_path(
                    root_id.as_deref(),
                    root_path.as_deref(),
                    relative_path.as_deref(),
                ) {
                    paths.entry(asset_id).or_default().push(path);
                }
            }
            drop(path_statement);
            summaries.retain(|asset_id, _| paths.contains_key(asset_id));

            let mut recordings = summaries
                .keys()
                .copied()
                .map(|asset_id| (asset_id, HashSet::new()))
                .collect::<std::collections::HashMap<_, _>>();
            let mut duration_by_asset = std::collections::HashMap::new();
            let mut version_by_asset = std::collections::HashMap::<i64, Option<String>>::new();
            let mut binding_statement = connection
                .prepare(
                    "SELECT binding.asset_id, binding.recording_id, recording.duration_ms,
                            recording.version_tags_json
                     FROM recording_lyric_bindings AS binding
                     JOIN recordings AS recording USING(recording_id)
                     ORDER BY binding.asset_id, binding.is_default DESC, binding.updated_at DESC",
                )
                .map_err(|error| format!("准备相似歌词关联集合查询失败：{error}"))?;
            for row in binding_statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })
                .map_err(|error| format!("读取相似歌词关联数据失败：{error}"))?
            {
                let (asset_id, recording_id, duration, version) =
                    row.map_err(|error| format!("解析相似歌词关联数据失败：{error}"))?;
                let Some(bound) = recordings.get_mut(&asset_id) else {
                    continue;
                };
                bound.insert(recording_id);
                duration_by_asset.entry(asset_id).or_insert(duration);
                version_by_asset.entry(asset_id).or_insert(Some(version));
            }
            let mut ignored_statement = connection
                .prepare("SELECT left_fingerprint, right_fingerprint FROM lyric_similarity_ignores")
                .map_err(|error| format!("准备相似歌词忽略项失败：{error}"))?;
            let ignored = ignored_statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|error| format!("读取相似歌词忽略项失败：{error}"))?
                .collect::<rusqlite::Result<HashSet<_>>>()
                .map_err(|error| format!("解析相似歌词忽略项失败：{error}"))?;
            let mut dirty_statement = connection
                .prepare(
                    "SELECT entity_id, generation FROM library_index_dirty
                     WHERE index_kind='lyric' ORDER BY entity_id",
                )
                .map_err(|error| format!("准备相似歌词索引水位失败：{error}"))?;
            let dirty_snapshot = dirty_statement
                .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
                .map_err(|error| format!("读取相似歌词索引水位失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析相似歌词索引水位失败：{error}"))?;
            (
                summaries,
                paths,
                recordings,
                duration_by_asset,
                version_by_asset,
                ignored,
                dirty_snapshot,
                changed_asset_ids,
                cached_edges,
                previous_ready,
            )
        };
        let mut content_cache = LyricContentLru::new(256);
        let mut candidate_pairs = lyric_similarity_candidate_pairs(&summaries, &recordings)
            .into_iter()
            .collect::<Vec<_>>();
        let incremental = previous_ready && !changed_asset_ids.is_empty();
        if incremental {
            candidate_pairs.retain(|(left_id, right_id)| {
                changed_asset_ids.contains(left_id) || changed_asset_ids.contains(right_id)
            });
        }
        candidate_pairs.sort_unstable();
        let progress_connection = Connection::open(&self.database_path)
            .map_err(|error| format!("打开相似歌词进度连接失败：{error}"))?;
        progress_connection
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(|error| format!("初始化相似歌词进度连接失败：{error}"))?;
        progress_connection
            .execute(
                "UPDATE library_index_state SET total=?2 WHERE index_kind=?1",
                params!["lyric_similarity", candidate_pairs.len() as i64],
            )
            .map_err(|error| format!("更新相似歌词候选总数失败：{error}"))?;
        let mut adjacency = std::collections::HashMap::<i64, Vec<(i64, f64)>>::new();
        for (left_id, right_id, score) in cached_edges {
            if changed_asset_ids.contains(&left_id)
                || changed_asset_ids.contains(&right_id)
                || !summaries.contains_key(&left_id)
                || !summaries.contains_key(&right_id)
            {
                continue;
            }
            adjacency
                .entry(left_id)
                .or_default()
                .push((right_id, score));
            adjacency
                .entry(right_id)
                .or_default()
                .push((left_id, score));
        }
        for (index, (left_id, right_id)) in candidate_pairs.iter().enumerate() {
            if index % 128 == 0 {
                progress_connection
                    .execute(
                        "UPDATE library_index_state SET processed=?2
                         WHERE index_kind=?1",
                        params!["lyric_similarity", index as i64],
                    )
                    .map_err(|error| format!("报告相似歌词索引进度失败：{error}"))?;
            }
            let left = &summaries[left_id];
            let right = &summaries[right_id];
            let fingerprint_pair = if left.content_fingerprint < right.content_fingerprint {
                (&left.content_fingerprint, &right.content_fingerprint)
            } else {
                (&right.content_fingerprint, &left.content_fingerprint)
            };
            if ignored.contains(&(fingerprint_pair.0.clone(), fingerprint_pair.1.clone())) {
                continue;
            }
            let title_left = normalized_library_label(&left.title);
            let title_right = normalized_library_label(&right.title);
            let shared_recording = recordings[left_id]
                .iter()
                .any(|id| recordings[right_id].contains(id));
            let title_similarity = strsim::normalized_levenshtein(&title_left, &title_right);
            if !shared_recording && (title_left.is_empty() || title_similarity < 0.70) {
                continue;
            }
            let Some(left_content) = content_cache.get(*left_id, &paths) else {
                continue;
            };
            let Some(right_content) = content_cache.get(*right_id, &paths) else {
                continue;
            };
            let left_length = left_content.chars().count();
            let right_length = right_content.chars().count();
            let maximum_length = left_length.max(right_length);
            let minimum_length = left_length.min(right_length);
            if maximum_length == 0
                || minimum_length as f64 / (maximum_length as f64) < MIN_LYRIC_SIMILARITY
            {
                continue;
            }
            let maximum_edits =
                ((1.0 - MIN_LYRIC_SIMILARITY) * maximum_length as f64).floor() as usize;
            if lyric_qgram_edit_lower_bound(left_content.as_str(), right_content.as_str())
                > maximum_edits
            {
                continue;
            }
            let score = strsim::normalized_levenshtein(&left_content, &right_content);
            if score < MIN_LYRIC_SIMILARITY {
                continue;
            }
            adjacency
                .entry(*left_id)
                .or_default()
                .push((*right_id, score));
            adjacency
                .entry(*right_id)
                .or_default()
                .push((*left_id, score));
        }
        let mut visited = HashSet::new();
        let mut groups = Vec::new();
        for seed in adjacency.keys().copied().collect::<Vec<_>>() {
            if !visited.insert(seed) {
                continue;
            }
            let mut pending = vec![seed];
            let mut members = Vec::new();
            let mut score: f64 = 0.0;
            while let Some(current) = pending.pop() {
                members.push(current);
                for (neighbor, pair_score) in adjacency.get(&current).into_iter().flatten() {
                    score = score.max(*pair_score);
                    if visited.insert(*neighbor) {
                        pending.push(*neighbor);
                    }
                }
            }
            members.sort_unstable();
            let mut items = members
                .iter()
                .filter_map(|id| summaries.get(id).cloned())
                .collect::<Vec<_>>();
            items.sort_by(|left, right| {
                lyric_quality(right)
                    .cmp(&lyric_quality(left))
                    .then_with(|| left.asset_id.cmp(&right.asset_id))
            });
            let recommended_asset_id = items.first().map(|item| item.asset_id).unwrap_or(seed);
            let durations = members
                .iter()
                .filter_map(|id| duration_by_asset.get(id).copied().flatten())
                .collect::<Vec<_>>();
            let duration_warning = durations
                .iter()
                .min()
                .zip(durations.iter().max())
                .is_some_and(|(min, max)| max - min > 10_000);
            let version_warning = members
                .iter()
                .filter_map(|id| version_by_asset.get(id).cloned().flatten())
                .collect::<HashSet<_>>()
                .len()
                > 1;
            groups.push(LyricSimilarityGroup {
                group_id: members
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join("-"),
                score,
                high_similarity: score >= HIGH_LYRIC_SIMILARITY,
                recommended_asset_id,
                duration_warning,
                version_warning,
                items,
            });
        }
        groups.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.group_id.cmp(&right.group_id))
        });
        drop(progress_connection);
        let mut connection = Connection::open(&self.database_path)
            .map_err(|error| format!("打开相似歌词后台连接失败：{error}"))?;
        connection
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(|error| format!("初始化相似歌词后台连接失败：{error}"))?;
        let current_dirty = pending_index_generations(&connection, "lyric")?;
        if current_dirty != dirty_snapshot {
            return Err("资料库在相似歌词分析期间发生变化，已保留增量重建任务，请重试".into());
        }
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始保存相似歌词索引失败：{error}"))?;
        transaction
            .execute_batch(
                "DELETE FROM lyric_similarity_edges;
                 DELETE FROM lyric_similarity_groups;",
            )
            .map_err(|error| format!("清理相似歌词索引失败：{error}"))?;
        {
            let mut edge_statement = transaction
                .prepare(
                    "INSERT INTO lyric_similarity_edges(left_asset_id, right_asset_id, score)
                     VALUES (?1, ?2, ?3)",
                )
                .map_err(|error| format!("准备保存相似歌词边失败：{error}"))?;
            for (left_id, neighbors) in &adjacency {
                for (right_id, score) in neighbors {
                    if left_id < right_id {
                        edge_statement
                            .execute(params![left_id, right_id, score])
                            .map_err(|error| format!("保存相似歌词边失败：{error}"))?;
                    }
                }
            }
        }
        {
            let mut group_statement = transaction
                .prepare(
                    "INSERT INTO lyric_similarity_groups(group_id, score, group_json)
                     VALUES (?1, ?2, ?3)",
                )
                .map_err(|error| format!("准备保存相似歌词分组失败：{error}"))?;
            for group in &groups {
                group_statement
                    .execute(params![
                        group.group_id,
                        group.score,
                        serde_json::to_string(group).unwrap_or_else(|_| "{}".into())
                    ])
                    .map_err(|error| format!("保存相似歌词分组失败：{error}"))?;
            }
        }
        transaction
            .execute(
                "INSERT INTO library_similarity_state(index_kind, entity_id, generation)
                 SELECT 'lyric_similarity', entity_id, generation FROM library_index_dirty
                 WHERE index_kind='lyric'
                 ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=excluded.generation",
                [],
            )
            .map_err(|error| format!("保存相似歌词索引水位失败：{error}"))?;
        transaction
            .execute(
                "UPDATE library_index_state SET phase='ready', processed=total,
                        revision=revision+1, last_error=NULL
                 WHERE index_kind='lyric_similarity'",
                [],
            )
            .map_err(|error| format!("完成相似歌词索引失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交相似歌词索引失败：{error}"))?;
        log::debug!(
            "相似歌词索引完成：assets={} dirty={} candidates={} groups={} elapsed_ms={}",
            summaries.len(),
            changed_asset_ids.len(),
            candidate_pairs.len(),
            groups.len(),
            started.elapsed().as_millis()
        );
        Ok(groups)
    }

    pub fn dismiss_library_lyric_similarity(&self, asset_ids: &[i64]) -> Result<(), String> {
        if asset_ids.len() < 2 {
            return Err("至少需要两份歌词".into());
        }
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始保存相似歌词决定失败：{error}"))?;
        let mut fingerprints = Vec::new();
        for id in asset_ids {
            let fingerprint = transaction
                .query_row(
                    "SELECT content_fingerprint FROM lyric_assets WHERE asset_id=?1",
                    params![id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| format!("读取歌词指纹失败：{error}"))?
                .ok_or_else(|| "歌词资源不存在".to_string())?;
            fingerprints.push(fingerprint);
        }
        for index in 0..fingerprints.len() {
            for right in fingerprints.iter().skip(index + 1) {
                let left = &fingerprints[index];
                let (left, right) = if left < right {
                    (left, right)
                } else {
                    (right, left)
                };
                transaction.execute(
                    "INSERT OR IGNORE INTO lyric_similarity_ignores (left_fingerprint, right_fingerprint) VALUES (?1, ?2)",
                    params![left, right],
                ).map_err(|error| format!("保存相似歌词决定失败：{error}"))?;
            }
        }
        transaction
            .execute(
                "UPDATE library_index_state SET phase='pending'
                 WHERE index_kind='lyric_similarity'",
                [],
            )
            .map_err(|error| format!("刷新相似歌词索引状态失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交相似歌词决定失败：{error}"))
    }

    pub fn merge_library_lyrics(
        &self,
        keeper_asset_id: i64,
        redundant_asset_ids: &[i64],
    ) -> Result<(), String> {
        if redundant_asset_ids.is_empty() {
            return Err("没有选择需要合并的歌词".into());
        }
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始合并歌词失败：{error}"))?;
        let keeper_fingerprint = transaction
            .query_row(
                "SELECT content_fingerprint FROM lyric_assets WHERE asset_id=?1",
                params![keeper_asset_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("读取保留歌词失败：{error}"))?
            .ok_or_else(|| "保留歌词不存在".to_string())?;
        transaction
            .execute_batch(
                "DROP TABLE IF EXISTS temp.library_merge_assets;
                 CREATE TEMP TABLE library_merge_assets (
                   asset_id INTEGER PRIMARY KEY,
                   content_fingerprint TEXT NOT NULL
                 );",
            )
            .map_err(|error| format!("准备歌词集合合并失败：{error}"))?;
        for redundant_id in redundant_asset_ids
            .iter()
            .copied()
            .filter(|id| *id != keeper_asset_id)
        {
            let redundant_fingerprint = transaction
                .query_row(
                    "SELECT content_fingerprint FROM lyric_assets WHERE asset_id=?1",
                    params![redundant_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| format!("读取待合并歌词失败：{error}"))?
                .ok_or_else(|| "待合并歌词不存在".to_string())?;
            transaction
                .execute(
                    "INSERT OR IGNORE INTO library_merge_assets(asset_id, content_fingerprint)
                     VALUES (?1, ?2)",
                    params![redundant_id, redundant_fingerprint],
                )
                .map_err(|error| format!("登记待合并歌词失败：{error}"))?;
            let (left, right) = if keeper_fingerprint < redundant_fingerprint {
                (&keeper_fingerprint, &redundant_fingerprint)
            } else {
                (&redundant_fingerprint, &keeper_fingerprint)
            };
            transaction.execute(
                "INSERT OR IGNORE INTO lyric_similarity_ignores (left_fingerprint, right_fingerprint) VALUES (?1, ?2)", params![left, right],
            ).map_err(|error| format!("保存歌词合并决定失败：{error}"))?;
        }
        transaction
            .execute_batch(
                "DROP TABLE IF EXISTS temp.library_merge_bindings;
                 CREATE TEMP TABLE library_merge_bindings AS
                   SELECT binding.* FROM recording_lyric_bindings AS binding
                   JOIN library_merge_assets AS selected ON selected.asset_id=binding.asset_id;
                 UPDATE recording_lyric_bindings SET is_default=0, updated_at=unixepoch()
                 WHERE recording_id IN (
                   SELECT recording_id FROM library_merge_bindings WHERE is_default=1
                 );",
            )
            .map_err(|error| format!("整理迁移默认歌词失败：{error}"))?;
        transaction
            .execute(
                "INSERT INTO recording_lyric_bindings
                   (recording_id, asset_id, selection_source, confidence,
                    evidence_json, is_default, offset_ms)
                 SELECT recording_id, ?1, selection_source, confidence,
                        evidence_json, is_default, offset_ms
                 FROM library_merge_bindings WHERE 1
                 ON CONFLICT(recording_id, asset_id) DO UPDATE SET
                   is_default=MAX(recording_lyric_bindings.is_default, excluded.is_default),
                   offset_ms=CASE WHEN excluded.is_default=1
                                  THEN excluded.offset_ms
                                  ELSE recording_lyric_bindings.offset_ms END,
                   updated_at=unixepoch()",
                params![keeper_asset_id],
            )
            .map_err(|error| format!("迁移歌词绑定失败：{error}"))?;
        transaction
            .execute(
                "UPDATE platform_lyric_overrides SET asset_id=?1, updated_at=unixepoch()
                 WHERE asset_id IN (SELECT asset_id FROM library_merge_assets)",
                params![keeper_asset_id],
            )
            .map_err(|error| format!("迁移平台歌词覆盖失败：{error}"))?;
        transaction
            .execute(
                "DELETE FROM recording_lyric_bindings
                 WHERE asset_id IN (SELECT asset_id FROM library_merge_assets)",
                [],
            )
            .map_err(|error| format!("清理旧歌词绑定失败：{error}"))?;
        transaction
            .execute_batch(
                "DROP TABLE temp.library_merge_bindings;
                 DROP TABLE temp.library_merge_assets;",
            )
            .map_err(|error| format!("完成歌词集合合并失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交歌词合并失败：{error}"))
    }
}
