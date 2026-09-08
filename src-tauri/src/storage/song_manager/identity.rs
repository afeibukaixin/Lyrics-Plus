use super::models::RecordingView;
use crate::lyrics::provider::version_tags_from_title;
use crate::storage::{ExternalIdentifier, Storage};
use rusqlite::OptionalExtension;

impl Storage {
    /// 记录播放器当前观察到的曲目；首次观察只创建独立 Recording，不做跨平台自动合并。
    pub(crate) fn observe_track(
        &self,
        track_key: &str,
        platform: &str,
        external_id: Option<&str>,
        title: &str,
        artists: &[String],
        album: Option<&str>,
        duration_ms: Option<u64>,
    ) -> Result<i64, String> {
        let track_key = track_key.trim();
        let platform = platform.trim();
        let title = title.trim();
        let artists = artists
            .iter()
            .map(|artist| artist.trim())
            .filter(|artist| !artist.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if track_key.is_empty() || platform.is_empty() || title.is_empty() || artists.is_empty() {
            return Err("记录歌曲观察需要曲目标识、平台、歌曲名和歌手".into());
        }
        let raw_artists_json = serde_json::to_string(&artists)
            .map_err(|error| format!("序列化歌手列表失败：{error}"))?;
        let duration_ms = duration_ms
            .map(|value| value.min(i64::MAX as u64) as i64)
            .filter(|value| *value >= 0);
        let album = album.map(str::trim).filter(|value| !value.is_empty());
        let external = external_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| external_id_descriptor(platform, value));

        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始记录歌曲观察失败：{error}"))?;
        let observed_recording_id = transaction
            .query_row(
                "SELECT recording_id FROM track_observations
                 WHERE platform=?1 AND track_key=?2",
                rusqlite::params![platform, track_key],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("读取歌曲观察失败：{error}"))?;
        let external_recording_id = external.as_ref().and_then(|(id_kind, value)| {
            transaction
                .query_row(
                    "SELECT recording_id FROM recording_external_ids
                         WHERE namespace=?1 AND id_kind=?2 AND value=?3 AND confirmed=1",
                    rusqlite::params![platform, id_kind, value],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .ok()
                .flatten()
        });
        let recording_id = if let Some(recording_id) = observed_recording_id {
            recording_id
        } else if let Some(recording_id) = external_recording_id {
            let occupied = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM track_observations
                               WHERE recording_id=?1 AND platform=?2 AND track_key!=?3)",
                    rusqlite::params![recording_id, platform, track_key],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(|error| format!("校验平台曲目冲突失败：{error}"))?;
            if occupied {
                return Err("已确认的歌曲关系存在同平台曲目冲突，请先移出多余曲目".into());
            }
            recording_id
        } else {
            let version_tags = serde_json::to_string(&version_tags_from_title(title))
                .map_err(|error| format!("序列化歌曲版本标签失败：{error}"))?;
            transaction
                .execute(
                    "INSERT INTO recordings (title, album, duration_ms, version_tags_json)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![title, album, duration_ms, version_tags],
                )
                .map_err(|error| format!("创建歌曲实体失败：{error}"))?;
            let recording_id = transaction.last_insert_rowid();
            insert_artist_credits(&transaction, recording_id, &artists)?;
            recording_id
        };

        transaction
            .execute(
                "INSERT INTO track_observations
                   (track_key, platform, raw_title, raw_artists_json, raw_album, duration_ms,
                    recording_id, observed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch())
                 ON CONFLICT(platform, track_key) DO UPDATE SET
                   raw_title=excluded.raw_title,
                   raw_artists_json=excluded.raw_artists_json,
                   raw_album=excluded.raw_album,
                   duration_ms=excluded.duration_ms,
                   recording_id=excluded.recording_id,
                   observed_at=unixepoch()",
                rusqlite::params![
                    track_key,
                    platform,
                    title,
                    raw_artists_json,
                    album,
                    duration_ms,
                    recording_id
                ],
            )
            .map_err(|error| format!("保存歌曲观察失败：{error}"))?;
        if let Some((id_kind, value)) = external {
            transaction
                .execute(
                    "INSERT INTO recording_external_ids
                       (namespace, id_kind, value, recording_id, confidence, confirmed)
                     VALUES (?1, ?2, ?3, ?4, 100, 0)
                    ON CONFLICT(namespace, id_kind, value) DO UPDATE SET
                       recording_id=excluded.recording_id,
                       confidence=MAX(recording_external_ids.confidence, excluded.confidence),
                       updated_at=unixepoch()
                     WHERE recording_external_ids.recording_id=excluded.recording_id",
                    rusqlite::params![platform, id_kind, value, recording_id],
                )
                .map_err(|error| format!("保存歌曲平台 ID 失败：{error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("提交歌曲观察失败：{error}"))?;
        Ok(recording_id)
    }
}

pub(super) fn confirmed_artist_aliases(
    connection: &rusqlite::Connection,
) -> Result<Vec<(String, String)>, String> {
    // 平台新建的歌手实体可能只保存别名；从已确认等价表读取，评分时只使用
    // 与双方署名命中的等价名称，不能仅依赖 Recording 初次创建时的 artist_id。
    let mut statement = connection
        .prepare(
            "SELECT artist.canonical_name, alias.alias
             FROM artists AS artist
             JOIN artist_aliases AS alias
               ON alias.artist_id=artist.artist_id AND alias.confirmed=1
             ORDER BY artist.artist_id, alias.alias_id",
        )
        .map_err(|error| format!("准备已确认歌手别名查询失败：{error}"))?;
    let aliases = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("读取已确认歌手别名失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析已确认歌手别名失败：{error}"))?;
    Ok(aliases)
}

pub(super) fn normalized_identity_artist(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| character.to_lowercase())
        .filter(|character| character.is_alphanumeric())
        .collect()
}

pub(super) fn matched_aliases(
    aliases: &[(String, String)],
    current_artists: &[String],
    candidate_artists: &[String],
) -> Vec<String> {
    let current = current_artists
        .iter()
        .map(|name| normalized_identity_artist(name))
        .collect::<std::collections::HashSet<_>>();
    let candidate = candidate_artists
        .iter()
        .map(|name| normalized_identity_artist(name))
        .collect::<std::collections::HashSet<_>>();
    let mut groups = std::collections::HashMap::<String, Vec<&String>>::new();
    for (canonical, alias) in aliases {
        groups
            .entry(normalized_identity_artist(canonical))
            .or_default()
            .push(alias);
    }
    let mut matched = Vec::new();
    for (canonical, names) in groups {
        let contains = |artists: &std::collections::HashSet<String>| {
            artists.contains(&canonical)
                || names
                    .iter()
                    .any(|name| artists.contains(&normalized_identity_artist(name)))
        };
        if contains(&current) && contains(&candidate) {
            matched.extend(
                names
                    .into_iter()
                    .filter(|name| {
                        let normalized = normalized_identity_artist(name);
                        current.contains(&normalized) || candidate.contains(&normalized)
                    })
                    .cloned(),
            );
        }
    }
    matched.sort();
    matched.dedup();
    matched
}

pub(super) fn shared_recording_identifier(
    current: &[ExternalIdentifier],
    candidate: &[ExternalIdentifier],
) -> Option<ExternalIdentifier> {
    current
        .iter()
        .filter(|left| is_cross_platform_recording_identifier(left))
        .find_map(|left| {
            candidate
                .iter()
                .find(|right| {
                    is_cross_platform_recording_identifier(right)
                        && left.id_kind.eq_ignore_ascii_case(&right.id_kind)
                        && left.value.eq_ignore_ascii_case(&right.value)
                })
                .map(|_| left.clone())
        })
}

pub(super) fn is_cross_platform_recording_identifier(identifier: &ExternalIdentifier) -> bool {
    identifier.namespace.eq_ignore_ascii_case("isrc")
        || matches!(
            identifier.id_kind.to_ascii_lowercase().as_str(),
            "isrc"
                | "recording_id"
                | "recording"
                | "musicbrainz_recording_id"
                | "musicbrainz_recording"
        )
}

pub(super) fn insert_artist_credits(
    transaction: &rusqlite::Transaction<'_>,
    recording_id: i64,
    artists: &[String],
) -> Result<(), String> {
    for (credit_order, raw_name) in artists.iter().enumerate() {
        let artist_id = if let Some(artist_id) = transaction
            .query_row(
                "SELECT artist_id FROM artists WHERE canonical_name=?1 ORDER BY artist_id LIMIT 1",
                rusqlite::params![raw_name],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("读取歌手实体失败：{error}"))?
        {
            artist_id
        } else {
            transaction
                .execute(
                    "INSERT INTO artists (canonical_name) VALUES (?1)",
                    rusqlite::params![raw_name],
                )
                .map_err(|error| format!("创建歌手实体失败：{error}"))?;
            transaction.last_insert_rowid()
        };
        let role = if credit_order == 0 {
            "lead"
        } else {
            "featured"
        };
        transaction
            .execute(
                "INSERT INTO recording_artist_credits
                   (recording_id, artist_id, raw_name, credit_order, role)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![recording_id, artist_id, raw_name, credit_order, role],
            )
            .map_err(|error| format!("保存歌曲署名失败：{error}"))?;
    }
    Ok(())
}

pub(super) fn external_id_descriptor<'a>(
    platform: &str,
    value: &'a str,
) -> Option<(&'static str, &'a str)> {
    let id_kind = match platform {
        "spotify" => "track_id",
        "apple_music" => "persistent_id",
        "qqmusic" => "songmid",
        "netease" => "song_id",
        "kugou" => "file_hash",
        "kuwo" => "song_id",
        "migu" => "copyright_id",
        "musixmatch" => "track_id",
        "amll_ttml" => "track_id",
        "lrclib" => "track_id",
        "qishui" => "track_id",
        _ => return None,
    };
    Some((id_kind, value))
}

pub(crate) fn exact_track_external_id<'a>(
    platform: &str,
    track_key: &'a str,
) -> Option<(&'static str, &'a str)> {
    let prefix = format!("{platform}:");
    let value = track_key.strip_prefix(&prefix)?;
    if value.is_empty() || value.starts_with("fallback:") {
        return None;
    }
    external_id_descriptor(platform, value)
}

pub(super) fn validate_recording_action_part<'a>(
    value: &'a str,
    label: &str,
) -> Result<&'a str, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{label}不能为空"))
    } else {
        Ok(value)
    }
}

pub(super) fn recording_view(
    connection: &rusqlite::Connection,
    recording_id: i64,
) -> Result<RecordingView, String> {
    let (title, album, duration_ms, version_tags_json) = connection
        .query_row(
            "SELECT title, album, duration_ms, version_tags_json
             FROM recordings WHERE recording_id=?1",
            rusqlite::params![recording_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("读取歌曲实体失败：{error}"))?
        .ok_or_else(|| "歌曲实体不存在".to_string())?;
    let version_tags = recording_version_tags(&version_tags_json, &title);
    Ok(RecordingView {
        recording_id,
        title,
        album,
        duration_ms: duration_ms.and_then(|value| u64::try_from(value).ok()),
        version_tags,
    })
}

/// 兼容旧库把所有歌曲默认写成 `original` 的情况；只有标题明确声明版本时
/// 才用标题推导结果覆盖这个历史占位值。
pub(crate) fn recording_version_tags(version_tags_json: &str, title: &str) -> Vec<String> {
    let stored = serde_json::from_str::<Vec<String>>(version_tags_json).unwrap_or_default();
    let inferred = version_tags_from_title(title);
    if stored.is_empty() || (stored.len() == 1 && stored[0] == "original") {
        inferred
    } else {
        stored
    }
}

pub(super) fn version_tags_conflict(expected: &[String], actual: &[String]) -> bool {
    !expected.is_empty() && !actual.is_empty() && expected != actual
}
