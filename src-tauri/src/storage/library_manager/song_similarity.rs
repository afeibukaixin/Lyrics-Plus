use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use rusqlite::{params, Connection};

use super::super::{load_observations, song_manager, SongAssociationCandidate, Storage};
use super::index::search_trigrams;
use super::models::{LibraryPage, LibrarySongSummary, SongSimilarityPair};
use super::pagination::{library_page, library_page_parameters};
use super::songs::library_song_summary;

const SONG_SIMILARITY_ALGORITHM_VERSION: u8 = 2;

fn song_similarity_candidate_targets(
    connection: &Connection,
    recording_ids: &[i64],
    settings: &crate::lyrics::provider::ProviderSettings,
) -> Result<std::collections::HashMap<i64, Vec<i64>>, String> {
    let known_ids = recording_ids.iter().copied().collect::<HashSet<_>>();
    let mut titles = std::collections::HashMap::<i64, Vec<String>>::new();
    let mut possible_pairs = HashSet::<(i64, i64)>::new();
    let mut strong_pairs = HashSet::<(i64, i64)>::new();
    let mut statement = connection
        .prepare(
            "SELECT recording_id, raw_title, split_from_recording_id
             FROM track_observations ORDER BY recording_id, observation_id",
        )
        .map_err(|error| format!("准备相似歌曲标题预筛失败：{error}"))?;
    let observations = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })
        .map_err(|error| format!("读取相似歌曲标题预筛失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析相似歌曲标题预筛失败：{error}"))?;
    drop(statement);
    let mut inverted = std::collections::HashMap::<String, Vec<i64>>::new();
    for (recording_id, title, split_from) in observations {
        if let Some(other) = split_from.filter(|id| known_ids.contains(id)) {
            let pair = (recording_id.min(other), recording_id.max(other));
            possible_pairs.insert(pair);
            strong_pairs.insert(pair);
        }
        for variant in crate::lyrics::provider::association_title_index_variants(&title, settings)?
        {
            inverted
                .entry(format!("exact:{variant}"))
                .or_default()
                .push(recording_id);
            for gram in search_trigrams(&variant) {
                inverted
                    .entry(format!("gram:{gram}"))
                    .or_default()
                    .push(recording_id);
            }
        }
        titles.entry(recording_id).or_default().push(title);
    }
    for ids in inverted.values_mut() {
        ids.sort_unstable();
        ids.dedup();
        for index in 0..ids.len() {
            for right in ids.iter().copied().skip(index + 1) {
                let pair = (ids[index], right);
                possible_pairs.insert(pair);
            }
        }
    }
    let mut identifier_statement = connection
        .prepare(
            "SELECT recording_id, lower(id_kind), lower(value)
             FROM recording_external_ids
             WHERE lower(namespace)='isrc' OR lower(id_kind) IN
               ('isrc', 'recording_id', 'recording',
                'musicbrainz_recording_id', 'musicbrainz_recording')",
        )
        .map_err(|error| format!("准备相似歌曲标识预筛失败：{error}"))?;
    let mut identifier_groups = std::collections::HashMap::<(String, String), Vec<i64>>::new();
    for value in identifier_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("读取相似歌曲标识预筛失败：{error}"))?
    {
        let (recording_id, kind, value) =
            value.map_err(|error| format!("解析相似歌曲标识预筛失败：{error}"))?;
        identifier_groups
            .entry((kind, value))
            .or_default()
            .push(recording_id);
    }
    for ids in identifier_groups.values_mut() {
        ids.sort_unstable();
        ids.dedup();
        for index in 0..ids.len() {
            for right in ids.iter().copied().skip(index + 1) {
                let pair = (ids[index], right);
                possible_pairs.insert(pair);
                strong_pairs.insert(pair);
            }
        }
    }
    let mut targets = std::collections::HashMap::<i64, Vec<i64>>::new();
    for (left, right) in possible_pairs {
        let strong = strong_pairs.contains(&(left, right));
        let title_match = if strong {
            true
        } else {
            titles.get(&left).into_iter().flatten().any(|left_title| {
                titles.get(&right).into_iter().flatten().any(|right_title| {
                    crate::lyrics::provider::association_title_similarity(
                        left_title,
                        right_title,
                        settings,
                    )
                    .is_ok_and(|score| score >= 0.90)
                })
            })
        };
        if title_match {
            targets.entry(left).or_default().push(right);
        }
    }
    Ok(targets)
}

impl Storage {
    pub fn list_library_song_similarity(
        &self,
        page: u64,
        page_size: u64,
        settings: &crate::lyrics::provider::ProviderSettings,
    ) -> Result<LibraryPage<SongSimilarityPair>, String> {
        let settings_fingerprint = song_similarity_settings_fingerprint(settings);
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let cache_ready = connection
            .query_row(
                "SELECT phase='ready' AND settings_fingerprint=?1 AND NOT EXISTS(
                   SELECT 1 FROM library_index_dirty AS dirty
                   LEFT JOIN library_similarity_state AS indexed
                     ON indexed.index_kind='song_similarity'
                    AND indexed.entity_id=dirty.entity_id
                   WHERE dirty.index_kind='song'
                     AND (indexed.generation IS NULL OR indexed.generation!=dirty.generation)
                 ) FROM library_index_state WHERE index_kind='song_similarity'",
                params![settings_fingerprint],
                |row| row.get::<_, bool>(0),
            )
            .unwrap_or(false);
        if cache_ready {
            let (page, page_size, offset) = library_page_parameters(page, page_size);
            let total = connection
                .query_row("SELECT COUNT(*) FROM song_similarity_pairs", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(|error| format!("读取相似歌曲总数失败：{error}"))?
                .max(0) as u64;
            let mut statement = connection
                .prepare(
                    "SELECT evidence_json FROM song_similarity_pairs
                     ORDER BY evidence_kind DESC, score DESC,
                              COALESCE(lyrics_similarity, 0) DESC,
                              left_recording_id, right_recording_id
                     LIMIT ?1 OFFSET ?2",
                )
                .map_err(|error| format!("准备相似歌曲分页失败：{error}"))?;
            let items = statement
                .query_map(params![page_size as i64, offset], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|error| format!("读取相似歌曲分页失败：{error}"))?
                .map(|value| {
                    let value = value.map_err(|error| format!("解析相似歌曲分页失败：{error}"))?;
                    serde_json::from_str(&value)
                        .map_err(|error| format!("解析相似歌曲分页失败：{error}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(LibraryPage {
                items,
                total,
                page,
                page_size: page_size as u64,
            });
        }
        drop(connection);
        Ok(library_page(
            self.analyze_library_song_similarity(settings)?,
            page,
            page_size,
        ))
    }

    pub fn analyze_library_song_similarity(
        &self,
        settings: &crate::lyrics::provider::ProviderSettings,
    ) -> Result<Vec<SongSimilarityPair>, String> {
        let started = std::time::Instant::now();
        let settings_fingerprint = song_similarity_settings_fingerprint(settings);
        let mut connection = Connection::open(&self.database_path)
            .map_err(|error| format!("打开相似歌曲后台连接失败：{error}"))?;
        connection
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(|error| format!("初始化相似歌曲后台连接失败：{error}"))?;
        let cache_ready = connection
            .query_row(
                "SELECT phase='ready' AND settings_fingerprint=?1 AND NOT EXISTS(
                   SELECT 1 FROM library_index_dirty AS dirty
                   LEFT JOIN library_similarity_state AS indexed
                     ON indexed.index_kind='song_similarity'
                    AND indexed.entity_id=dirty.entity_id
                   WHERE dirty.index_kind='song'
                     AND (indexed.generation IS NULL OR indexed.generation!=dirty.generation)
                 )
                 FROM library_index_state WHERE index_kind='song_similarity'",
                params![settings_fingerprint],
                |row| row.get::<_, bool>(0),
            )
            .unwrap_or(false);
        if cache_ready {
            let mut statement = connection
                .prepare(
                    "SELECT evidence_json FROM song_similarity_pairs
                     ORDER BY evidence_kind DESC, score DESC,
                              COALESCE(lyrics_similarity, 0) DESC,
                              left_recording_id, right_recording_id",
                )
                .map_err(|error| format!("准备相似歌曲缓存失败：{error}"))?;
            return statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|error| format!("读取相似歌曲缓存失败：{error}"))?
                .map(|value| {
                    let value = value.map_err(|error| format!("解析相似歌曲缓存失败：{error}"))?;
                    serde_json::from_str(&value)
                        .map_err(|error| format!("解析相似歌曲缓存失败：{error}"))
                })
                .collect();
        }
        let settings_match = connection
            .query_row(
                "SELECT phase='ready' AND settings_fingerprint=?1 FROM library_index_state
                 WHERE index_kind='song_similarity'",
                params![settings_fingerprint],
                |row| row.get::<_, bool>(0),
            )
            .unwrap_or(false);
        let dirty_recording_ids = {
            let mut statement = connection
                .prepare(
                    "SELECT dirty.entity_id FROM library_index_dirty AS dirty
                     LEFT JOIN library_similarity_state AS indexed
                       ON indexed.index_kind='song_similarity'
                      AND indexed.entity_id=dirty.entity_id
                     WHERE dirty.index_kind='song'
                       AND (indexed.generation IS NULL
                            OR indexed.generation!=dirty.generation)",
                )
                .map_err(|error| format!("准备相似歌曲增量任务失败：{error}"))?;
            let dirty_recording_ids = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(|error| format!("读取相似歌曲增量任务失败：{error}"))?
                .collect::<rusqlite::Result<HashSet<_>>>()
                .map_err(|error| format!("解析相似歌曲增量任务失败：{error}"))?;
            dirty_recording_ids
        };
        let incremental = settings_match && !dirty_recording_ids.is_empty();
        connection
            .execute(
                "UPDATE library_index_state SET phase='building', processed=0,
                        total=(SELECT COUNT(*) FROM recordings), last_error=NULL
                 WHERE index_kind='song_similarity'",
                [],
            )
            .map_err(|error| format!("更新相似歌曲索引状态失败：{error}"))?;
        let dirty_snapshot = {
            let mut statement = connection
                .prepare(
                    "SELECT entity_id, generation FROM library_index_dirty
                     WHERE index_kind='song' ORDER BY entity_id",
                )
                .map_err(|error| format!("准备相似歌曲索引水位失败：{error}"))?;
            let dirty_snapshot = statement
                .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
                .map_err(|error| format!("读取相似歌曲索引水位失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析相似歌曲索引水位失败：{error}"))?;
            dirty_snapshot
        };
        let mut statement = connection
            .prepare(
                "SELECT recording_id FROM recordings
                 WHERE EXISTS (SELECT 1 FROM track_observations
                               WHERE track_observations.recording_id=recordings.recording_id)
                 ORDER BY recording_id",
            )
            .map_err(|error| format!("准备相似歌曲分析失败：{error}"))?;
        let recording_ids = statement
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(|error| format!("读取相似歌曲失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析相似歌曲失败：{error}"))?;
        drop(statement);
        let candidate_targets =
            song_similarity_candidate_targets(&connection, &recording_ids, settings)?;
        let recording_count = recording_ids.len();
        let candidate_count = candidate_targets.values().map(Vec::len).sum::<usize>();
        let possible_count = recording_count.saturating_mul(recording_count.saturating_sub(1)) / 2;
        let mut pairs = std::collections::BTreeMap::<(i64, i64), SongSimilarityPair>::new();
        if incremental {
            let mut statement = connection
                .prepare("SELECT evidence_json FROM song_similarity_pairs")
                .map_err(|error| format!("准备相似歌曲增量缓存失败：{error}"))?;
            for value in statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|error| format!("读取相似歌曲增量缓存失败：{error}"))?
            {
                let value = value.map_err(|error| format!("解析相似歌曲增量缓存失败：{error}"))?;
                let pair: SongSimilarityPair = serde_json::from_str(&value)
                    .map_err(|error| format!("解析相似歌曲增量缓存失败：{error}"))?;
                let left = pair.songs[0].recording_id.min(pair.songs[1].recording_id);
                let right = pair.songs[0].recording_id.max(pair.songs[1].recording_id);
                if !dirty_recording_ids.contains(&left) && !dirty_recording_ids.contains(&right) {
                    pairs.insert((left, right), pair);
                }
            }
        }

        for (processed_recordings, source_recording_id) in recording_ids.into_iter().enumerate() {
            if processed_recordings % 32 == 0 {
                connection
                    .execute(
                        "UPDATE library_index_state SET processed=?2
                         WHERE index_kind=?1",
                        params!["song_similarity", processed_recordings as i64],
                    )
                    .map_err(|error| format!("报告相似歌曲索引进度失败：{error}"))?;
            }
            let Some(all_target_recording_ids) = candidate_targets.get(&source_recording_id) else {
                continue;
            };
            let target_recording_ids = if incremental {
                all_target_recording_ids
                    .iter()
                    .copied()
                    .filter(|target| {
                        dirty_recording_ids.contains(&source_recording_id)
                            || dirty_recording_ids.contains(target)
                    })
                    .collect::<Vec<_>>()
            } else {
                all_target_recording_ids.clone()
            };
            if target_recording_ids.is_empty() {
                continue;
            }
            let Some(observation) = load_observations(&connection, source_recording_id)?
                .into_iter()
                .next()
            else {
                continue;
            };
            let candidates = song_manager::collect_song_association_candidates_for_targets(
                &connection,
                &observation.platform,
                &observation.track_key,
                settings,
                &target_recording_ids,
            )?;
            for evidence in candidates {
                // 每个无序 Recording 对只执行一次完整证据汇总。
                if evidence.recording_id <= source_recording_id {
                    continue;
                }
                let pair_key = if source_recording_id < evidence.recording_id {
                    (source_recording_id, evidence.recording_id)
                } else {
                    (evidence.recording_id, source_recording_id)
                };
                if ignored_song_similarity(&connection, pair_key.0, pair_key.1) {
                    continue;
                }
                let should_replace = pairs.get(&pair_key).is_none_or(|current| {
                    stronger_song_similarity_evidence(&evidence, &current.evidence)
                });
                if !should_replace {
                    continue;
                }
                let songs = [
                    library_song_summary(&connection, pair_key.0)?,
                    library_song_summary(&connection, pair_key.1)?,
                ];
                let recommended_recording_id = recommended_song_recording_id(&songs);
                pairs.insert(
                    pair_key,
                    SongSimilarityPair {
                        pair_id: format!("{}-{}", pair_key.0, pair_key.1),
                        songs,
                        recommended_recording_id,
                        evidence_source_recording_id: source_recording_id,
                        evidence,
                    },
                );
            }
        }

        let mut pairs = pairs.into_values().collect::<Vec<_>>();
        pairs.sort_by(|left, right| {
            right
                .evidence
                .kind
                .cmp(&left.evidence.kind)
                .then_with(|| right.evidence.score.total_cmp(&left.evidence.score))
                .then_with(|| {
                    right
                        .evidence
                        .lyrics_similarity
                        .unwrap_or_default()
                        .total_cmp(&left.evidence.lyrics_similarity.unwrap_or_default())
                })
                .then_with(|| left.pair_id.cmp(&right.pair_id))
        });
        let current_dirty = {
            let mut statement = connection
                .prepare(
                    "SELECT entity_id, generation FROM library_index_dirty
                     WHERE index_kind='song' ORDER BY entity_id",
                )
                .map_err(|error| format!("准备校验相似歌曲索引水位失败：{error}"))?;
            let current_dirty = statement
                .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
                .map_err(|error| format!("校验相似歌曲索引水位失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析相似歌曲索引水位失败：{error}"))?;
            current_dirty
        };
        if current_dirty != dirty_snapshot {
            return Err("资料库在相似分析期间发生变化，已保留增量重建任务，请重试".into());
        }
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始保存相似歌曲索引失败：{error}"))?;
        transaction
            .execute("DELETE FROM song_similarity_pairs", [])
            .map_err(|error| format!("清理相似歌曲索引失败：{error}"))?;
        {
            let mut insert = transaction
                .prepare(
                    "INSERT INTO song_similarity_pairs
                       (left_recording_id, right_recording_id, evidence_source_recording_id,
                        evidence_kind, score, lyrics_similarity, evidence_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .map_err(|error| format!("准备保存相似歌曲索引失败：{error}"))?;
            for pair in &pairs {
                let evidence_kind = match pair.evidence.kind {
                    song_manager::SongCandidateKind::Metadata => 0,
                    song_manager::SongCandidateKind::SharedIdentifier => 1,
                    song_manager::SongCandidateKind::OriginalRelation => 2,
                };
                insert
                    .execute(params![
                        pair.songs[0].recording_id.min(pair.songs[1].recording_id),
                        pair.songs[0].recording_id.max(pair.songs[1].recording_id),
                        pair.evidence_source_recording_id,
                        evidence_kind,
                        pair.evidence.score,
                        pair.evidence.lyrics_similarity,
                        serde_json::to_string(pair).unwrap_or_else(|_| "{}".into()),
                    ])
                    .map_err(|error| format!("保存相似歌曲索引失败：{error}"))?;
            }
        }
        transaction
            .execute(
                "INSERT INTO library_similarity_state(index_kind, entity_id, generation)
                 SELECT 'song_similarity', entity_id, generation FROM library_index_dirty
                 WHERE index_kind='song'
                 ON CONFLICT(index_kind, entity_id) DO UPDATE SET generation=excluded.generation",
                [],
            )
            .map_err(|error| format!("保存相似歌曲索引水位失败：{error}"))?;
        transaction
            .execute(
                "UPDATE library_index_state SET phase='ready', processed=total,
                        revision=revision+1, settings_fingerprint=?1, last_error=NULL
                 WHERE index_kind='song_similarity'",
                params![settings_fingerprint],
            )
            .map_err(|error| format!("完成相似歌曲索引失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交相似歌曲索引失败：{error}"))?;
        log::debug!(
            "相似歌曲索引完成：recordings={} candidates={}/{} pairs={} elapsed_ms={}",
            recording_count,
            candidate_count,
            possible_count,
            pairs.len(),
            started.elapsed().as_millis()
        );
        Ok(pairs)
    }

    pub fn dismiss_library_song_similarity(
        &self,
        left_recording_id: i64,
        right_recording_id: i64,
    ) -> Result<(), String> {
        if left_recording_id == right_recording_id {
            return Err("需要选择两首不同的歌曲".into());
        }
        let (left_recording_id, right_recording_id) = if left_recording_id < right_recording_id {
            (left_recording_id, right_recording_id)
        } else {
            (right_recording_id, left_recording_id)
        };
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let existing_count = connection
            .query_row(
                "SELECT COUNT(*) FROM recordings WHERE recording_id IN (?1, ?2)",
                params![left_recording_id, right_recording_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("读取相似歌曲失败：{error}"))?;
        if existing_count != 2 {
            return Err("相似歌曲已不存在，请刷新后重试".into());
        }
        connection
            .execute(
                "INSERT OR IGNORE INTO song_similarity_ignores
                   (left_recording_id, right_recording_id) VALUES (?1, ?2)",
                params![left_recording_id, right_recording_id],
            )
            .map_err(|error| format!("保存相似歌曲决定失败：{error}"))?;
        connection
            .execute(
                "DELETE FROM song_similarity_pairs
                 WHERE left_recording_id=?1 AND right_recording_id=?2",
                params![left_recording_id, right_recording_id],
            )
            .map_err(|error| format!("更新相似歌曲缓存失败：{error}"))?;
        Ok(())
    }
}

fn ignored_song_similarity(connection: &Connection, left: i64, right: i64) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM song_similarity_ignores
               WHERE left_recording_id=?1 AND right_recording_id=?2
             )",
            params![left, right],
            |row| row.get::<_, bool>(0),
        )
        .unwrap_or(false)
}

fn song_similarity_settings_fingerprint(
    settings: &crate::lyrics::provider::ProviderSettings,
) -> String {
    let mut hasher = DefaultHasher::new();
    SONG_SIMILARITY_ALGORITHM_VERSION.hash(&mut hasher);
    settings.match_weights.hash(&mut hasher);
    settings.normalize_chinese.hash(&mut hasher);
    settings.title_filter_keywords.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn stronger_song_similarity_evidence(
    candidate: &SongAssociationCandidate,
    current: &SongAssociationCandidate,
) -> bool {
    candidate.kind > current.kind
        || (candidate.kind == current.kind
            && (candidate.score > current.score
                || (candidate.score == current.score
                    && candidate.lyrics_similarity.unwrap_or_default()
                        > current.lyrics_similarity.unwrap_or_default())))
}

fn recommended_song_recording_id(songs: &[LibrarySongSummary; 2]) -> i64 {
    let quality = |song: &LibrarySongSummary| {
        let metadata_count = u8::from(song.album.is_some())
            + u8::from(song.duration_ms.is_some())
            + u8::from(!song.artists.is_empty());
        (
            song.source_count,
            song.lyric_count,
            song.default_lyric.is_some(),
            metadata_count,
        )
    };
    match quality(&songs[0]).cmp(&quality(&songs[1])) {
        std::cmp::Ordering::Greater => songs[0].recording_id,
        std::cmp::Ordering::Less => songs[1].recording_id,
        std::cmp::Ordering::Equal => songs[0].recording_id.min(songs[1].recording_id),
    }
}
