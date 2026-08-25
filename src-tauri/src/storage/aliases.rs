const TRACK_IDENTITY_DURATION_TOLERANCE_MS: i64 = 2_000;

struct AliasCandidate {
    canonical_track_key: String,
    manual_selected: bool,
    updated_at: i64,
    content_path: PathBuf,
}

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

fn identity_matches(
    album: Option<&str>,
    duration_ms: Option<i64>,
    candidate_album: Option<&str>,
    candidate_duration_ms: Option<i64>,
) -> bool {
    let album_matches = album
        .zip(candidate_album)
        .is_some_and(|(left, right)| left == right);
    let duration_matches = duration_ms
        .zip(candidate_duration_ms)
        .is_some_and(|(left, right)| (left - right).abs() <= TRACK_IDENTITY_DURATION_TOLERANCE_MS);
    album_matches || duration_matches
}

fn candidate_is_better(left: &AliasCandidate, right: &AliasCandidate) -> bool {
    (
        left.manual_selected,
        left.updated_at,
        &left.canonical_track_key,
    ) > (
        right.manual_selected,
        right.updated_at,
        &right.canonical_track_key,
    )
}

impl Storage {
    pub(crate) fn migrate_track_aliases(&self) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let rows = {
            let mut statement = connection
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
            connection
                .execute(
                    "INSERT INTO lyric_track_aliases
                       (observed_track_key, canonical_track_key, title_norm, artist_norm,
                        album_norm, duration_ms, updated_at)
                     VALUES (?1, ?1, ?2, ?3, NULL, ?4, unixepoch())
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
        let mut cleanup_paths = Vec::new();
        let canonical_track_key = {
            let mut connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let transaction = connection
                .transaction()
                .map_err(|error| format!("开始整理歌词关联失败：{error}"))?;

            let existing_canonical = transaction
                .query_row(
                    "SELECT canonical_track_key FROM lyric_track_aliases
                     WHERE observed_track_key=?1",
                    params![observed_track_key],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| format!("读取歌词关联别名失败：{error}"))?;

            let mut candidate_keys = HashSet::new();
            if let Some(canonical) = &existing_canonical {
                candidate_keys.insert(canonical.clone());
            }
            let mut statement = transaction
                .prepare(
                    "SELECT canonical_track_key, album_norm, duration_ms
                     FROM lyric_track_aliases
                     WHERE title_norm=?1 AND artist_norm=?2",
                )
                .map_err(|error| format!("查询相似歌词关联失败：{error}"))?;
            let candidates = statement
                .query_map(params![title_norm, artist_norm], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                })
                .map_err(|error| format!("查询相似歌词关联失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析相似歌词关联失败：{error}"))?;
            drop(statement);
            for (canonical, candidate_album, candidate_duration_ms) in candidates {
                if existing_canonical.as_deref() == Some(canonical.as_str())
                    || identity_matches(
                        album_norm.as_deref(),
                        duration_ms,
                        candidate_album.as_deref(),
                        candidate_duration_ms,
                    )
                {
                    candidate_keys.insert(canonical);
                }
            }

            let mut associated_candidates = Vec::new();
            for canonical in &candidate_keys {
                let candidate = transaction
                    .query_row(
                        "SELECT track_key, manual_selected, updated_at, content_path
                         FROM lyric_associations WHERE track_key=?1",
                        params![canonical],
                        |row| {
                            Ok(AliasCandidate {
                                canonical_track_key: row.get(0)?,
                                manual_selected: row.get::<_, i64>(1)? != 0,
                                updated_at: row.get(2)?,
                                content_path: PathBuf::from(row.get::<_, String>(3)?),
                            })
                        },
                    )
                    .optional()
                    .map_err(|error| format!("读取候选歌词关联失败：{error}"))?;
                if let Some(candidate) = candidate {
                    associated_candidates.push(candidate);
                }
            }
            associated_candidates.sort_by(|left, right| {
                if candidate_is_better(left, right) {
                    std::cmp::Ordering::Less
                } else if candidate_is_better(right, left) {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            });

            let canonical = associated_candidates
                .first()
                .map(|candidate| candidate.canonical_track_key.clone())
                .or(existing_canonical)
                .unwrap_or_else(|| observed_track_key.to_owned());
            let loser_keys = associated_candidates
                .iter()
                .skip(1)
                .map(|candidate| candidate.canonical_track_key.clone())
                .collect::<Vec<_>>();
            for loser in loser_keys {
                if loser == canonical {
                    continue;
                }
                if let Some(candidate) = associated_candidates
                    .iter()
                    .find(|candidate| candidate.canonical_track_key == loser)
                {
                    cleanup_paths.push(candidate.content_path.clone());
                }
                transaction
                    .execute(
                        "UPDATE lyric_track_aliases SET canonical_track_key=?2, updated_at=unixepoch()
                         WHERE canonical_track_key=?1",
                        params![loser, canonical],
                    )
                    .map_err(|error| format!("合并歌词关联别名失败：{error}"))?;
                transaction
                    .execute(
                        "UPDATE lyric_history SET track_key=?2 WHERE track_key=?1",
                        params![loser, canonical],
                    )
                    .map_err(|error| format!("合并歌词历史失败：{error}"))?;
                transaction
                    .execute(
                        "DELETE FROM lyric_associations WHERE track_key=?1",
                        params![loser],
                    )
                    .map_err(|error| format!("合并歌词关联失败：{error}"))?;
            }
            transaction
                .execute(
                    "INSERT INTO lyric_track_aliases
                       (observed_track_key, canonical_track_key, title_norm, artist_norm,
                        album_norm, duration_ms, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, unixepoch())
                     ON CONFLICT(observed_track_key) DO UPDATE SET
                       canonical_track_key=excluded.canonical_track_key,
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
                .map_err(|error| format!("保存歌词关联别名失败：{error}"))?;
            transaction
                .commit()
                .map_err(|error| format!("提交歌词关联整理失败：{error}"))?;
            canonical
        };
        if let Err(error) = self.cleanup_unreferenced_app_owned_files(cleanup_paths) {
            log::warn!("整理歌词关联后的旧文件失败：{error}");
        }
        Ok(canonical_track_key)
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

    pub(crate) fn cleanup_orphan_app_owned_files(&self) {
        let paths = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mut statement = match connection.prepare(
                "SELECT content_path FROM lyric_files
                 WHERE app_owned=1
                   AND source NOT IN ('本地文件', '本地导入', '手动导入')
                   AND NOT EXISTS (
                     SELECT 1 FROM lyric_associations
                     WHERE lyric_associations.content_path=lyric_files.content_path
                   )",
            ) {
                Ok(statement) => statement,
                Err(error) => {
                    log::warn!("读取孤立歌词文件失败：{error}");
                    return;
                }
            };
            match statement
                .query_map([], |row| row.get::<_, String>(0))
                .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
            {
                Ok(paths) => paths.into_iter().map(PathBuf::from).collect::<Vec<_>>(),
                Err(error) => {
                    log::warn!("解析孤立歌词文件失败：{error}");
                    return;
                }
            }
        };
        if let Err(error) = self.cleanup_unreferenced_app_owned_files(paths) {
            log::warn!("清理孤立歌词文件失败：{error}");
        }
    }

    fn cleanup_unreferenced_app_owned_files(
        &self,
        paths: impl IntoIterator<Item = PathBuf>,
    ) -> Result<(), String> {
        let mut unique_paths = HashSet::new();
        for path in paths {
            if !unique_paths.insert(path.clone()) {
                continue;
            }
            let path_string = path.to_string_lossy().into_owned();
            let eligible = {
                let connection = self
                    .connection
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                connection
                    .query_row(
                        "SELECT app_owned, source,
                                EXISTS(
                                  SELECT 1 FROM lyric_associations
                                  WHERE content_path=?1
                                )
                         FROM lyric_files WHERE content_path=?1",
                        params![path_string],
                        |row| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(|error| format!("检查歌词文件引用失败：{error}"))?
                    .is_some_and(|(app_owned, source, referenced)| {
                        app_owned != 0 && !is_user_owned_source(&source) && referenced == 0
                    })
            };
            if !eligible {
                continue;
            }
            if path.exists() {
                fs::remove_file(&path).map_err(|error| format!("删除孤立歌词文件失败：{error}"))?;
            }
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            connection
                .execute(
                    "DELETE FROM lyric_files
                     WHERE content_path=?1 AND app_owned=1
                       AND source NOT IN ('本地文件', '本地导入', '手动导入')
                       AND NOT EXISTS (
                         SELECT 1 FROM lyric_associations
                         WHERE content_path=?1
                       )",
                    params![path_string],
                )
                .map_err(|error| format!("删除孤立歌词索引失败：{error}"))?;
        }
        Ok(())
    }
}
