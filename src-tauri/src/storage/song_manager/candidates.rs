use super::{identity::*, lyrics::*, models::*};
use crate::lyrics::provider::{
    recording_scoring_settings, score_candidate, score_evidence, version_conflict,
    version_tags_from_title, LyricsSearchInput, LyricsSearchResult, ProviderSettings,
};
use crate::storage::{load_external_identifiers, load_observations, Storage};
use rusqlite::OptionalExtension;

impl Storage {
    pub(crate) fn song_association_candidates(
        &self,
        platform: &str,
        track_key: &str,
        settings: &ProviderSettings,
    ) -> Result<Vec<SongAssociationCandidate>, String> {
        let platform = validate_recording_action_part(platform, "平台")?;
        let track_key = validate_recording_action_part(track_key, "曲目标识")?;
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        collect_song_association_candidates(&connection, platform, track_key, settings)
    }
}

/// 同一事务中也使用这个查询，保证展示规则与写入校验一致。
pub(in crate::storage) fn collect_song_association_candidates(
    connection: &rusqlite::Connection,
    platform: &str,
    track_key: &str,
    settings: &ProviderSettings,
) -> Result<Vec<SongAssociationCandidate>, String> {
    collect_song_association_candidates_filtered(
        connection, platform, track_key, settings, None, None,
    )
}

pub(in crate::storage) fn collect_song_association_candidates_for_target(
    connection: &rusqlite::Connection,
    platform: &str,
    track_key: &str,
    settings: &ProviderSettings,
    target_recording_id: Option<i64>,
) -> Result<Vec<SongAssociationCandidate>, String> {
    let target_recording_ids = target_recording_id.map(|id| vec![id]);
    collect_song_association_candidates_filtered(
        connection,
        platform,
        track_key,
        settings,
        target_recording_ids.as_deref(),
        None,
    )
}

pub(in crate::storage) fn collect_song_association_candidates_for_targets(
    connection: &rusqlite::Connection,
    platform: &str,
    track_key: &str,
    settings: &ProviderSettings,
    target_recording_ids: &[i64],
) -> Result<Vec<SongAssociationCandidate>, String> {
    collect_song_association_candidates_filtered(
        connection,
        platform,
        track_key,
        settings,
        Some(target_recording_ids),
        None,
    )
}

fn collect_song_association_candidates_filtered(
    connection: &rusqlite::Connection,
    platform: &str,
    track_key: &str,
    settings: &ProviderSettings,
    target_recording_ids: Option<&[i64]>,
    after_recording_id: Option<i64>,
) -> Result<Vec<SongAssociationCandidate>, String> {
    let current_id = connection
        .query_row(
            "SELECT recording_id FROM track_observations WHERE platform=?1 AND track_key=?2",
            rusqlite::params![platform, track_key],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("读取当前歌曲失败：{error}"))?
        .ok_or_else(|| "当前曲目尚未建立歌曲观察".to_string())?;
    let current = recording_view(connection, current_id)?;
    let current_observations = load_observations(connection, current_id)?;
    let current_ids = load_external_identifiers(connection, current_id)?;
    let confirmed_aliases = confirmed_artist_aliases(connection)?;
    let current_lyrics = load_default_lyric_binding(connection, current_id)?;
    let mut current_tags = current.version_tags.clone();
    for observation in &current_observations {
        current_tags.extend(version_tags_from_title(&observation.raw_title));
    }
    current_tags.sort();
    current_tags.dedup();
    // 只遍历已经观察到的本地歌曲，不截断在最近 200 首，避免漏掉历史原关联/ISRC。
    let candidate_ids = if let Some(target_recording_ids) = target_recording_ids {
        target_recording_ids
            .iter()
            .copied()
            .filter(|id| *id != current_id)
            .collect()
    } else {
        let mut statement = connection
            .prepare(
                "SELECT recording_id FROM recordings WHERE recording_id != ?1
             AND (?2 IS NULL OR recording_id > ?2)
             AND EXISTS (SELECT 1 FROM track_observations
                         WHERE track_observations.recording_id=recordings.recording_id)",
            )
            .map_err(|error| format!("准备歌曲候选失败：{error}"))?;
        let rows = statement
            .query_map(rusqlite::params![current_id, after_recording_id], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| format!("读取歌曲候选失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析歌曲候选失败：{error}"))?;
        rows
    };
    let mut candidates = Vec::new();
    for recording_id in candidate_ids {
        let candidate = recording_view(connection, recording_id)?;
        let observations = load_observations(connection, recording_id)?;
        let external_identifiers = load_external_identifiers(connection, recording_id)?;
        // 双向来源：被移出的歌曲在原歌曲中也应当优先展示。
        let is_original_recording = current_observations
            .iter()
            .any(|item| item.split_from_recording_id == Some(recording_id))
            || observations
                .iter()
                .any(|item| item.split_from_recording_id == Some(current_id));
        let shared_identifier = shared_recording_identifier(&current_ids, &external_identifiers);
        let kind = if is_original_recording {
            SongCandidateKind::OriginalRelation
        } else if shared_identifier.is_some() {
            SongCandidateKind::SharedIdentifier
        } else {
            SongCandidateKind::Metadata
        };
        let strong = kind != SongCandidateKind::Metadata;
        let mut aliases = confirmed_aliases.clone();
        aliases.sort();
        aliases.dedup();
        let scoring = recording_scoring_settings(settings, aliases.clone())?;
        let mut candidate_tags = candidate.version_tags.clone();
        for observation in &observations {
            candidate_tags.extend(version_tags_from_title(&observation.raw_title));
        }
        candidate_tags.sort();
        candidate_tags.dedup();
        let mut pairs = Vec::new();
        for current_observation in &current_observations {
            let input = LyricsSearchInput {
                title: current_observation.raw_title.clone(),
                artist: current_observation.raw_artists.join(" / "),
                album: current_observation
                    .raw_album
                    .clone()
                    .or(current.album.clone()),
                duration_ms: current_observation.duration_ms.or(current.duration_ms),
                platform: Some(current_observation.platform.clone()),
                platform_item_id: None,
                scoring: scoring.clone(),
            };
            for observation in &observations {
                let result = LyricsSearchResult {
                    id: observation.track_key.clone(),
                    provider_id: observation.platform.clone(),
                    title: observation.raw_title.clone(),
                    artist: observation.raw_artists.join(" / "),
                    album: observation.raw_album.clone().or(candidate.album.clone()),
                    duration_ms: observation.duration_ms.or(candidate.duration_ms),
                    source: "recording".into(),
                    synced: false,
                    has_translation: false,
                    has_word_timing: false,
                    has_romanization: false,
                    score: 0.0,
                    lyrics: String::new(),
                };
                let evidence = score_evidence(&input, &result);
                let delta = input
                    .duration_ms
                    .zip(result.duration_ms)
                    .map(|(a, b)| a.abs_diff(b));
                // 固定门槛不读取权重。未知时长仅标记缺失，不假装为相同。
                let gates = AssociationGates {
                    title: evidence.title_similarity >= 0.90,
                    artist: evidence.artist_similarity >= 0.80 && !evidence.artist_main_conflict,
                    duration: delta.is_none_or(|value| value <= 10_000),
                    version: !version_conflict(&input.title, &result.title)
                        && !version_tags_conflict(&current_tags, &candidate_tags),
                    current_observation_id: current_observation.observation_id,
                    candidate_observation_id: observation.observation_id,
                };
                if strong || (gates.title && gates.artist && gates.duration && gates.version) {
                    pairs.push((
                        score_candidate(&input, &result),
                        evidence,
                        gates,
                        delta,
                        matched_aliases(
                            &aliases,
                            &current_observation.raw_artists,
                            &observation.raw_artists,
                        ),
                    ));
                }
            }
        }
        let Some((score, evidence, gates, duration_delta_ms, confirmed_artist_aliases)) =
            pairs.into_iter().max_by(|a, b| {
                let valid = |g: &AssociationGates| g.title && g.artist && g.duration && g.version;
                valid(&a.2)
                    .cmp(&valid(&b.2))
                    .then_with(|| a.0.total_cmp(&b.0))
            })
        else {
            continue;
        };
        let mut conflicts = Vec::new();
        for (passed, reason) in [
            (gates.title, "title"),
            (gates.artist, "artist"),
            (gates.duration, "duration"),
            (gates.version, "version"),
        ] {
            if !passed {
                conflicts.push(reason.to_string());
            }
        }
        let mut platforms = std::collections::HashSet::new();
        let platform_conflict = current_observations
            .iter()
            .chain(&observations)
            .filter(|item| !item.platform.eq_ignore_ascii_case("system"))
            .any(|item| !platforms.insert(item.platform.to_ascii_lowercase()));
        if platform_conflict {
            conflicts.push("platform".into());
        }
        let mut warnings = Vec::new();
        if duration_delta_ms.is_none() {
            warnings.push("durationMissing".into());
        }
        if current_tags.is_empty() || candidate_tags.is_empty() {
            warnings.push("versionMissing".into());
        }
        let candidate_lyrics = load_default_lyric_binding(connection, recording_id)?;
        let lyric_evidence = current_lyrics
            .as_ref()
            .zip(candidate_lyrics.as_ref())
            .and_then(|(a, b)| lyric_content_similarity(connection, a.asset_id, b.asset_id));
        let (lyrics_similarity, lyrics_content_same) =
            lyric_evidence.map_or((None, false), |(score, same)| (Some(score), same));
        let mut match_reasons = Vec::new();
        if is_original_recording {
            match_reasons.push("original_relation".into());
        }
        if shared_identifier.is_some() {
            match_reasons.push("isrc".into());
        }
        if gates.title {
            match_reasons.push("title".into());
        }
        if gates.artist {
            match_reasons.push("artist".into());
        }
        if !confirmed_artist_aliases.is_empty() {
            match_reasons.push("artist_alias".into());
        }
        if lyrics_content_same {
            match_reasons.push("lyrics".into());
        }
        let isrc = external_identifiers
            .iter()
            .find(|id| {
                id.id_kind.eq_ignore_ascii_case("isrc") || id.namespace.eq_ignore_ascii_case("isrc")
            })
            .map(|id| id.value.clone());
        candidates.push(SongAssociationCandidate {
            recording_id,
            title: candidate.title,
            album: candidate.album,
            duration_ms: candidate.duration_ms,
            version_tags: candidate_tags,
            observations,
            external_identifiers,
            is_original_recording,
            title_similarity: evidence.title_similarity,
            artist_similarity: evidence.artist_similarity,
            album_similarity: evidence.album_similarity,
            duration_delta_ms,
            lyrics_similarity,
            isrc,
            shared_identifier,
            confirmed_artist_aliases,
            score,
            match_reasons,
            kind,
            gates,
            score_weights: settings.match_weights,
            conflicts,
            warnings,
            can_associate: !platform_conflict,
            lyrics_content_same,
        });
    }
    candidates.sort_by(|a, b| {
        b.kind
            .cmp(&a.kind)
            .then_with(|| b.score.total_cmp(&a.score))
            .then_with(|| {
                b.lyrics_similarity
                    .unwrap_or_default()
                    .total_cmp(&a.lyrics_similarity.unwrap_or_default())
            })
            .then_with(|| {
                b.observations
                    .iter()
                    .map(|o| o.observed_at)
                    .max()
                    .cmp(&a.observations.iter().map(|o| o.observed_at).max())
            })
    });
    Ok(candidates)
}
