use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};

use super::SPOTIFY_DJ_CLEANUP_SCHEMA_VERSION;

struct DjObservation {
    observation_id: i64,
    track_key: String,
    title: String,
}

/// 删除 DJ 口播的来源观察；歌曲还包含正常来源时只修正明显来自口播的展示元数据。
pub(super) fn remove_spotify_dj_interludes(connection: &mut Connection) -> rusqlite::Result<()> {
    cleanup_spotify_dj_interludes(connection, SPOTIFY_DJ_CLEANUP_SCHEMA_VERSION)
}

/// 第 21 版迁移复用清理逻辑，移除第 20 版迁移后由旧数据重放的口播。
pub(super) fn cleanup_spotify_dj_interludes(
    connection: &mut Connection,
    schema_version: i64,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    let mut dj_observations = HashMap::<i64, Vec<DjObservation>>::new();
    {
        let mut statement = transaction.prepare(
            "SELECT observation_id, recording_id, track_key, raw_title, raw_artists_json
             FROM track_observations
             WHERE platform='spotify' AND track_key LIKE 'spotify:spotify:media:%'",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        for row in rows {
            let (observation_id, recording_id, track_key, title, artists_json) = row?;
            let artists = serde_json::from_str::<Vec<String>>(&artists_json).unwrap_or_default();
            if artists.iter().any(|artist| {
                track_key
                    .strip_prefix("spotify:")
                    .is_some_and(|id| crate::spotify_media::is_dj_interlude(id, artist))
            }) {
                dj_observations
                    .entry(recording_id)
                    .or_default()
                    .push(DjObservation {
                        observation_id,
                        track_key,
                        title,
                    });
            }
        }
    }

    for (recording_id, observations) in dj_observations {
        let observation_count = transaction.query_row(
            "SELECT COUNT(*) FROM track_observations WHERE recording_id=?1",
            params![recording_id],
            |row| row.get::<_, i64>(0),
        )?;
        if observation_count != observations.len() as i64 {
            for observation in &observations {
                transaction.execute(
                    "DELETE FROM recording_external_ids
                     WHERE recording_id=?1 AND namespace='spotify' AND id_kind='track_id'
                       AND value=?2",
                    params![
                        recording_id,
                        observation.track_key.strip_prefix("spotify:").unwrap_or("")
                    ],
                )?;
                transaction.execute(
                    "UPDATE lyrics_search_runs SET recording_id=NULL
                     WHERE recording_id=?1 AND track_key=?2",
                    params![recording_id, observation.track_key],
                )?;
                transaction.execute(
                    "DELETE FROM track_observations WHERE observation_id=?1",
                    params![observation.observation_id],
                )?;
            }
            transaction.execute(
                "DELETE FROM platform_lyric_overrides
                 WHERE recording_id=?1 AND platform='spotify'
                   AND NOT EXISTS (SELECT 1 FROM track_observations
                                   WHERE recording_id=?1 AND platform='spotify')",
                params![recording_id],
            )?;

            let current_title = transaction.query_row(
                "SELECT title FROM recordings WHERE recording_id=?1",
                params![recording_id],
                |row| row.get::<_, String>(0),
            )?;
            let non_dj_credits = transaction.query_row(
                "SELECT COUNT(*) FROM recording_artist_credits
                 WHERE recording_id=?1 AND lower(trim(raw_name))!='dj x'",
                params![recording_id],
                |row| row.get::<_, i64>(0),
            )?;
            if non_dj_credits == 0
                && observations
                    .iter()
                    .any(|item| item.title.eq_ignore_ascii_case(&current_title))
            {
                let replacement = transaction
                    .query_row(
                        "SELECT raw_title, raw_artists_json, raw_album, duration_ms
                         FROM track_observations WHERE recording_id=?1
                         ORDER BY observation_id LIMIT 1",
                        params![recording_id],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, Option<String>>(2)?,
                                row.get::<_, Option<i64>>(3)?,
                            ))
                        },
                    )
                    .optional()?;
                if let Some((title, artists_json, album, duration_ms)) = replacement {
                    let artists =
                        serde_json::from_str::<Vec<String>>(&artists_json).unwrap_or_default();
                    let version_tags = serde_json::to_string(
                        &crate::lyrics::provider::version_tags_from_title(&title),
                    )
                    .unwrap_or_else(|_| "[]".into());
                    transaction.execute(
                        "UPDATE recordings SET title=?2, album=?3, duration_ms=?4,
                         version_tags_json=?5 WHERE recording_id=?1",
                        params![recording_id, title, album, duration_ms, version_tags],
                    )?;
                    if !artists.iter().all(|artist| artist.trim().is_empty()) {
                        transaction.execute(
                            "DELETE FROM recording_artist_credits WHERE recording_id=?1",
                            params![recording_id],
                        )?;
                    }
                    let mut credit_order = 0_i64;
                    for artist in &artists {
                        if artist.trim().is_empty() {
                            continue;
                        }
                        let existing_artist_id = transaction
                            .query_row(
                                "SELECT artist_id FROM artists WHERE canonical_name=?1
                                 ORDER BY artist_id LIMIT 1",
                                params![artist],
                                |row| row.get::<_, i64>(0),
                            )
                            .optional()?;
                        let artist_id = if let Some(artist_id) = existing_artist_id {
                            artist_id
                        } else {
                            transaction.execute(
                                "INSERT INTO artists (canonical_name) VALUES (?1)",
                                params![artist],
                            )?;
                            transaction.last_insert_rowid()
                        };
                        transaction.execute(
                            "INSERT INTO recording_artist_credits
                               (recording_id, artist_id, raw_name, credit_order, role)
                             VALUES (?1, ?2, ?3, ?4, ?5)",
                            params![
                                recording_id,
                                artist_id,
                                artist,
                                credit_order,
                                if credit_order == 0 {
                                    "lead"
                                } else {
                                    "featured"
                                }
                            ],
                        )?;
                        credit_order += 1;
                    }
                }
            }
            continue;
        }
        transaction.execute(
            "UPDATE track_observations SET split_from_recording_id=NULL
             WHERE split_from_recording_id=?1",
            params![recording_id],
        )?;
        transaction.execute(
            "UPDATE lyrics_search_runs SET recording_id=NULL WHERE recording_id=?1",
            params![recording_id],
        )?;
        for table in [
            "platform_lyric_overrides",
            "recording_lyric_bindings",
            "recording_external_ids",
            "recording_artist_credits",
            "track_observations",
        ] {
            transaction.execute(
                &format!("DELETE FROM {table} WHERE recording_id=?1"),
                params![recording_id],
            )?;
        }
        transaction.execute(
            "DELETE FROM song_similarity_ignores
             WHERE left_recording_id=?1 OR right_recording_id=?1",
            params![recording_id],
        )?;
        transaction.execute(
            "DELETE FROM recordings WHERE recording_id=?1",
            params![recording_id],
        )?;
    }
    transaction.execute("DELETE FROM song_similarity_pairs", [])?;
    transaction.execute(
        "DELETE FROM library_similarity_state WHERE index_kind='song_similarity'",
        [],
    )?;
    transaction.execute(
        "UPDATE library_index_state
         SET phase='pending', processed=0, total=0, settings_fingerprint=NULL, last_error=NULL
         WHERE index_kind='song_similarity'",
        [],
    )?;
    transaction.execute_batch(&format!("PRAGMA user_version = {schema_version};"))?;
    transaction.commit()
}
