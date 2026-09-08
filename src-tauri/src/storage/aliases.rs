fn normalize_identity_component(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn normalized_optional(value: Option<&str>) -> Option<String> {
    value
        .map(normalize_identity_component)
        .filter(|value| !value.is_empty())
}

fn duration_from_track_key(track_key: &str) -> Option<i64> {
    track_key
        .rsplit('|')
        .next()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
}

impl Storage {
    pub(crate) fn migrate_track_aliases(&self) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始迁移歌词关联别名失败：{error}"))?;
        let rows = {
            let mut statement = transaction
                .prepare(
                    "SELECT track_key, title, artist
                     FROM lyric_associations",
                )
                .map_err(|error| format!("读取歌词关联别名失败：{error}"))?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map_err(|error| format!("读取歌词关联别名失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析歌词关联别名失败：{error}"))?;
            rows
        };
        for (track_key, title, artist) in rows {
            transaction
                .execute(
                    "INSERT INTO lyric_track_aliases
                       (observed_track_key, canonical_track_key, title_norm, artist_norm,
                        album_norm, duration_ms, evidence_kind, updated_at)
                     VALUES (?1, ?1, ?2, ?3, NULL, ?4, 'unverified_legacy', unixepoch())
                     ON CONFLICT(observed_track_key) DO UPDATE SET
                       title_norm=excluded.title_norm, artist_norm=excluded.artist_norm,
                       duration_ms=COALESCE(lyric_track_aliases.duration_ms, excluded.duration_ms),
                       updated_at=unixepoch()",
                    params![
                        track_key,
                        normalize_identity_component(&title),
                        normalize_identity_component(&artist),
                        duration_from_track_key(&track_key),
                    ],
                )
                .map_err(|error| format!("迁移歌词关联别名失败：{error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("提交歌词关联别名迁移失败：{error}"))?;
        Ok(())
    }

    pub(crate) fn ensure_track_alias(
        &self,
        observed_track_key: &str,
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration_ms: Option<u64>,
    ) -> Result<String, String> {
        let title_norm = normalize_identity_component(title);
        let artist_norm = normalize_identity_component(artist);
        if title_norm.is_empty() || artist_norm.is_empty() {
            return Ok(observed_track_key.to_owned());
        }
        let album_norm = normalized_optional(album);
        let duration_ms = duration_ms
            .map(|value| value.min(i64::MAX as u64) as i64)
            .filter(|value| *value > 0);
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始保存歌词关联证据失败：{error}"))?;
        let canonical = transaction
            .query_row(
                "SELECT canonical_track_key FROM lyric_track_aliases
                 WHERE observed_track_key=?1",
                params![observed_track_key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("读取歌词关联证据失败：{error}"))?
            .unwrap_or_else(|| observed_track_key.to_owned());
        transaction
            .execute(
                "INSERT INTO lyric_track_aliases
                   (observed_track_key, canonical_track_key, title_norm, artist_norm,
                    album_norm, duration_ms, evidence_kind, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'unverified_legacy', unixepoch())
                 ON CONFLICT(observed_track_key) DO UPDATE SET
                   title_norm=excluded.title_norm, artist_norm=excluded.artist_norm,
                   album_norm=COALESCE(excluded.album_norm, lyric_track_aliases.album_norm),
                   duration_ms=COALESCE(excluded.duration_ms, lyric_track_aliases.duration_ms),
                   updated_at=unixepoch()",
                params![
                    observed_track_key,
                    canonical,
                    title_norm,
                    artist_norm,
                    album_norm,
                    duration_ms,
                ],
            )
            .map_err(|error| format!("保存歌词关联证据失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交歌词关联证据失败：{error}"))?;
        Ok(canonical)
    }

    pub(crate) fn canonical_track_key(&self, observed_track_key: &str) -> Result<String, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .query_row(
                "SELECT canonical_track_key FROM lyric_track_aliases
                 WHERE observed_track_key=?1",
                params![observed_track_key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("读取歌词关联键失败：{error}"))
            .map(|value| value.unwrap_or_else(|| observed_track_key.to_owned()))
    }

}
