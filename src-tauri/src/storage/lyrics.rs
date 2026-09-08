impl Storage {
    /// 删除数据库明确标记为应用拥有的在线歌词文件。
    ///
    /// 手动导入和外部目录索引会使用用户拥有的来源标记，因此不会进入此列表。
    pub(crate) fn remove_application_downloads(&self) -> Result<usize, String> {
        let scan = self.scanner.snapshot();
        self.scanner.cancel_if_matches(Path::new(&scan.library_dir));

        let paths = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mut statement = connection
                .prepare(
                    "SELECT DISTINCT content_path
                     FROM lyric_files
                     WHERE app_owned=1
                       AND source NOT IN ('本地文件', '本地导入', '手动导入')",
                )
                .map_err(|error| format!("读取应用下载歌词失败：{error}"))?;
            let paths = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|error| format!("查询应用下载歌词失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析应用下载歌词失败：{error}"))?
                .into_iter()
                .map(PathBuf::from)
                .filter(|path| is_application_lyric_path(path))
                .collect::<Vec<_>>();
            paths
        };

        let mut deleted = 0;
        for path in paths {
            if !path.is_absolute() {
                return Err(format!("应用下载歌词路径无效：{}", path.display()));
            }
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!("读取应用下载歌词失败 {}：{error}", path.display()))
                }
            };
            if !metadata.file_type().is_file() {
                return Err(format!("应用下载歌词路径不是文件：{}", path.display()));
            }
            fs::remove_file(&path)
                .map_err(|error| format!("删除应用下载歌词失败 {}：{error}", path.display()))?;
            deleted += 1;
        }
        Ok(deleted)
    }

    /// 在应用托管目录中复用同一正文的已有文件；找不到时为新版本分配稳定文件名。
    ///
    /// 这里只复用应用拥有的资源，避免把用户导入或外部目录文件改成应用托管文件。
    fn select_managed_lyric_path(
        &self,
        title: &str,
        artist: &str,
        raw: &str,
        original_format: &str,
    ) -> Result<(PathBuf, bool), String> {
        let fingerprint = content_hash(raw);
        let library_dir = self.library_directory();
        let candidates = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mut statement = connection
                .prepare(
                    "SELECT content_path FROM lyric_files
                     WHERE content_hash=?1 AND managed=1 AND app_owned=1
                     ORDER BY updated_at DESC, content_path ASC",
                )
                .map_err(|error| format!("读取可复用歌词文件失败：{error}"))?;
            let candidates = statement
                .query_map(rusqlite::params![fingerprint], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|error| format!("查询可复用歌词文件失败：{error}"))?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|error| format!("解析可复用歌词文件失败：{error}"))?;
            candidates
        };
        if let Some(path) = candidates.into_iter().map(PathBuf::from).find(|path| {
            path.strip_prefix(&library_dir).is_ok()
                && lyric_path_matches_format(path, original_format)
                && path.is_file()
                && read_lyric_text(path).ok().as_deref() == Some(raw)
        }) {
            return Ok((path, false));
        }

        Ok((
            available_path(&library_dir, title, artist, raw, original_format)?,
            true,
        ))
    }

    pub fn save(&self, request: SaveRequest<'_>) -> Result<LyricsDocument, String> {
        let SaveRequest {
            track_key,
            title,
            artist,
            album,
            duration_ms,
            source,
            raw,
            provider_id,
            provider_item_id,
            confidence,
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
        let (path, needs_write) =
            self.select_managed_lyric_path(title, artist, raw, &document.metadata.original_format)?;
        if needs_write {
            atomic_write_lyric(&path, raw)?;
        }
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
        let source_kind = if kind == SaveKind::Import {
            "local"
        } else {
            "managed"
        };
        if let Err(error) = self.sync_v2_lyric_binding(
            track_key,
            title,
            artist,
            album,
            duration_ms,
            source,
            raw,
            &path,
            &document.metadata.original_format,
            provider_id,
            provider_item_id,
            confidence,
            kind,
            source_kind,
            document.offset_ms,
            document.tracks.translation.is_some(),
            document
                .tracks
                .original
                .lines
                .iter()
                .any(|line| line.words.as_ref().is_some_and(|words| !words.is_empty())),
            document.tracks.romanization.is_some(),
        ) {
            log::warn!("同步新歌词绑定失败，保留旧结构：{error}");
        }
        // 平台移出后，旧 canonical 别名仍可能指向原歌曲；按精确曲目读取新绑定。
        self.load(track_key)?
            .ok_or_else(|| "歌词保存后无法读取".into())
    }

    pub fn search_local_lyrics(
        &self,
        input: &LyricsSearchInput,
    ) -> Result<Vec<LyricsSearchResult>, String> {
        let indexed = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let roots = connection
                .prepare("SELECT root_kind, path FROM library_roots WHERE enabled=1")
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
                .map_err(|error| format!("读取本地歌词根目录失败：{error}"))?;
            let mut statement = connection
                .prepare(
                    // 在线歌词保存到托管目录后仍是应用资源，不应以本地候选参与在线结果去重。
                    "SELECT content_path, title, artist, duration_ms, content_hash
                     FROM lyric_files WHERE managed=1 AND available=1",
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
            rows.into_iter()
                .filter(|candidate| is_searchable_local_candidate(&candidate.path, &roots))
                .collect::<Vec<_>>()
        };

        let mut candidates = indexed
            .into_iter()
            .filter_map(|mut candidate| {
                let result = LyricsSearchResult {
                    id: local_provider_item_id(&candidate.content_hash),
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
            if !seen_content.insert(content_key.clone()) {
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
                id: local_provider_item_id(&content_key),
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
            album,
            duration_ms,
            provider_item_id,
            confidence,
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

        let requested_id = provider_item_id.ok_or_else(|| "本地歌词缺少索引标识".to_string())?;
        let (path, original_format) = {
            let connection = self
                .connection
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let roots = connection
                .prepare("SELECT root_kind, path FROM library_roots WHERE enabled=1")
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
                .map_err(|error| format!("读取本地歌词根目录失败：{error}"))?;
            let row = if let Some(content_hash) = requested_id.strip_prefix("local:") {
                // 内容哈希可能同时出现在应用目录和本地目录，必须从同哈希文件中选出本地根目录内的那一份。
                let mut statement = connection
                    .prepare(
                        "SELECT content_path, original_format
                         FROM lyric_files
                         WHERE content_hash=?1 AND managed=1 AND available=1
                         ORDER BY updated_at DESC",
                    )
                    .map_err(|error| format!("读取本地歌词索引失败：{error}"))?;
                let candidate = statement
                    .query_map(params![content_hash], |row| {
                        Ok((
                            PathBuf::from(row.get::<_, String>(0)?),
                            row.get::<_, String>(1)?,
                        ))
                    })
                    .map_err(|error| format!("查询本地歌词索引失败：{error}"))?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(|error| format!("解析本地歌词索引失败：{error}"))?
                    .into_iter()
                    .find(|(path, _)| is_searchable_local_candidate(path, &roots));
                candidate
            } else {
                // 兼容旧前端传入的绝对路径，但仍会经过根目录和可用状态校验。
                connection
                    .query_row(
                        "SELECT content_path, original_format
                         FROM lyric_files
                         WHERE content_path=?1 AND managed=1 AND available=1",
                        params![requested_id],
                        |row| {
                            Ok((
                                PathBuf::from(row.get::<_, String>(0)?),
                                row.get::<_, String>(1)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(|error| format!("读取本地歌词索引失败：{error}"))?
                    .filter(|(path, _)| is_searchable_local_candidate(path, &roots))
            };
            row.ok_or_else(|| "本地歌词不属于已启用的只读目录".to_string())?
        };
        if !path.is_file() {
            return Err("本地歌词文件已不可用".into());
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
                    requested_id,
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
        if let Err(error) = self.sync_v2_lyric_binding(
            track_key,
            title,
            artist,
            album,
            duration_ms,
            LOCAL_FILE_SOURCE,
            &raw,
            &path,
            &original_format,
            Some(LOCAL_PROVIDER_ID),
            Some(requested_id),
            confidence,
            kind,
            "local",
            document.offset_ms,
            document.tracks.translation.is_some(),
            document
                .tracks
                .original
                .lines
                .iter()
                .any(|line| line.words.as_ref().is_some_and(|words| !words.is_empty())),
            document.tracks.romanization.is_some(),
        ) {
            log::warn!("同步本地歌词新绑定失败，保留旧结构：{error}");
        }
        Ok(document)
    }
}

fn is_application_lyric_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    file_name.ends_with(".lyricsfile.yaml")
        || path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("lrc"))
}

fn lyric_path_matches_format(path: &Path, original_format: &str) -> bool {
    let is_lyricsfile = path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.to_ascii_lowercase().ends_with(".lyricsfile.yaml"));
    (original_format == "lyricsfile") == is_lyricsfile
}

fn local_provider_item_id(content_hash: &str) -> String {
    format!(
        "local:{}",
        if content_hash.trim().is_empty() {
            "unknown"
        } else {
            content_hash
        }
    )
}

fn is_searchable_local_candidate(path: &Path, roots: &[(String, PathBuf)]) -> bool {
    let matching_root = roots
        .iter()
        .filter(|(_, root_path)| path.starts_with(root_path))
        .max_by_key(|(_, root_path)| root_path.as_os_str().len());
    match matching_root.map(|(kind, _)| kind.as_str()) {
        // “本地歌词”只代表用户明确绑定的本地目录；应用下载目录、历史目录
        // 和应用管理目录中的导入文件不会混入该候选分组。
        Some("local") => true,
        _ => false,
    }
}
