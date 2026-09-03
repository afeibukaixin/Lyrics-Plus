fn migrate_provider_source_names(connection: &Connection) -> rusqlite::Result<()> {
    const SOURCE_TABLES: [&str; 3] = ["lyric_associations", "lyric_files", "lyric_history"];
    let aliases = [
        ("网易云音乐", NETEASE_DISPLAY_NAME),
        ("QQ 音乐", QQMUSIC_DISPLAY_NAME),
        ("QQ音乐", QQMUSIC_DISPLAY_NAME),
        ("酷狗音乐", KUGOU_DISPLAY_NAME),
    ];

    for table in SOURCE_TABLES {
        let statement = format!("UPDATE {table} SET source=?1 WHERE source=?2");
        for (legacy_name, display_name) in aliases {
            connection.execute(&statement, params![display_name, legacy_name])?;
        }
    }
    Ok(())
}
struct Association {
    title: String,
    artist: String,
    source: String,
    path: PathBuf,
    offset_ms: i64,
    original_format: String,
    manual_selected: bool,
}

struct LocalLyricsCandidate {
    path: PathBuf,
    title: String,
    artist: String,
    duration_ms: Option<u64>,
    content_hash: String,
    score: f64,
}

pub(super) fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !columns.iter().any(|existing| existing == column) {
        connection.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
        return Ok(true);
    }
    Ok(false)
}

pub(super) fn safe_component(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|character| {
            if character.is_control() || matches!(character, '/' | '\\' | ':') {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned
        .trim_matches(['.', ' '])
        .chars()
        .take(96)
        .collect::<String>();
    if cleaned.is_empty() {
        "未知".into()
    } else {
        cleaned
    }
}

pub(super) fn available_path(
    library_dir: &Path,
    title: &str,
    artist: &str,
    raw: &str,
    original_format: &str,
) -> PathBuf {
    let stem = format!("{} - {}", safe_component(artist), safe_component(title));
    let extension = if original_format == "lyricsfile" {
        ".lyricsfile.yaml"
    } else {
        ".lrc"
    };
    let initial = library_dir.join(format!("{stem}{extension}"));
    if !initial.exists() || read_lyric_text(&initial).ok().as_deref() == Some(raw) {
        return initial;
    }
    for suffix in 2..10_000 {
        let candidate = library_dir.join(format!("{stem} ({suffix}){extension}"));
        if !candidate.exists() || read_lyric_text(&candidate).ok().as_deref() == Some(raw) {
            return candidate;
        }
    }
    library_dir.join(format!("{stem}-{}{extension}", content_hash(raw)))
}

pub(super) fn content_hash(raw: &str) -> String {
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub(super) fn read_lyric_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("读取歌词文件失败：{error}"))?;
    if bytes.len() > 5 * 1024 * 1024 {
        return Err("歌词文件超过 5 MB".into());
    }
    decode_lyrics_bytes(&bytes)
        .map(|raw| raw.trim_start_matches('\u{feff}').to_string())
        .map_err(|error| format!("读取歌词文件失败：{error}"))
}

pub(super) fn upsert_file_index(
    connection: &Connection,
    path: &Path,
    title: &str,
    artist: &str,
    source: &str,
    original_format: &str,
    manual_selected: bool,
    hash: &str,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO lyric_files
               (content_path, title, artist, source, original_format, manual_selected,
                content_hash, app_owned, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, unixepoch())
             ON CONFLICT(content_path) DO UPDATE SET
               title=excluded.title, artist=excluded.artist, source=excluded.source,
               original_format=excluded.original_format, manual_selected=excluded.manual_selected,
               content_hash=excluded.content_hash, app_owned=excluded.app_owned, updated_at=unixepoch()",
            params![
                path.to_string_lossy(),
                title,
                artist,
                source,
                original_format,
                manual_selected,
                hash,
                !is_user_owned_source(source),
            ],
        )
        .map_err(|error| format!("保存歌词文件索引失败：{error}"))?;
    Ok(())
}

fn migrate_legacy_files(
    connection: &Connection,
    legacy_dir: &Path,
    library_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut statement = connection.prepare(
        "SELECT track_key, title, artist, source, content_path, manual_selected
         FROM lyric_associations",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                PathBuf::from(row.get::<_, String>(4)?),
                row.get::<_, i64>(5)? != 0,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    for (track_key, title, artist, source, old_path, manual_selected) in rows {
        if !old_path.exists() {
            continue;
        }
        let raw = read_lyric_text(&old_path)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        let original_format = parse_lrc_with_options(&raw, &source, manual_selected)
            .map(|document| document.metadata.original_format)
            .unwrap_or_else(|_| "lrc".into());
        let path = if old_path.starts_with(library_dir) {
            old_path.clone()
        } else if old_path.starts_with(legacy_dir) {
            let destination = available_path(library_dir, &title, &artist, &raw, &original_format);
            if destination != old_path && !destination.exists() {
                fs::copy(&old_path, &destination)?;
            }
            connection.execute(
                "UPDATE lyric_associations SET content_path=?2, updated_at=unixepoch()
                 WHERE track_key=?1",
                params![track_key, destination.to_string_lossy()],
            )?;
            if destination.exists() && old_path != destination {
                let _ = fs::remove_file(&old_path);
            }
            destination
        } else {
            old_path.clone()
        };
        upsert_file_index(
            connection,
            &path,
            &title,
            &artist,
            &source,
            &original_format,
            manual_selected,
            &content_hash(&raw),
        )?;
    }
    Ok(())
}
