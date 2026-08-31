impl Storage {
    pub fn save(&self, request: SaveRequest<'_>) -> Result<LyricsDocument, String> {
        let SaveRequest {
            track_key,
            title,
            artist,
            source,
            raw,
            provider_id,
            provider_item_id,
            kind,
        } = request;
        let canonical_track_key = self.canonical_track_key(track_key)?;
        let existing = self.association(&canonical_track_key)?;
        if kind == SaveKind::Automatic
            && existing
                .as_ref()
                .is_some_and(|association| association.manual_selected)
        {
            return self
                .load(track_key)?
                .ok_or_else(|| "受保护的歌词关联无法读取".into());
        }

        let mut document = parse_lrc_with_options(raw, source, kind.is_manual())?;
        if document.metadata.title.is_none() {
            document.metadata.title = Some(title.into());
        }
        if document.metadata.artist.is_none() {
            document.metadata.artist = Some(artist.into());
        }
        let library_dir = self
            .library_dir
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let reusable_path = existing
            .as_ref()
            .map(|association| association.path.clone())
            .filter(|path| path.starts_with(&library_dir))
            .filter(|path| self.file_is_app_owned(path).unwrap_or(false))
            .filter(|path| lyric_path_matches_format(path, &document.metadata.original_format));
        let path = reusable_path
            .clone()
            .unwrap_or_else(|| {
                available_path(
                    &library_dir,
                    title,
                    artist,
                    raw,
                    &document.metadata.original_format,
                )
            });
        fs::write(&path, raw).map_err(|error| format!("保存歌词文件失败：{error}"))?;
        let content_hash = content_hash(raw);
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute(
                "INSERT INTO lyric_associations
                   (track_key, title, artist, source, content_path, offset_ms, original_format,
                    manual_selected, provider_id, provider_item_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, unixepoch())
                 ON CONFLICT(track_key) DO UPDATE SET
                   title=excluded.title, artist=excluded.artist, source=excluded.source,
                   content_path=excluded.content_path, original_format=excluded.original_format,
                   manual_selected=excluded.manual_selected, provider_id=excluded.provider_id,
                   provider_item_id=excluded.provider_item_id, updated_at=unixepoch()",
                params![
                    canonical_track_key,
                    title,
                    artist,
                    source,
                    path.to_string_lossy(),
                    document.offset_ms,
                    document.metadata.original_format.as_str(),
                    kind.is_manual(),
                    provider_id,
                    provider_item_id
                ],
            )
            .map_err(|error| format!("保存歌词关联失败：{error}"))?;
        upsert_file_index(
            &connection,
            &path,
            title,
            artist,
            source,
            &document.metadata.original_format,
            kind.is_manual(),
            &content_hash,
        )?;
        connection
            .execute(
                "INSERT INTO lyric_history (track_key, title, artist, source, used_at)
                 VALUES (?1, ?2, ?3, ?4, unixepoch())",
                params![canonical_track_key, title, artist, source],
            )
            .map_err(|error| format!("保存歌词使用记录失败：{error}"))?;
        connection
            .execute(
                "DELETE FROM lyric_history WHERE id NOT IN (
                   SELECT id FROM lyric_history ORDER BY used_at DESC, id DESC LIMIT 100
                 )",
                [],
            )
            .map_err(|error| format!("整理歌词使用记录失败：{error}"))?;
        drop(connection);
        if let Some(old_path) = existing
            .as_ref()
            .map(|association| association.path.clone())
            .filter(|old_path| old_path != &path)
        {
            if let Err(error) = self.cleanup_unreferenced_app_owned_files([old_path]) {
                log::warn!("整理旧歌词文件失败：{error}");
            }
        }
        self.load(&canonical_track_key)?
            .ok_or_else(|| "歌词保存后无法读取".into())
    }

    pub fn search_local_lyrics(
        &self,
        input: &LyricsSearchInput,
    ) -> Result<Vec<LyricsSearchResult>, String> {
        let library_dir = self.library_directory();
        let indexed = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mut statement = connection
                .prepare(
                    // 在线歌词保存到歌词目录后仍是缓存，不应以本地候选参与在线结果去重。
                    "SELECT content_path, title, artist, duration_ms, content_hash
                     FROM lyric_files
                     WHERE managed=1
                       AND (app_owned=0 OR source IN ('本地文件', '本地导入', '手动导入'))",
                )
                .map_err(|error| format!("读取本地歌词索引失败：{error}"))?;
            let rows = statement
                .query_map([], |row| {
                    Ok(LocalLyricsCandidate {
                        path: PathBuf::from(row.get::<_, String>(0)?),
                        title: row.get(1)?,
                        artist: row.get(2)?,
                        duration_ms: row.get::<_, Option<i64>>(3)?.map(|value| value as u64),
                        content_hash: row.get(4)?,
                        score: 0.0,
                    })
                })
                .map_err(|error| format!("查询本地歌词索引失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析本地歌词索引失败：{error}"))?;
            rows
        };

        let mut candidates = indexed
            .into_iter()
            .filter(|candidate| candidate.path.starts_with(&library_dir))
            .filter_map(|mut candidate| {
                let result = LyricsSearchResult {
                    id: candidate.path.to_string_lossy().into_owned(),
                    provider_id: LOCAL_PROVIDER_ID.into(),
                    title: candidate.title.clone(),
                    artist: candidate.artist.clone(),
                    album: None,
                    duration_ms: candidate.duration_ms,
                    source: LOCAL_FILE_SOURCE.into(),
                    synced: true,
                    has_translation: false,
                    has_word_timing: false,
                    has_romanization: false,
                    score: 0.0,
                    lyrics: String::new(),
                };
                if !title_matches(input, &result) {
                    return None;
                }
                candidate.score = score_candidate(input, &result);
                (candidate.score >= MIN_LOCAL_SEARCH_SCORE).then_some(candidate)
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| right.score.total_cmp(&left.score));

        let mut seen_content = HashSet::new();
        let mut results = Vec::with_capacity(MAX_LOCAL_SEARCH_RESULTS);
        for candidate in candidates {
            if results.len() >= MAX_LOCAL_SEARCH_RESULTS {
                break;
            }
            let Ok(raw) = read_lyric_text(&candidate.path) else {
                continue;
            };
            let content_key = if candidate.content_hash.is_empty() {
                content_hash(&raw)
            } else {
                candidate.content_hash.clone()
            };
            if !seen_content.insert(content_key) {
                continue;
            }
            let Ok(document) = parse_lrc_with_options(&raw, LOCAL_FILE_SOURCE, false) else {
                continue;
            };
            let duration_ms = candidate.duration_ms.or_else(|| {
                document
                    .tracks
                    .original
                    .lines
                    .last()
                    .map(|line| line.end_ms.unwrap_or(line.start_ms))
            });
            results.push(LyricsSearchResult {
                id: candidate.path.to_string_lossy().into_owned(),
                provider_id: LOCAL_PROVIDER_ID.into(),
                title: candidate.title,
                artist: candidate.artist,
                album: None,
                duration_ms,
                source: LOCAL_FILE_SOURCE.into(),
                synced: true,
                has_translation: document.tracks.translation.is_some(),
                has_word_timing: document
                    .tracks
                    .original
                    .lines
                    .iter()
                    .any(|line| line.words.as_ref().is_some_and(|words| !words.is_empty())),
                has_romanization: document.tracks.romanization.is_some(),
                score: candidate.score,
                lyrics: raw,
            });
        }
        Ok(results)
    }

    pub fn associate_local_lyrics(
        &self,
        request: SaveRequest<'_>,
    ) -> Result<LyricsDocument, String> {
        let SaveRequest {
            track_key,
            title,
            artist,
            provider_item_id,
            kind,
            ..
        } = request;
        let canonical_track_key = self.canonical_track_key(track_key)?;
        let existing = self.association(&canonical_track_key)?;
        if kind == SaveKind::Automatic
            && existing
                .as_ref()
                .is_some_and(|association| association.manual_selected)
        {
            return self
                .load(track_key)?
                .ok_or_else(|| "受保护的歌词关联无法读取".into());
        }

        let requested_path = provider_item_id.ok_or_else(|| "本地歌词缺少索引标识".to_string())?;
        let library_dir = self.library_directory();
        let (path, original_format) = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            connection
                .query_row(
                    "SELECT content_path, original_format FROM lyric_files
                     WHERE content_path=?1 AND managed=1",
                    params![requested_path],
                    |row| {
                        Ok((
                            PathBuf::from(row.get::<_, String>(0)?),
                            row.get::<_, String>(1)?,
                        ))
                    },
                )
                .optional()
                .map_err(|error| format!("读取本地歌词索引失败：{error}"))?
                .ok_or_else(|| "本地歌词已不在索引中，请重新扫描".to_string())?
        };
        if !path.starts_with(&library_dir) || !path.is_file() {
            return Err("本地歌词文件不在当前歌词目录中".into());
        }

        let raw = read_lyric_text(&path)?;
        let mut document = parse_lrc_with_options(&raw, LOCAL_FILE_SOURCE, kind.is_manual())?;
        document.metadata.title = Some(title.into());
        document.metadata.artist = Some(artist.into());
        document.metadata.original_format = original_format.clone();

        let connection = self
            .connection
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        connection
            .execute(
                "INSERT INTO lyric_associations
                   (track_key, title, artist, source, content_path, offset_ms, original_format,
                    manual_selected, provider_id, provider_item_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, unixepoch())
                 ON CONFLICT(track_key) DO UPDATE SET
                   title=excluded.title, artist=excluded.artist, source=excluded.source,
                   content_path=excluded.content_path, offset_ms=excluded.offset_ms,
                   original_format=excluded.original_format,
                   manual_selected=excluded.manual_selected, provider_id=excluded.provider_id,
                   provider_item_id=excluded.provider_item_id, updated_at=unixepoch()",
                params![
                    canonical_track_key,
                    title,
                    artist,
                    LOCAL_FILE_SOURCE,
                    path.to_string_lossy(),
                    document.offset_ms,
                    original_format,
                    kind.is_manual(),
                    LOCAL_PROVIDER_ID,
                    path.to_string_lossy(),
                ],
            )
            .map_err(|error| format!("保存本地歌词关联失败：{error}"))?;
        connection
            .execute(
                "INSERT INTO lyric_history (track_key, title, artist, source, used_at)
                 VALUES (?1, ?2, ?3, ?4, unixepoch())",
                params![canonical_track_key, title, artist, LOCAL_FILE_SOURCE],
            )
            .map_err(|error| format!("保存歌词使用记录失败：{error}"))?;
        connection
            .execute(
                "DELETE FROM lyric_history WHERE id NOT IN (
                   SELECT id FROM lyric_history ORDER BY used_at DESC, id DESC LIMIT 100
                 )",
                [],
            )
            .map_err(|error| format!("整理歌词使用记录失败：{error}"))?;
        drop(connection);
        if let Some(old_path) = existing
            .as_ref()
            .map(|association| association.path.clone())
            .filter(|old_path| old_path != &path)
        {
            if let Err(error) = self.cleanup_unreferenced_app_owned_files([old_path]) {
                log::warn!("整理旧歌词文件失败：{error}");
            }
        }
        Ok(document)
    }
}

fn lyric_path_matches_format(path: &Path, original_format: &str) -> bool {
    let is_lyricsfile = path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.to_ascii_lowercase().ends_with(".lyricsfile.yaml"));
    (original_format == "lyricsfile") == is_lyricsfile
}
