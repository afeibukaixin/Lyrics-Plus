const LYRICS_LIBRARY_ROOT_ID: &str = "lyrics_library";
const EXTERNAL_ROOT_ID: &str = "external";

struct LegacyBindingRow {
    track_key: String,
    title: String,
    artist: String,
    source: String,
    path: PathBuf,
    offset_ms: i64,
    manual_selected: bool,
    provider_id: Option<String>,
    provider_item_id: Option<String>,
    app_owned: bool,
}

impl Storage {
    pub(crate) fn sync_v2_lyric_binding(
        &self,
        track_key: &str,
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration_ms: Option<u64>,
        source: &str,
        raw: &str,
        path: &Path,
        original_format: &str,
        provider_id: Option<&str>,
        provider_item_id: Option<&str>,
        confidence: Option<u8>,
        kind: SaveKind,
        source_kind: &str,
        offset_ms: i64,
        has_translation: bool,
        has_word_timing: bool,
        has_romanization: bool,
    ) -> Result<(), String> {
        let selection_source = if kind == SaveKind::Automatic {
            "automatic"
        } else {
            "user"
        };
        let confidence = confidence.unwrap_or(if kind == SaveKind::Automatic { 0 } else { 100 });
        self.sync_v2_lyric_binding_with_labels(
            track_key,
            title,
            artist,
            album,
            duration_ms,
            source,
            raw,
            path,
            original_format,
            provider_id,
            provider_item_id,
            confidence,
            source_kind,
            selection_source,
            offset_ms,
            has_translation,
            has_word_timing,
            has_romanization,
        )
    }

    fn sync_v2_lyric_binding_with_labels(
        &self,
        track_key: &str,
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration_ms: Option<u64>,
        source: &str,
        raw: &str,
        path: &Path,
        original_format: &str,
        provider_id: Option<&str>,
        provider_item_id: Option<&str>,
        confidence: u8,
        source_kind: &str,
        selection_source: &str,
        offset_ms: i64,
        has_translation: bool,
        has_word_timing: bool,
        has_romanization: bool,
    ) -> Result<(), String> {
        let platform = platform_from_track_key(track_key);
        let external_id =
            song_manager::exact_track_external_id(platform, track_key).map(|(_, value)| value);
        let artists = [artist.to_owned()];
        let recording_id = self.observe_track(
            track_key,
            platform,
            external_id,
            title,
            &artists,
            album,
            duration_ms,
        )?;
        let fingerprint = content_hash(raw);
        let (root_dir_id, relative_path) = self.asset_location(path);
        let source_name = if source.trim().is_empty() {
            source_kind
        } else {
            source.trim()
        };
        let evidence_json = serde_json::json!({
            "providerId": provider_id,
            "providerItemId": provider_item_id,
            "source": source_name,
            "sourceKind": source_kind,
        })
        .to_string();
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始同步歌词绑定失败：{error}"))?;
        transaction
            .execute(
                "INSERT INTO lyric_assets
                   (source_kind, source_name, provider_id, provider_item_id, original_format,
                    language, has_word_timing, has_translation, has_romanization,
                    content_fingerprint, root_dir_id, relative_path, available)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'und', ?6, ?7, ?8, ?9, ?10, ?11, 1)
                 ON CONFLICT(content_fingerprint) DO UPDATE SET
                   source_name=CASE WHEN lyric_assets.source_name='' THEN excluded.source_name
                                    ELSE lyric_assets.source_name END,
                   has_word_timing=MAX(lyric_assets.has_word_timing, excluded.has_word_timing),
                   has_translation=MAX(lyric_assets.has_translation, excluded.has_translation),
                   has_romanization=MAX(lyric_assets.has_romanization, excluded.has_romanization),
                   available=1,
                   updated_at=unixepoch()",
                rusqlite::params![
                    source_kind,
                    source_name,
                    provider_id,
                    provider_item_id,
                    original_format,
                    bool_flag(has_word_timing),
                    bool_flag(has_translation),
                    bool_flag(has_romanization),
                    fingerprint,
                    root_dir_id,
                    relative_path,
                ],
            )
            .map_err(|error| format!("同步歌词资源失败：{error}"))?;
        let asset_id = transaction
            .query_row(
                "SELECT asset_id FROM lyric_assets WHERE content_fingerprint=?1",
                rusqlite::params![fingerprint],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("读取歌词资源失败：{error}"))?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO lyric_asset_sources
                   (asset_id, source_kind, source_name, provider_id, provider_item_id,
                    root_dir_id, relative_path, available)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)",
                rusqlite::params![
                    asset_id,
                    source_kind,
                    source_name,
                    provider_id,
                    provider_item_id,
                    root_dir_id,
                    relative_path,
                ],
            )
            .map_err(|error| format!("保存歌词来源证据失败：{error}"))?;
        if let (Some("qishui"), Some(provider_item_id)) = (provider_id, provider_item_id) {
            // 选择歌词只保存来源证据；平台关系必须由“关联到当前歌曲”明确确认。
            transaction
                .execute(
                    "INSERT INTO recording_external_ids
                       (namespace, id_kind, value, recording_id, confidence, confirmed)
                     VALUES ('qishui', 'track_id', ?1, ?2, ?3, ?4)
                     ON CONFLICT(namespace, id_kind, value) DO UPDATE SET
                       recording_id=excluded.recording_id,
                       confidence=MAX(recording_external_ids.confidence, excluded.confidence),
                       confirmed=MAX(recording_external_ids.confirmed, excluded.confirmed),
                       updated_at=unixepoch()
                     WHERE recording_external_ids.recording_id=excluded.recording_id",
                    rusqlite::params![
                        provider_item_id,
                        recording_id,
                        i64::from(confidence.min(100)),
                        0_i64,
                    ],
                )
                .map_err(|error| format!("保存汽水音乐外部 ID 失败：{error}"))?;
        }
        transaction
            .execute(
                "UPDATE lyric_asset_sources SET available=1, updated_at=unixepoch()
                 WHERE asset_id=?1 AND source_kind=?2 AND source_name=?3
                   AND COALESCE(provider_id, '')=COALESCE(?4, '')
                   AND COALESCE(provider_item_id, '')=COALESCE(?5, '')
                   AND COALESCE(root_dir_id, '')=COALESCE(?6, '')
                   AND COALESCE(relative_path, '')=COALESCE(?7, '')",
                rusqlite::params![
                    asset_id,
                    source_kind,
                    source_name,
                    provider_id,
                    provider_item_id,
                    root_dir_id,
                    relative_path,
                ],
            )
            .map_err(|error| format!("更新歌词来源状态失败：{error}"))?;
        if selection_source != "migration" {
            song_manager::normalize_platform_lyrics(&transaction, recording_id)?;
        }
        transaction
            .execute(
                "UPDATE recording_lyric_bindings SET is_default=0, updated_at=unixepoch()
                 WHERE recording_id=?1",
                rusqlite::params![recording_id],
            )
            .map_err(|error| format!("整理旧歌词默认绑定失败：{error}"))?;
        transaction
            .execute(
                "INSERT INTO recording_lyric_bindings
                   (recording_id, asset_id, selection_source, confidence, evidence_json,
                    is_default, offset_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)
                 ON CONFLICT(recording_id, asset_id) DO UPDATE SET
                   selection_source=excluded.selection_source,
                   confidence=excluded.confidence,
                   evidence_json=excluded.evidence_json,
                   is_default=1,
                   offset_ms=excluded.offset_ms,
                   updated_at=unixepoch()",
                rusqlite::params![
                    recording_id,
                    asset_id,
                    selection_source,
                    i64::from(confidence.min(100)),
                    evidence_json,
                    offset_ms,
                ],
            )
            .map_err(|error| format!("保存歌词默认绑定失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交歌词绑定同步失败：{error}"))
    }

    pub(crate) fn load_v2_lyrics(&self, track_key: &str) -> Result<Option<LyricsDocument>, String> {
        let platform = platform_from_track_key(track_key);
        let row = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            connection
                .query_row(
                    "SELECT asset.source_name, asset.original_format, asset.root_dir_id,
                            asset.relative_path, binding.selection_source,
                            COALESCE(platform_override.offset_ms, binding.offset_ms),
                            observation.raw_title, observation.raw_artists_json
                     FROM track_observations AS observation
                     LEFT JOIN platform_lyric_overrides AS platform_override
                       ON platform_override.recording_id=observation.recording_id
                      AND platform_override.platform=observation.platform
                     JOIN recording_lyric_bindings AS binding
                       ON binding.recording_id=observation.recording_id
                     JOIN lyric_assets AS asset
                       ON asset.asset_id=CASE
                            WHEN platform_override.asset_id IS NOT NULL THEN platform_override.asset_id
                            ELSE binding.asset_id
                          END
                     WHERE observation.platform=?1 AND observation.track_key=?2
                       AND (platform_override.asset_id IS NOT NULL OR binding.is_default=1)
                       AND asset.available=1
                     ORDER BY CASE WHEN platform_override.asset_id IS NOT NULL THEN 0 ELSE 1 END,
                              binding.updated_at DESC
                     LIMIT 1",
                    rusqlite::params![platform, track_key],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, i64>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, String>(7)?,
                        ))
                    },
                )
                .optional()
                .map_err(|error| format!("读取新歌词绑定失败：{error}"))?
        };
        let Some((
            source,
            original_format,
            root_dir_id,
            relative_path,
            selection_source,
            offset_ms,
            title,
            raw_artists_json,
        )) = row
        else {
            return Ok(None);
        };
        let Some(path) = self.resolve_asset_path(root_dir_id.as_deref(), relative_path.as_deref())
        else {
            return Ok(None);
        };
        let Ok(raw) = read_lyric_text(&path) else {
            return Ok(None);
        };
        let Ok(mut document) = parse_lrc_with_options(
            &raw,
            if source.trim().is_empty() {
                "歌词资源"
            } else {
                &source
            },
            selection_source == "user",
        ) else {
            return Ok(None);
        };
        document.metadata.title = Some(title);
        let artists = serde_json::from_str::<Vec<String>>(&raw_artists_json).unwrap_or_default();
        if !artists.is_empty() {
            document.metadata.artist = Some(artists.join(" / "));
        }
        document.metadata.original_format = original_format;
        document.offset_ms = offset_ms;
        Ok(Some(document))
    }

    pub(crate) fn set_v2_lyrics_offset(
        &self,
        track_key: &str,
        offset_ms: i64,
    ) -> Result<(), String> {
        let platform = platform_from_track_key(track_key);
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始保存新歌词偏移失败：{error}"))?;
        let recording_id = transaction
            .query_row(
                "SELECT recording_id FROM track_observations
                 WHERE platform=?1 AND track_key=?2",
                rusqlite::params![platform, track_key],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("读取新歌词录音失败：{error}"))?;
        let Some(recording_id) = recording_id else {
            transaction
                .commit()
                .map_err(|error| format!("提交新歌词偏移失败：{error}"))?;
            return Ok(());
        };
        transaction
            .execute(
                "INSERT INTO platform_lyric_overrides (recording_id, platform, offset_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(recording_id, platform) DO UPDATE SET
                   offset_ms=excluded.offset_ms, updated_at=unixepoch()",
                rusqlite::params![recording_id, platform, offset_ms],
            )
            .map_err(|error| format!("保存新歌词偏移失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交新歌词偏移失败：{error}"))
    }

    pub(crate) fn unbind_v2_lyrics(&self, track_key: &str) -> Result<(), String> {
        let platform = platform_from_track_key(track_key);
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始解除新歌词绑定失败：{error}"))?;
        let recording_id = transaction
            .query_row(
                "SELECT recording_id FROM track_observations
                 WHERE platform=?1 AND track_key=?2",
                rusqlite::params![platform, track_key],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("读取新歌词录音失败：{error}"))?;
        if let Some(recording_id) = recording_id {
            transaction
                .execute(
                    "UPDATE recording_lyric_bindings SET is_default=0, updated_at=unixepoch()
                     WHERE recording_id=?1",
                    rusqlite::params![recording_id],
                )
                .map_err(|error| format!("解除新歌词默认绑定失败：{error}"))?;
            transaction
                .execute(
                    "DELETE FROM platform_lyric_overrides
                     WHERE recording_id=?1 AND platform=?2",
                    rusqlite::params![recording_id, platform],
                )
                .map_err(|error| format!("解除平台歌词覆盖失败：{error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("提交新歌词解绑失败：{error}"))
    }

    fn migrate_legacy_observations(&self) -> Result<(), String> {
        let rows = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mut statement = connection
                .prepare(
                    "SELECT track_key, title, artist FROM lyric_associations
                     UNION
                     SELECT track_key, title, artist FROM lyric_history
                     UNION
                     SELECT observed_track_key, title_norm, artist_norm
                     FROM lyric_track_aliases",
                )
                .map_err(|error| format!("读取旧曲目观察失败：{error}"))?;
            let observations = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map_err(|error| format!("查询旧曲目观察失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析旧曲目观察失败：{error}"))?;
            observations
        };
        for (track_key, title, artist) in rows {
            let platform = platform_from_track_key(&track_key);
            let exists = self.connection.lock().unwrap_or_else(|error| error.into_inner())
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM track_observations WHERE platform=?1 AND track_key=?2)",
                    rusqlite::params![platform, track_key], |row| row.get::<_, bool>(0),
                ).map_err(|error| format!("核对已有曲目观察失败：{error}"))?;
            if exists {
                continue;
            }
            let artists = split_legacy_artist_credits(&artist);
            if artists.is_empty() {
                continue;
            }
            let external_id = track_key
                .strip_prefix(&format!("{platform}:"))
                .filter(|value| !value.is_empty())
                .filter(|value| !value.starts_with("fallback:"));
            self.observe_track(
                &track_key,
                platform,
                external_id,
                &title,
                &artists,
                None,
                binding_duration_from_track_key(&track_key),
            )?;
        }
        let (legacy_count, migrated_count) = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let legacy_count = connection
                .query_row("SELECT COUNT(*) FROM lyric_associations", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(|error| format!("核对旧歌词绑定数量失败：{error}"))?;
            let migrated_count = connection
                .query_row(
                    "SELECT COUNT(*) FROM lyric_associations AS association
                     WHERE EXISTS (
                       SELECT 1 FROM track_observations AS observation
                       JOIN recording_lyric_bindings AS binding
                         ON binding.recording_id=observation.recording_id
                        AND binding.is_default=1
                       WHERE observation.track_key=association.track_key
                     )",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|error| format!("核对新歌词绑定数量失败：{error}"))?;
            (legacy_count, migrated_count)
        };
        if legacy_count != migrated_count {
            log::warn!(
                "旧新歌词生效绑定数量不一致，保留旧读取兜底：legacy={legacy_count} migrated={migrated_count}"
            );
        }
        Ok(())
    }

    pub(crate) fn migrate_legacy_bindings(&self) -> Result<(), String> {
        self.migrate_legacy_observations()?;
        let rows = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mut statement = connection
                .prepare(
                    "SELECT association.track_key, association.title, association.artist,
                            association.source, association.content_path, association.offset_ms,
                            association.manual_selected, association.provider_id,
                            association.provider_item_id,
                            COALESCE(files.app_owned,
                              CASE WHEN association.source IN ('本地文件', '本地导入', '手动导入')
                                   THEN 0 ELSE 1 END)
                     FROM lyric_associations AS association
                     LEFT JOIN lyric_files AS files
                       ON files.content_path=association.content_path",
                )
                .map_err(|error| format!("读取旧歌词绑定失败：{error}"))?;
            let bindings = statement
                .query_map([], |row| {
                    Ok(LegacyBindingRow {
                        track_key: row.get(0)?,
                        title: row.get(1)?,
                        artist: row.get(2)?,
                        source: row.get(3)?,
                        path: PathBuf::from(row.get::<_, String>(4)?),
                        offset_ms: row.get(5)?,
                        manual_selected: row.get::<_, i64>(6)? != 0,
                        provider_id: row.get(7)?,
                        provider_item_id: row.get(8)?,
                        app_owned: row.get::<_, i64>(9)? != 0,
                    })
                })
                .map_err(|error| format!("查询旧歌词绑定失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析旧歌词绑定失败：{error}"))?;
            bindings
        };
        for row in rows {
            // 启动迁移不能覆盖用户已管理的歌曲，尤其是明确选择暂不绑定的曲目。
            if self.has_managed_recording(&row.track_key)? {
                continue;
            }
            if !row.path.is_file() {
                continue;
            }
            let Ok(raw) = read_lyric_text(&row.path) else {
                continue;
            };
            let Ok(document) = parse_lrc_with_options(&raw, &row.source, row.manual_selected)
            else {
                continue;
            };
            self.sync_v2_lyric_binding_with_labels(
                &row.track_key,
                &row.title,
                &row.artist,
                None,
                None,
                &row.source,
                &raw,
                &row.path,
                &document.metadata.original_format,
                row.provider_id.as_deref(),
                row.provider_item_id.as_deref(),
                if row.manual_selected { 100 } else { 0 },
                if row.app_owned { "legacy" } else { "local" },
                "migration",
                row.offset_ms,
                document.tracks.translation.is_some(),
                document
                    .tracks
                    .original
                    .lines
                    .iter()
                    .any(|line| line.words.as_ref().is_some_and(|words| !words.is_empty())),
                document.tracks.romanization.is_some(),
            )?;
        }
        Ok(())
    }

    fn asset_location(&self, path: &Path) -> (String, String) {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let roots = connection
            .prepare("SELECT root_id, path FROM library_roots WHERE enabled=1")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            PathBuf::from(row.get::<_, String>(1)?),
                        ))
                    })
                    .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
            })
            .unwrap_or_default();
        roots
            .into_iter()
            .filter_map(|(root_id, root_path)| {
                path.strip_prefix(&root_path)
                    .ok()
                    .map(|relative| (root_id, root_path.as_os_str().len(), relative))
            })
            .max_by_key(|(_, path_length, _)| *path_length)
            .map(|(root_id, _, relative)| (root_id, relative.to_string_lossy().into_owned()))
            .unwrap_or_else(|| {
                (
                    EXTERNAL_ROOT_ID.to_owned(),
                    path.to_string_lossy().into_owned(),
                )
            })
    }

    fn resolve_asset_path(
        &self,
        root_dir_id: Option<&str>,
        relative_path: Option<&str>,
    ) -> Option<PathBuf> {
        let (Some(root_dir_id), Some(relative_path)) = (root_dir_id, relative_path) else {
            return None;
        };
        if root_dir_id == EXTERNAL_ROOT_ID {
            return Some(PathBuf::from(relative_path));
        }
        let relative = Path::new(relative_path);
        if relative.is_absolute() {
            return None;
        }
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root_path = connection
            .query_row(
                "SELECT path FROM library_roots WHERE root_id=?1",
                rusqlite::params![root_dir_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
            .map(PathBuf::from)
            .or_else(|| {
                (root_dir_id == LYRICS_LIBRARY_ROOT_ID).then(|| self.library_directory())
            })?;
        let path = root_path.join(relative);
        path.starts_with(&root_path).then_some(path)
    }
}

fn bool_flag(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn split_legacy_artist_credits(value: &str) -> Vec<String> {
    value
        .split(|character: char| matches!(character, '/' | '／' | '、' | '；' | ';' | ',' | '，'))
        .map(str::trim)
        .filter(|artist| !artist.is_empty())
        .map(str::to_owned)
        .collect()
}

fn binding_duration_from_track_key(track_key: &str) -> Option<u64> {
    track_key
        .rsplit('|')
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
}

fn platform_from_track_key(track_key: &str) -> &str {
    track_key
        .split_once(':')
        .map(|(platform, _)| platform)
        .filter(|platform| !platform.trim().is_empty())
        .unwrap_or("unknown")
}
