#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchTraceCandidate {
    pub provider_id: String,
    pub provider_item_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub version_tags: Vec<String>,
    pub score: f64,
    pub state: String,
    pub rejection_reason: Option<String>,
    pub selection_reason: Option<String>,
    pub score_evidence: Option<crate::lyrics::provider::LyricsScoreEvidence>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchTraceProvider {
    pub provider_id: String,
    pub status: String,
    pub candidate_count: usize,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchTraceStage {
    pub stage: String,
    pub provider_id: Option<String>,
    pub status: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub elapsed_ms: u64,
    pub candidate_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsSearchTrace {
    pub run_id: String,
    pub recording_id: Option<i64>,
    pub intent: String,
    pub status: String,
    pub title: String,
    pub artist: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub total_elapsed_ms: Option<u64>,
    pub error_code: Option<String>,
    pub selected_provider_id: Option<String>,
    pub selected_provider_item_id: Option<String>,
    pub selection_reason: Option<String>,
    pub providers: Vec<LyricsSearchTraceProvider>,
    pub stages: Vec<LyricsSearchTraceStage>,
    pub candidates: Vec<LyricsSearchTraceCandidate>,
}

static NEXT_SEARCH_RUN_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Storage {
    pub(crate) fn begin_lyrics_search_run(
        &self,
        track_key: &str,
        intent: &str,
        title: &str,
        artist: &str,
    ) -> Result<String, String> {
        let run_id = format!(
            "{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            NEXT_SEARCH_RUN_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let recording_id = connection
            .query_row(
                "SELECT recording_id FROM track_observations WHERE track_key=?1",
                rusqlite::params![track_key],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("读取搜索录音身份失败：{error}"))?;
        connection
            .execute(
                "INSERT INTO lyrics_search_runs
                   (run_id, track_key, recording_id, intent, title, artist, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'running')",
                rusqlite::params![run_id, track_key, recording_id, intent, title, artist],
            )
            .map_err(|error| format!("保存歌词搜索摘要失败：{error}"))?;
        Ok(run_id)
    }

    pub(crate) fn finish_lyrics_search_run(
        &self,
        run_id: &str,
        status: &str,
        error_code: Option<&str>,
        provider_statuses: &[crate::lyrics::provider::ProviderStatus],
        provider_elapsed_ms: &std::collections::HashMap<String, u64>,
        results: &[crate::lyrics::provider::LyricsSearchResult],
        input: &crate::lyrics::provider::LyricsSearchInput,
        selected: Option<(&str, &str)>,
        selection_reason: Option<&str>,
        total_elapsed_ms: u64,
    ) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let track_key = connection
            .query_row(
                "SELECT track_key FROM lyrics_search_runs WHERE run_id=?1",
                rusqlite::params![run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("读取歌词搜索摘要失败：{error}"))?
            .ok_or_else(|| "歌词搜索摘要不存在".to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("开始保存歌词搜索摘要失败：{error}"))?;
        for provider in provider_statuses {
            let status = match provider.health {
                crate::lyrics::provider::ProviderHealth::Unknown => "unknown",
                crate::lyrics::provider::ProviderHealth::Available => "available",
                crate::lyrics::provider::ProviderHealth::Degraded => "degraded",
                crate::lyrics::provider::ProviderHealth::Unavailable => "unavailable",
            };
            let candidate_count = provider.detail.result_count();
            let elapsed_ms = provider_elapsed_ms
                .get(&provider.provider_id)
                .copied()
                .unwrap_or_default();
            transaction
                .execute(
                    "INSERT INTO lyrics_search_providers
                       (run_id, provider_id, status, candidate_count, elapsed_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(run_id, provider_id) DO UPDATE SET
                       status=excluded.status, candidate_count=excluded.candidate_count,
                       elapsed_ms=excluded.elapsed_ms",
                    rusqlite::params![
                        run_id,
                        provider.provider_id,
                        status,
                        candidate_count,
                        elapsed_ms
                    ],
                )
                .map_err(|error| format!("保存歌词源搜索状态失败：{error}"))?;
        }
        for result in results {
            let provider_item_id = if result.provider_id == crate::storage::LOCAL_PROVIDER_ID {
                local_provider_item_id(&content_hash(&result.lyrics))
            } else {
                result.id.trim().to_owned()
            };
            if provider_item_id.is_empty() {
                continue;
            }
            let artists = result
                .artist
                .split(" / ")
                .map(str::trim)
                .filter(|artist| !artist.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let is_selected = selected.is_some_and(|(provider_id, item_id)| {
                provider_id == result.provider_id
                    && (item_id == result.id || item_id == provider_item_id)
            });
            let rejection_reason = if is_selected {
                None
            } else if result.score < 0.85 {
                Some("score_below_threshold")
            } else {
                Some("not_selected")
            };
            let score_evidence = crate::lyrics::provider::score_evidence(input, result);
            transaction
                .execute(
                    "INSERT INTO lyrics_search_candidates
                       (run_id, provider_id, provider_item_id, title, artists_json, album,
                        duration_ms, version_tags_json, score, state, rejection_reason,
                        selection_reason, score_evidence_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                     ON CONFLICT(run_id, provider_id, provider_item_id) DO UPDATE SET
                       title=excluded.title, artists_json=excluded.artists_json,
                       album=excluded.album, duration_ms=excluded.duration_ms,
                       version_tags_json=excluded.version_tags_json, score=excluded.score,
                       state=excluded.state, rejection_reason=excluded.rejection_reason,
                       selection_reason=excluded.selection_reason,
                       score_evidence_json=excluded.score_evidence_json",
                    rusqlite::params![
                        run_id,
                        result.provider_id,
                        provider_item_id,
                        result.title,
                        serde_json::to_string(&artists).unwrap_or_else(|_| "[]".into()),
                        result.album,
                        result.duration_ms.map(|value| value as i64),
                        serde_json::to_string(&crate::lyrics::provider::version_tags_from_title(
                            &result.title
                        ))
                        .unwrap_or_else(|_| "[]".into()),
                        result.score,
                        if is_selected { "selected" } else { "fetched" },
                        rejection_reason,
                        is_selected.then_some(selection_reason.unwrap_or("user_selected")),
                        serde_json::to_string(&score_evidence).unwrap_or_else(|_| "{}".into()),
                    ],
                )
                .map_err(|error| format!("保存歌词搜索候选失败：{error}"))?;
        }
        transaction
            .execute(
                "UPDATE lyrics_search_runs SET
                   status=?2, finished_at=unixepoch(), error_code=?3,
                   selected_provider_id=?4, selected_provider_item_id=?5,
                   selection_reason=?6, total_elapsed_ms=?7
                 WHERE run_id=?1",
                rusqlite::params![
                    run_id,
                    status,
                    error_code,
                    selected.map(|value| value.0),
                    selected.map(|value| value.1),
                    selection_reason,
                    total_elapsed_ms.min(i64::MAX as u64) as i64,
                ],
            )
            .map_err(|error| format!("更新歌词搜索摘要状态失败：{error}"))?;
        let old_runs = transaction
            .prepare(
                "SELECT run_id FROM lyrics_search_runs
                 WHERE track_key=?1 AND run_id!=?2",
            )
            .and_then(|mut statement| {
                statement
                    .query_map(rusqlite::params![track_key, run_id], |row| {
                        row.get::<_, String>(0)
                    })
                    .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
            })
            .map_err(|error| format!("读取旧歌词搜索摘要失败：{error}"))?;
        for old_run_id in old_runs {
            transaction
                .execute(
                    "DELETE FROM lyrics_search_stages WHERE run_id=?1",
                    rusqlite::params![old_run_id],
                )
                .map_err(|error| format!("清理旧歌词搜索阶段失败：{error}"))?;
            transaction
                .execute(
                    "DELETE FROM lyrics_search_providers WHERE run_id=?1",
                    rusqlite::params![old_run_id],
                )
                .map_err(|error| format!("清理旧歌词源搜索状态失败：{error}"))?;
            transaction
                .execute(
                    "DELETE FROM lyrics_search_candidates WHERE run_id=?1",
                    rusqlite::params![old_run_id],
                )
                .map_err(|error| format!("清理旧歌词搜索候选失败：{error}"))?;
            transaction
                .execute(
                    "DELETE FROM lyrics_search_runs WHERE run_id=?1",
                    rusqlite::params![old_run_id],
                )
                .map_err(|error| format!("清理旧歌词搜索摘要失败：{error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("提交歌词搜索摘要失败：{error}"))
    }

    pub(crate) fn fail_lyrics_search_run(
        &self,
        run_id: &str,
        error_code: &str,
        total_elapsed_ms: u64,
    ) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute(
                "UPDATE lyrics_search_runs SET status='failed', finished_at=unixepoch(), error_code=?2,
                 total_elapsed_ms=?3
                 WHERE run_id=?1",
                rusqlite::params![run_id, error_code, total_elapsed_ms.min(i64::MAX as u64) as i64],
            )
            .map_err(|error| format!("更新失败的歌词搜索摘要失败：{error}"))?;
        Ok(())
    }

    pub(crate) fn record_lyrics_search_stage(
        &self,
        run_id: &str,
        stage: &str,
        provider_id: Option<&str>,
        status: &str,
        elapsed_ms: u64,
        candidate_count: usize,
    ) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let provider_key = provider_id.unwrap_or_default();
        connection
            .execute(
                "INSERT INTO lyrics_search_stages
                   (run_id, stage, provider_id, provider_key, status, started_at, finished_at,
                    elapsed_ms, candidate_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, unixepoch(),
                         CASE WHEN ?5='running' THEN NULL ELSE unixepoch() END, ?6, ?7)
                 ON CONFLICT(run_id, stage, provider_key) DO UPDATE SET
                   provider_id=excluded.provider_id, status=excluded.status,
                   finished_at=CASE WHEN excluded.status='running'
                                    THEN lyrics_search_stages.finished_at
                                    ELSE excluded.finished_at END,
                   elapsed_ms=excluded.elapsed_ms, candidate_count=excluded.candidate_count",
                rusqlite::params![
                    run_id,
                    stage,
                    provider_id,
                    provider_key,
                    status,
                    elapsed_ms.min(i64::MAX as u64) as i64,
                    candidate_count.min(i64::MAX as usize) as i64,
                ],
            )
            .map_err(|error| format!("保存歌词搜索阶段失败：{error}"))?;
        Ok(())
    }

    pub fn latest_lyrics_search_trace(
        &self,
        track_key: &str,
    ) -> Result<Option<LyricsSearchTrace>, String> {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let run = connection
            .query_row(
                "SELECT run_id, recording_id, intent, status, title, artist, started_at,
                        finished_at, total_elapsed_ms, error_code, selected_provider_id,
                        selected_provider_item_id, selection_reason
                 FROM lyrics_search_runs
                 WHERE track_key=?1 ORDER BY started_at DESC LIMIT 1",
                rusqlite::params![track_key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("读取最近歌词搜索摘要失败：{error}"))?;
        let Some((
            run_id,
            recording_id,
            intent,
            status,
            title,
            artist,
            started_at,
            finished_at,
            total_elapsed_ms,
            error_code,
            selected_provider_id,
            selected_provider_item_id,
            selection_reason,
        )) = run
        else {
            return Ok(None);
        };
        let total_elapsed_ms = total_elapsed_ms.map(|value: i64| value.max(0) as u64);
        let mut statement = connection
            .prepare(
                "SELECT provider_id, provider_item_id, title, artists_json, album, duration_ms,
                        version_tags_json, score, state, rejection_reason, selection_reason,
                        score_evidence_json
                 FROM lyrics_search_candidates WHERE run_id=?1 ORDER BY score DESC, candidate_id",
            )
            .map_err(|error| format!("读取歌词搜索候选失败：{error}"))?;
        let candidates = statement
            .query_map(rusqlite::params![run_id], |row| {
                let artists_json = row.get::<_, String>(3)?;
                let version_tags_json = row.get::<_, String>(6)?;
                let score_evidence_json = row.get::<_, String>(11)?;
                Ok(LyricsSearchTraceCandidate {
                    provider_id: row.get(0)?,
                    provider_item_id: row.get(1)?,
                    title: row.get(2)?,
                    artists: serde_json::from_str(&artists_json).unwrap_or_default(),
                    album: row.get(4)?,
                    duration_ms: row
                        .get::<_, Option<i64>>(5)?
                        .map(|value| value.max(0) as u64),
                    version_tags: serde_json::from_str(&version_tags_json).unwrap_or_default(),
                    score: row.get(7)?,
                    state: row.get(8)?,
                    rejection_reason: row.get(9)?,
                    selection_reason: row.get(10)?,
                    score_evidence: serde_json::from_str(&score_evidence_json).ok(),
                })
            })
            .map_err(|error| format!("解析歌词搜索候选失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析歌词搜索候选失败：{error}"))?;
        let mut provider_statement = connection
            .prepare(
                "SELECT provider_id, status, candidate_count, elapsed_ms
                 FROM lyrics_search_providers WHERE run_id=?1 ORDER BY provider_id",
            )
            .map_err(|error| format!("读取歌词源搜索状态失败：{error}"))?;
        let providers = provider_statement
            .query_map(rusqlite::params![run_id], |row| {
                Ok(LyricsSearchTraceProvider {
                    provider_id: row.get(0)?,
                    status: row.get(1)?,
                    candidate_count: row.get::<_, i64>(2)?.max(0) as usize,
                    elapsed_ms: row.get::<_, i64>(3)?.max(0) as u64,
                })
            })
            .map_err(|error| format!("解析歌词源搜索状态失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析歌词源搜索状态失败：{error}"))?;
        let mut stage_statement = connection
            .prepare(
                "SELECT stage, provider_id, status, started_at, finished_at, elapsed_ms,
                        candidate_count
                 FROM lyrics_search_stages WHERE run_id=?1 ORDER BY stage_id",
            )
            .map_err(|error| format!("读取歌词搜索阶段失败：{error}"))?;
        let stages = stage_statement
            .query_map(rusqlite::params![run_id], |row| {
                Ok(LyricsSearchTraceStage {
                    stage: row.get(0)?,
                    provider_id: row.get(1)?,
                    status: row.get(2)?,
                    started_at: row.get(3)?,
                    finished_at: row.get(4)?,
                    elapsed_ms: row.get::<_, i64>(5)?.max(0) as u64,
                    candidate_count: row.get::<_, i64>(6)?.max(0) as usize,
                })
            })
            .map_err(|error| format!("解析歌词搜索阶段失败：{error}"))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| format!("解析歌词搜索阶段失败：{error}"))?;
        Ok(Some(LyricsSearchTrace {
            run_id,
            recording_id,
            intent,
            status,
            title,
            artist,
            started_at,
            finished_at,
            total_elapsed_ms,
            error_code,
            selected_provider_id,
            selected_provider_item_id,
            selection_reason,
            providers,
            stages,
            candidates,
        }))
    }
}
