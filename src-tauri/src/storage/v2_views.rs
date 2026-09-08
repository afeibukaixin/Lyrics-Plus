use serde_json::Value;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistCredit {
    pub artist_id: Option<i64>,
    pub raw_name: String,
    pub canonical_name: String,
    pub credit_order: u32,
    pub role: String,
    pub confirmed_aliases: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingIdentity {
    pub recording_id: i64,
    pub title: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub version_tags: Vec<String>,
    pub artist_credits: Vec<ArtistCredit>,
    pub external_identifiers: Vec<ExternalIdentifier>,
    pub observations: Vec<TrackObservation>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricAsset {
    pub asset_id: i64,
    pub source_kind: String,
    pub source_name: String,
    pub provider_id: Option<String>,
    pub provider_item_id: Option<String>,
    pub original_format: String,
    pub language: String,
    pub has_word_timing: bool,
    pub has_translation: bool,
    pub has_romanization: bool,
    pub content_fingerprint: String,
    pub root_id: Option<String>,
    pub root_name: Option<String>,
    pub relative_path: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsBinding {
    pub binding_id: i64,
    pub recording_id: i64,
    pub asset_id: i64,
    pub selection_source: String,
    pub confidence: u8,
    pub evidence: Value,
    pub is_default: bool,
    pub offset_ms: i64,
    pub asset: Option<LyricAsset>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformLyricOverride {
    pub override_id: i64,
    pub recording_id: i64,
    pub platform: String,
    pub asset_id: Option<i64>,
    pub offset_ms: i64,
    pub asset: Option<LyricAsset>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsContext {
    pub track_key: String,
    pub platform: String,
    pub observation: TrackObservation,
    pub recording: RecordingIdentity,
    pub bindings: Vec<LyricsBinding>,
    pub platform_override: Option<PlatformLyricOverride>,
    pub current_asset: Option<LyricAsset>,
    pub current_binding_id: Option<i64>,
}

impl Storage {
    /// 从播放器搜索输入补齐 Recording/TrackObservation，避免手动搜索没有身份记录。
    pub(crate) fn ensure_lyrics_observation(
        &self,
        track_key: &str,
        input: &LyricsSearchInput,
    ) -> Result<i64, String> {
        let platform = input
            .platform
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| platform_from_track_key(track_key));
        let artists = split_legacy_artist_credits(&input.artist);
        let artists = if artists.is_empty() {
            vec![input.artist.trim().to_owned()]
        } else {
            artists
        };
        self.observe_track(
            track_key,
            platform,
            input.platform_item_id.as_deref(),
            &input.title,
            &artists,
            input.album.as_deref(),
            input.duration_ms,
        )
    }

    /// 将已确认的歌手别名放入本次搜索评分上下文；未确认别名不会进入评分。
    pub(crate) fn enrich_lyrics_input_with_confirmed_aliases(
        &self,
        track_key: &str,
        input: &LyricsSearchInput,
    ) -> Result<LyricsSearchInput, String> {
        let Some(context) = self.current_lyrics_context(track_key)? else {
            return Ok(input.clone());
        };
        let aliases = context
            .recording
            .artist_credits
            .into_iter()
            .flat_map(|credit| {
                credit
                    .confirmed_aliases
                    .into_iter()
                    .map(move |alias| (credit.canonical_name.clone(), alias))
            })
            .collect::<Vec<_>>();
        if aliases.is_empty() {
            return Ok(input.clone());
        }
        let mut enriched = input.clone();
        let scoring = (*input.scoring)
            .clone()
            .with_confirmed_artist_aliases(aliases);
        enriched.scoring = std::sync::Arc::new(scoring);
        Ok(enriched)
    }

    /// 用户添加或移除当前录音的歌手别名；别名只影响后续匹配评分。
    pub(crate) fn set_artist_alias_confirmation(
        &self,
        track_key: &str,
        artist_id: i64,
        alias: &str,
        confirmed: bool,
    ) -> Result<(), String> {
        if artist_id <= 0 {
            return Err("歌手标识无效".into());
        }
        let alias = alias.trim();
        if alias.is_empty() {
            return Err("歌手别名不能为空".into());
        }
        let normalized_alias = normalize_artist_alias(alias);
        if normalized_alias.is_empty() {
            return Err("歌手别名不包含有效字符".into());
        }
        let observed_track_key = track_key.trim();
        if observed_track_key.is_empty() {
            return Err("曲目标识不能为空".into());
        }
        let canonical_track_key = self.canonical_track_key(observed_track_key)?;
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始更新歌手别名失败：{error}"))?;
        let recording_id = transaction
            .query_row(
                "SELECT recording_id FROM track_observations
                 WHERE track_key=?1 OR track_key=?2
                 ORDER BY CASE WHEN track_key=?1 THEN 0 ELSE 1 END
                 LIMIT 1",
                rusqlite::params![observed_track_key, canonical_track_key],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("读取当前录音失败：{error}"))?
            .ok_or_else(|| "当前曲目尚未建立录音身份".to_string())?;
        let (artist_belongs, canonical_name) = transaction
            .query_row(
                "SELECT
                    EXISTS(SELECT 1 FROM recording_artist_credits
                           WHERE recording_id=?1 AND artist_id=?2),
                    COALESCE((SELECT COALESCE(artist.canonical_name, credit.raw_name)
                              FROM recording_artist_credits AS credit
                              LEFT JOIN artists AS artist ON artist.artist_id=credit.artist_id
                              WHERE credit.recording_id=?1 AND credit.artist_id=?2
                              ORDER BY credit.credit_order LIMIT 1), '')",
                rusqlite::params![recording_id, artist_id],
                |row| Ok((row.get::<_, i64>(0)? != 0, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| format!("读取当前录音歌手失败：{error}"))?
            .ok_or_else(|| "当前录音没有该歌手署名".to_string())?;
        if !artist_belongs {
            return Err("歌手不属于当前录音".into());
        }
        if confirmed && normalize_artist_alias(&canonical_name) == normalized_alias {
            return Err("歌手别名与规范歌手名称相同".into());
        }
        if confirmed {
            transaction
                .execute(
                    "INSERT INTO artist_aliases
                       (artist_id, alias, normalized_alias, confirmed)
                     VALUES (?1, ?2, ?3, 1)
                     ON CONFLICT(artist_id, normalized_alias) DO UPDATE SET
                       alias=excluded.alias, confirmed=1, updated_at=unixepoch()",
                    rusqlite::params![artist_id, alias, normalized_alias],
                )
                .map_err(|error| format!("保存歌手别名失败：{error}"))?;
        } else {
            transaction
                .execute(
                    "UPDATE artist_aliases
                     SET confirmed=0, updated_at=unixepoch()
                     WHERE artist_id=?1 AND normalized_alias=?2",
                    rusqlite::params![artist_id, normalized_alias],
                )
                .map_err(|error| format!("移除歌手别名失败：{error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("提交歌手别名更新失败：{error}"))
    }

    pub fn current_lyrics_context(&self, track_key: &str) -> Result<Option<LyricsContext>, String> {
        let observed_track_key = track_key.trim();
        if observed_track_key.is_empty() {
            return Ok(None);
        }
        let canonical_track_key = self.canonical_track_key(observed_track_key)?;
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some((observation, recording)) = connection
            .query_row(
                "SELECT observation.observation_id, observation.track_key, observation.platform,
                        observation.raw_title, observation.raw_artists_json, observation.raw_album,
                        observation.duration_ms, observation.observed_at, observation.recording_id,
                        observation.split_from_recording_id,
                        recording.title, recording.album, recording.duration_ms,
                        recording.version_tags_json
                 FROM track_observations AS observation
                 JOIN recordings AS recording ON recording.recording_id=observation.recording_id
                 WHERE observation.track_key=?1 OR observation.track_key=?2
                 ORDER BY CASE WHEN observation.track_key=?1 THEN 0 ELSE 1 END,
                          observation.observed_at DESC, observation.observation_id DESC
                 LIMIT 1",
                rusqlite::params![observed_track_key, canonical_track_key],
                |row| {
                    let title = row.get::<_, String>(10)?;
                    let version_tags_json = row.get::<_, String>(13)?;
                    let version_tags = recording_version_tags(&version_tags_json, &title);
                    Ok((
                        TrackObservation {
                            observation_id: row.get(0)?,
                            track_key: row.get(1)?,
                            platform: row.get(2)?,
                            raw_title: row.get(3)?,
                            raw_artists: parse_json_vec(row.get::<_, String>(4)?),
                            raw_album: row.get(5)?,
                            duration_ms: row.get::<_, Option<i64>>(6)?.and_then(to_u64),
                            observed_at: row.get(7)?,
                            recording_id: row.get(8)?,
                            split_from_recording_id: row.get(9)?,
                        },
                        (
                            row.get::<_, i64>(8)?,
                            title,
                            row.get::<_, Option<String>>(11)?,
                            row.get::<_, Option<i64>>(12)?.and_then(to_u64),
                            version_tags,
                        ),
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("读取当前歌词身份失败：{error}"))?
        else {
            return Ok(None);
        };

        let recording_id = recording.0;
        let artist_credits = load_artist_credits(&connection, recording_id)?;
        let external_identifiers = load_external_identifiers(&connection, recording_id)?;
        let observations = load_observations(&connection, recording_id)?;
        let bindings = load_bindings(&connection, recording_id)?;
        let platform_override = connection
            .query_row(
                "SELECT override_id, recording_id, platform, asset_id, offset_ms
                 FROM platform_lyric_overrides
                 WHERE recording_id=?1 AND platform=?2",
                rusqlite::params![recording_id, observation.platform],
                |row| {
                    Ok(PlatformLyricOverride {
                        override_id: row.get(0)?,
                        recording_id: row.get(1)?,
                        platform: row.get(2)?,
                        asset_id: row.get(3)?,
                        offset_ms: row.get(4)?,
                        asset: None,
                    })
                },
            )
            .optional()
            .map_err(|error| format!("读取平台歌词覆盖失败：{error}"))?;
        let platform_override = platform_override.map(|mut override_view| {
            override_view.asset = override_view
                .asset_id
                .and_then(|asset_id| load_asset(&connection, asset_id).ok().flatten());
            override_view
        });
        let current_binding = bindings.iter().find(|binding| {
            platform_override
                .as_ref()
                .and_then(|override_view| override_view.asset_id)
                .map_or(binding.is_default, |asset_id| binding.asset_id == asset_id)
                && binding.asset.as_ref().is_some_and(|asset| asset.available)
        });
        let current_asset = platform_override
            .as_ref()
            .and_then(|override_view| override_view.asset.clone())
            .or_else(|| current_binding.and_then(|binding| binding.asset.clone()));
        let current_binding_id = current_binding.map(|binding| binding.binding_id);
        Ok(Some(LyricsContext {
            track_key: observation.track_key.clone(),
            platform: observation.platform.clone(),
            observation,
            recording: RecordingIdentity {
                recording_id,
                title: recording.1,
                album: recording.2,
                duration_ms: recording.3,
                version_tags: recording.4,
                artist_credits,
                external_identifiers,
                observations,
            },
            bindings,
            platform_override,
            current_asset,
            current_binding_id,
        }))
    }

    /// 解绑共用歌词也保留资源与各平台偏移，禁止旧绑定回退。
    pub fn clear_lyrics_binding(&self, track_key: &str) -> Result<(), String> {
        let platform = platform_from_track_key(track_key);
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始解绑歌词失败：{error}"))?;
        let recording_id = transaction
            .query_row(
                "SELECT recording_id FROM track_observations WHERE platform=?1 AND track_key=?2",
                rusqlite::params![platform, track_key],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("读取歌曲失败：{error}"))?;
        if let Some(recording_id) = recording_id {
            song_manager::normalize_platform_lyrics(&transaction, recording_id)?;
            transaction
                .execute(
                    "UPDATE recording_lyric_bindings SET is_default=0, updated_at=unixepoch()
                 WHERE recording_id=?1",
                    rusqlite::params![recording_id],
                )
                .map_err(|error| format!("解绑歌曲共用歌词失败：{error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("提交歌词解绑失败：{error}"))
    }

    pub fn mark_latest_lyrics_search_selection(
        &self,
        track_key: &str,
        provider_id: &str,
        provider_item_id: &str,
        selection_reason: &str,
    ) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let run_id = connection
            .query_row(
                "SELECT run_id FROM lyrics_search_runs WHERE track_key=?1
                 ORDER BY started_at DESC LIMIT 1",
                rusqlite::params![track_key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("读取最近歌词搜索失败：{error}"))?;
        let Some(run_id) = run_id else {
            return Ok(());
        };
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始记录歌词选择失败：{error}"))?;
        transaction
            .execute(
                "UPDATE lyrics_search_candidates SET state='fetched',
                   rejection_reason=CASE WHEN provider_id=?2 AND provider_item_id=?3
                                         THEN NULL ELSE COALESCE(rejection_reason, 'not_selected') END,
                   selection_reason=NULL
                 WHERE run_id=?1",
                rusqlite::params![run_id, provider_id, provider_item_id],
            )
            .map_err(|error| format!("整理歌词选择记录失败：{error}"))?;
        transaction
            .execute(
                "UPDATE lyrics_search_candidates SET state='selected',
                   rejection_reason=NULL, selection_reason=?4
                 WHERE run_id=?1 AND provider_id=?2 AND provider_item_id=?3",
                rusqlite::params![run_id, provider_id, provider_item_id, selection_reason],
            )
            .map_err(|error| format!("保存歌词选择记录失败：{error}"))?;
        transaction
            .execute(
                "UPDATE lyrics_search_runs SET status='completed', finished_at=COALESCE(finished_at, unixepoch()),
                   selected_provider_id=?2, selected_provider_item_id=?3, selection_reason=?4
                 WHERE run_id=?1",
                rusqlite::params![run_id, provider_id, provider_item_id, selection_reason],
            )
            .map_err(|error| format!("更新歌词搜索选择失败：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("提交歌词选择失败：{error}"))
    }
}

fn normalize_artist_alias(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn load_artist_credits(
    connection: &rusqlite::Connection,
    recording_id: i64,
) -> Result<Vec<ArtistCredit>, String> {
    let rows = connection
        .prepare(
            "SELECT credit.artist_id, credit.raw_name,
                    COALESCE(artist.canonical_name, credit.raw_name),
                    credit.credit_order, credit.role
             FROM recording_artist_credits AS credit
             LEFT JOIN artists AS artist ON artist.artist_id=credit.artist_id
             WHERE credit.recording_id=?1 ORDER BY credit.credit_order",
        )
        .and_then(|mut statement| {
            statement
                .query_map(rusqlite::params![recording_id], |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
        })
        .map_err(|error| format!("读取录音歌手署名失败：{error}"))?;
    rows.into_iter()
        .map(
            |(artist_id, raw_name, canonical_name, credit_order, role)| {
                let confirmed_aliases = artist_id
                    .map(|artist_id| {
                        connection
                            .prepare(
                                "SELECT alias FROM artist_aliases
                             WHERE artist_id=?1 AND confirmed=1 ORDER BY alias_id",
                            )
                            .and_then(|mut statement| {
                                statement
                                    .query_map(rusqlite::params![artist_id], |row| row.get(0))
                                    .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
                            })
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                Ok(ArtistCredit {
                    artist_id,
                    raw_name,
                    canonical_name,
                    credit_order: credit_order.max(0) as u32,
                    role,
                    confirmed_aliases,
                })
            },
        )
        .collect()
}

fn load_external_identifiers(
    connection: &rusqlite::Connection,
    recording_id: i64,
) -> Result<Vec<ExternalIdentifier>, String> {
    let mut statement = connection
        .prepare(
            "SELECT external_id, namespace, id_kind, value, confidence, confirmed
             FROM recording_external_ids WHERE recording_id=?1
             ORDER BY confirmed DESC, confidence DESC, external_id",
        )
        .map_err(|error| format!("读取录音外部标识失败：{error}"))?;
    let identifiers = statement
        .query_map(rusqlite::params![recording_id], |row| {
            Ok(ExternalIdentifier {
                external_id: row.get(0)?,
                namespace: row.get(1)?,
                id_kind: row.get(2)?,
                value: row.get(3)?,
                confidence: row.get::<_, i64>(4)?.clamp(0, 100) as u8,
                confirmed: row.get::<_, i64>(5)? != 0,
            })
        })
        .map_err(|error| format!("解析录音外部标识失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析录音外部标识失败：{error}"))?;
    Ok(identifiers)
}

fn load_observations(
    connection: &rusqlite::Connection,
    recording_id: i64,
) -> Result<Vec<TrackObservation>, String> {
    let mut statement = connection
        .prepare(
            "SELECT observation_id, track_key, platform, raw_title, raw_artists_json,
                    raw_album, duration_ms, observed_at, recording_id,
                    split_from_recording_id
             FROM track_observations WHERE recording_id=?1
             ORDER BY observed_at DESC, observation_id DESC",
        )
        .map_err(|error| format!("读取录音平台观察失败：{error}"))?;
    let observations = statement
        .query_map(rusqlite::params![recording_id], |row| {
            Ok(TrackObservation {
                observation_id: row.get(0)?,
                track_key: row.get(1)?,
                platform: row.get(2)?,
                raw_title: row.get(3)?,
                raw_artists: parse_json_vec(row.get::<_, String>(4)?),
                raw_album: row.get(5)?,
                duration_ms: row.get::<_, Option<i64>>(6)?.and_then(to_u64),
                observed_at: row.get(7)?,
                recording_id: row.get(8)?,
                split_from_recording_id: row.get(9)?,
            })
        })
        .map_err(|error| format!("解析录音平台观察失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析录音平台观察失败：{error}"))?;
    Ok(observations)
}

fn load_bindings(
    connection: &rusqlite::Connection,
    recording_id: i64,
) -> Result<Vec<LyricsBinding>, String> {
    let mut statement = connection
        .prepare(
            "SELECT binding_id, recording_id, asset_id, selection_source, confidence,
                    evidence_json, is_default, offset_ms
             FROM recording_lyric_bindings
             WHERE recording_id=?1 ORDER BY is_default DESC, updated_at DESC, binding_id DESC",
        )
        .map_err(|error| format!("读取歌词绑定失败：{error}"))?;
    let rows = statement
        .query_map(rusqlite::params![recording_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)? != 0,
                row.get::<_, i64>(7)?,
            ))
        })
        .map_err(|error| format!("解析歌词绑定失败：{error}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析歌词绑定失败：{error}"))?;
    rows.into_iter()
        .map(
            |(
                binding_id,
                recording_id,
                asset_id,
                selection_source,
                confidence,
                evidence_json,
                is_default,
                offset_ms,
            )| {
                let evidence = serde_json::from_str(&evidence_json)
                    .unwrap_or(Value::Object(Default::default()));
                Ok(LyricsBinding {
                    binding_id,
                    recording_id,
                    asset_id,
                    selection_source,
                    confidence: confidence.clamp(0, 100) as u8,
                    evidence,
                    is_default,
                    offset_ms,
                    asset: load_asset(connection, asset_id)?,
                })
            },
        )
        .collect()
}

fn load_asset(
    connection: &rusqlite::Connection,
    asset_id: i64,
) -> Result<Option<LyricAsset>, String> {
    connection
        .query_row(
            "SELECT asset.asset_id, asset.source_kind, asset.source_name,
                    asset.provider_id, asset.provider_item_id, asset.original_format,
                    asset.language, asset.has_word_timing, asset.has_translation,
                    asset.has_romanization, asset.content_fingerprint, asset.root_dir_id,
                    asset.relative_path, asset.available, root.display_name
             FROM lyric_assets AS asset
             LEFT JOIN library_roots AS root ON root.root_id=asset.root_dir_id
             WHERE asset.asset_id=?1",
            rusqlite::params![asset_id],
            |row| {
                Ok(LyricAsset {
                    asset_id: row.get(0)?,
                    source_kind: row.get(1)?,
                    source_name: row.get(2)?,
                    provider_id: row.get(3)?,
                    provider_item_id: row.get(4)?,
                    original_format: row.get(5)?,
                    language: row.get(6)?,
                    has_word_timing: row.get::<_, i64>(7)? != 0,
                    has_translation: row.get::<_, i64>(8)? != 0,
                    has_romanization: row.get::<_, i64>(9)? != 0,
                    content_fingerprint: row.get(10)?,
                    root_id: row.get(11)?,
                    relative_path: row.get(12)?,
                    available: row.get::<_, i64>(13)? != 0,
                    root_name: row.get(14)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("读取歌词资源失败：{error}"))
}

fn parse_json_vec(value: String) -> Vec<String> {
    serde_json::from_str(&value).unwrap_or_default()
}

fn to_u64(value: i64) -> Option<u64> {
    u64::try_from(value).ok()
}
