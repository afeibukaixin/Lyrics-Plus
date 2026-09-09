use super::{
    candidates::collect_song_association_candidates_for_target, identity::*, lyrics::*, models::*,
};
use crate::lyrics::provider::{version_tags_from_title, ProviderSettings};
use crate::storage::{load_observations, Storage};
use rusqlite::OptionalExtension;

impl Storage {
    /// 将候选歌曲组关联到当前歌曲，所有平台关系移动在同一事务内完成。
    pub(crate) fn associate_song_candidate(
        &self,
        platform: &str,
        track_key: &str,
        candidate_recording_id: i64,
        settings: &ProviderSettings,
    ) -> Result<RecordingView, String> {
        let platform = validate_recording_action_part(platform, "平台")?;
        let track_key = validate_recording_action_part(track_key, "曲目标识")?;
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始关联歌曲失败：{error}"))?;
        // 事务内重新校验候选，避免详情打开后候选已发生变化仍执行旧操作。
        let candidates = collect_song_association_candidates_for_target(
            &transaction,
            platform,
            track_key,
            settings,
            Some(candidate_recording_id),
        )?;
        let candidate = candidates
            .into_iter()
            .find(|candidate| candidate.recording_id == candidate_recording_id)
            .ok_or_else(|| "候选歌曲已不存在或不满足关联条件，请刷新后重试".to_string())?;
        if !candidate.can_associate {
            return Err("存在同平台曲目冲突，请先移出多余曲目".into());
        }
        let current_recording_id = transaction
            .query_row(
                "SELECT recording_id FROM track_observations
                 WHERE platform=?1 AND track_key=?2",
                rusqlite::params![platform, track_key],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("读取当前歌曲失败：{error}"))?;
        let current_lyrics = load_default_lyric_binding(&transaction, current_recording_id)?;
        let candidate_lyrics = load_default_lyric_binding(&transaction, candidate.recording_id)?;

        normalize_platform_lyrics(&transaction, current_recording_id)?;
        normalize_platform_lyrics(&transaction, candidate.recording_id)?;
        merge_lyric_bindings(&transaction, current_recording_id, candidate.recording_id)?;
        transaction
            .execute(
                "UPDATE track_observations
                 SET recording_id=?2, split_from_recording_id=NULL, observed_at=unixepoch()
                 WHERE recording_id=?1",
                rusqlite::params![candidate.recording_id, current_recording_id],
            )
            .map_err(|error| format!("迁移歌曲平台关系失败：{error}"))?;
        // 选择任意其他候选后，当前独立版本与原歌曲的恢复关系一并解除。
        // 候选为原歌曲时也必须清除该标记，避免删除来源实体后留下悬空外键。
        transaction
            .execute(
                "UPDATE track_observations
                 SET split_from_recording_id=NULL
                 WHERE recording_id=?1",
                rusqlite::params![current_recording_id],
            )
            .map_err(|error| format!("清除独立歌曲来源关系失败：{error}"))?;
        move_external_ids(&transaction, candidate.recording_id, current_recording_id)?;
        // 只确认当前及候选各平台观察对应的精确 ID；ISRC 或歌词来源等
        // 其他外部证据继续保留原确认状态，避免关联操作扩大确认范围。
        relink_external_id(
            &transaction,
            platform,
            track_key,
            current_recording_id,
            true,
        )?;
        for observation in load_observations(&transaction, current_recording_id)? {
            relink_external_id(
                &transaction,
                &observation.platform,
                &observation.track_key,
                current_recording_id,
                true,
            )?;
        }
        transaction
            .execute(
                "UPDATE lyrics_search_runs SET recording_id=?2 WHERE recording_id=?1",
                rusqlite::params![candidate.recording_id, current_recording_id],
            )
            .map_err(|error| format!("迁移歌词搜索身份失败：{error}"))?;

        // 当前歌曲已有歌词时保留它；当前没有而候选有默认歌词时采用候选歌词。
        if current_lyrics.is_none() && candidate_lyrics.is_some() {
            set_shared_lyrics_default(
                &transaction,
                current_recording_id,
                candidate_lyrics.as_ref(),
            )?;
        }
        move_platform_overrides(&transaction, candidate.recording_id, current_recording_id)?;
        // 合并会改变保留歌曲的身份内容，之前涉及两端的“保持分开”判断需要重新审核。
        transaction
            .execute(
                "DELETE FROM song_similarity_ignores
                 WHERE left_recording_id IN (?1, ?2) OR right_recording_id IN (?1, ?2)",
                rusqlite::params![current_recording_id, candidate.recording_id],
            )
            .map_err(|error| format!("清理相似歌曲忽略记录失败：{error}"))?;
        // 候选歌曲可能曾经作为其他独立版本的来源；候选实体清理前解除这些旧来源引用，
        // 避免留下悬空的 split_from_recording_id。
        transaction
            .execute(
                "UPDATE track_observations
                 SET split_from_recording_id=NULL
                 WHERE split_from_recording_id=?1",
                rusqlite::params![candidate.recording_id],
            )
            .map_err(|error| format!("清理候选歌曲来源关系失败：{error}"))?;
        transaction
            .execute(
                "DELETE FROM recording_lyric_bindings WHERE recording_id=?1",
                rusqlite::params![candidate.recording_id],
            )
            .map_err(|error| format!("清理候选歌曲歌词绑定失败：{error}"))?;
        transaction
            .execute(
                "DELETE FROM recording_artist_credits WHERE recording_id=?1",
                rusqlite::params![candidate.recording_id],
            )
            .map_err(|error| format!("清理候选歌曲署名失败：{error}"))?;
        transaction
            .execute(
                "DELETE FROM recordings WHERE recording_id=?1",
                rusqlite::params![candidate.recording_id],
            )
            .map_err(|error| format!("清理空歌曲实体失败：{error}"))?;
        let view = recording_view(&transaction, current_recording_id)?;
        transaction
            .commit()
            .map_err(|error| format!("提交歌曲关联失败：{error}"))?;
        Ok(view)
    }

    /// 将当前播放器曲目拆到新 Recording，并记录来源以便后续恢复。
    pub(crate) fn detach_platform_track(
        &self,
        platform: &str,
        track_key: &str,
        inherit_current_lyrics: bool,
    ) -> Result<RecordingView, String> {
        let platform = validate_recording_action_part(platform, "平台")?;
        let track_key = validate_recording_action_part(track_key, "曲目标识")?;
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始拆分歌曲失败：{error}"))?;
        let (
            source_recording_id,
            _split_from_recording_id,
            title,
            album,
            duration_ms,
            _version_tags_json,
            observation_title,
            observation_album,
            observation_duration_ms,
        ) = transaction
            .query_row(
                "SELECT observation.recording_id, observation.split_from_recording_id,
                        recording.title, recording.album, recording.duration_ms,
                        recording.version_tags_json, observation.raw_title,
                        observation.raw_album, observation.duration_ms
                 FROM track_observations AS observation
                 JOIN recordings AS recording ON recording.recording_id=observation.recording_id
                 WHERE observation.platform=?1 AND observation.track_key=?2",
                rusqlite::params![platform, track_key],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("读取待拆分歌曲失败：{error}"))?
            .ok_or_else(|| "当前曲目尚未建立歌曲观察，无法拆分".to_string())?;
        let observation_count = transaction
            .query_row(
                "SELECT COUNT(*) FROM track_observations WHERE recording_id=?1",
                rusqlite::params![source_recording_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("读取歌曲平台映射数量失败：{error}"))?;
        if observation_count < 2 {
            return Err("当前歌曲没有其他平台映射，无需拆分".into());
        }
        // 独立实体代表被移出的具体平台版本；优先采用该平台观察到的原始元数据，
        // 缺失字段再回退到拆分前歌曲实体的规范值。
        let title = if observation_title.trim().is_empty() {
            title
        } else {
            observation_title
        };
        let album = observation_album.or(album);
        let duration_ms = observation_duration_ms.or(duration_ms);
        let version_tags_json = serde_json::to_string(&version_tags_from_title(&title))
            .map_err(|error| format!("序列化独立歌曲版本标签失败：{error}"))?;
        // 平台偏移/覆盖属于被移出的平台观察；先补齐旧数据中的绑定，再随观察
        // 一起移动。选择“暂不绑定歌词”时只保留偏移，不把旧平台歌词带入新歌曲。
        normalize_platform_lyrics(&transaction, source_recording_id)?;
        let lyric_binding = if inherit_current_lyrics {
            Some(load_current_lyric_binding(
                &transaction,
                source_recording_id,
                platform,
            )?)
        } else {
            None
        };
        transaction
            .execute(
                "INSERT INTO recordings (title, album, duration_ms, version_tags_json)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![title, album, duration_ms, version_tags_json],
            )
            .map_err(|error| format!("创建独立歌曲失败：{error}"))?;
        let new_recording_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO recording_artist_credits
                   (recording_id, artist_id, raw_name, credit_order, role)
                 SELECT ?2, artist_id, raw_name, credit_order, role
                 FROM recording_artist_credits WHERE recording_id=?1",
                rusqlite::params![source_recording_id, new_recording_id],
            )
            .map_err(|error| format!("复制歌曲署名失败：{error}"))?;
        transaction
            .execute(
                "UPDATE track_observations
                 SET recording_id=?3, split_from_recording_id=?4, observed_at=unixepoch()
                 WHERE platform=?1 AND track_key=?2",
                rusqlite::params![platform, track_key, new_recording_id, source_recording_id],
            )
            .map_err(|error| format!("拆分歌曲关联失败：{error}"))?;
        // 保留旧表绑定供原歌曲兼容读取；拆分后的曲目带有 split 标记，
        // associations::load_with_status 会阻断旧表回退，避免把旧歌词泄漏到独立歌曲。
        // 歌曲管理操作保留偏移记录作为管理状态，重新关联后也不再回退旧行。
        // 唯一平台观察时，其命名空间下标识随曲目移动；旧重复平台只移精确 ID，
        // 无法确定归属的其他标识保留在原歌曲，避免误移另一个曲目的 ID。
        transaction
            .execute(
                "UPDATE recording_external_ids SET recording_id=?3, updated_at=unixepoch()
             WHERE recording_id=?1 AND namespace=?2
               AND NOT EXISTS (SELECT 1 FROM track_observations
                               WHERE recording_id=?1 AND platform=?2)",
                rusqlite::params![source_recording_id, platform, new_recording_id],
            )
            .map_err(|error| format!("迁移平台标识失败：{error}"))?;
        if let Some((kind, value)) = exact_track_external_id(platform, track_key) {
            transaction
                .execute(
                    "UPDATE recording_external_ids SET recording_id=?4, updated_at=unixepoch()
                 WHERE recording_id=?1 AND namespace=?2 AND id_kind=?3 AND value=?5",
                    rusqlite::params![source_recording_id, platform, kind, new_recording_id, value],
                )
                .map_err(|error| format!("迁移精确曲目 ID 失败：{error}"))?;
        }
        relink_external_id(&transaction, platform, track_key, new_recording_id, false)?;
        if let Some((asset_id, selection_source, confidence, evidence_json, offset_ms)) =
            lyric_binding
        {
            transaction
                .execute(
                    "INSERT INTO recording_lyric_bindings
                       (recording_id, asset_id, selection_source, confidence, evidence_json,
                        is_default, offset_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
                    rusqlite::params![
                        new_recording_id,
                        asset_id,
                        selection_source,
                        confidence,
                        evidence_json,
                        offset_ms
                    ],
                )
                .map_err(|error| format!("沿用当前歌词失败：{error}"))?;
        }
        move_platform_override(
            &transaction,
            source_recording_id,
            new_recording_id,
            platform,
        )?;
        transaction
            .execute(
                "UPDATE lyrics_search_runs SET recording_id=?2 WHERE track_key=?1",
                rusqlite::params![track_key, new_recording_id],
            )
            .map_err(|error| format!("更新歌词搜索身份失败：{error}"))?;
        let view = recording_view(&transaction, new_recording_id)?;
        transaction
            .commit()
            .map_err(|error| format!("提交歌曲拆分失败：{error}"))?;
        Ok(view)
    }
}

pub(super) fn relink_external_id(
    transaction: &rusqlite::Transaction<'_>,
    platform: &str,
    track_key: &str,
    recording_id: i64,
    confirmed: bool,
) -> Result<(), String> {
    let Some((id_kind, value)) = exact_track_external_id(platform, track_key) else {
        return Ok(());
    };
    let changed = transaction
        .execute(
            "UPDATE recording_external_ids
             SET recording_id=?4,
                 confidence=CASE WHEN ?5=1 THEN 100 ELSE confidence END,
                 confirmed=MAX(confirmed, ?5), updated_at=unixepoch()
             WHERE namespace=?1 AND id_kind=?2 AND value=?3 AND recording_id=?4",
            rusqlite::params![
                platform,
                id_kind,
                value,
                recording_id,
                if confirmed { 1_i64 } else { 0_i64 }
            ],
        )
        .map_err(|error| format!("更新歌曲平台 ID 关联失败：{error}"))?;
    if changed == 0 && confirmed {
        transaction
            .execute(
                "INSERT INTO recording_external_ids
                   (namespace, id_kind, value, recording_id, confidence, confirmed)
                 VALUES (?1, ?2, ?3, ?4, 100, 1)",
                rusqlite::params![platform, id_kind, value, recording_id],
            )
            .map_err(|error| format!("保存已确认的平台 ID 关联失败：{error}"))?;
    }
    Ok(())
}

pub(super) fn move_external_ids(
    transaction: &rusqlite::Transaction<'_>,
    source_recording_id: i64,
    target_recording_id: i64,
) -> Result<(), String> {
    transaction
        .execute(
            "DELETE FROM recording_external_ids
             WHERE external_id IN (
               SELECT source.external_id
               FROM recording_external_ids AS source
               JOIN recording_external_ids AS target
                 ON target.namespace=source.namespace
                AND target.id_kind=source.id_kind
                AND target.value=source.value
              WHERE source.recording_id=?1 AND target.recording_id=?2
             )",
            rusqlite::params![source_recording_id, target_recording_id],
        )
        .map_err(|error| format!("清理重复歌曲外部 ID 失败：{error}"))?;
    transaction
        .execute(
            "UPDATE recording_external_ids
             SET recording_id=?2, updated_at=unixepoch()
             WHERE recording_id=?1",
            rusqlite::params![source_recording_id, target_recording_id],
        )
        .map_err(|error| format!("迁移歌曲外部 ID 失败：{error}"))?;
    Ok(())
}
