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
) -> Result<PathBuf, String> {
    let stem = format!("{} - {}", safe_component(artist), safe_component(title));
    let extension = if original_format == "lyricsfile" {
        ".lyricsfile.yaml"
    } else {
        ".lrc"
    };
    let initial = library_dir.join(format!("{stem}{extension}"));
    if !path_is_occupied(&initial)? {
        return Ok(initial);
    }
    let digest = stable_filename_hash(raw);
    for prefix_length in (8..=64).step_by(4) {
        let prefix = &digest[..prefix_length];
        let candidate = library_dir.join(format!("{stem} [{prefix}]{extension}"));
        if !path_is_occupied(&candidate)? {
            return Ok(candidate);
        }
    }
    Err(format!(
        "歌词文件名哈希冲突，无法安全保存：{stem}{extension}"
    ))
}

pub(super) fn content_hash(raw: &str) -> String {
    let mut hasher = DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// 用稳定的 SHA-256 摘要区分同名但内容不同的托管歌词版本。
pub(super) fn stable_filename_hash(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

fn path_is_occupied(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("检查歌词文件路径失败 {}：{error}", path.display())),
    }
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

pub(super) fn atomic_write_lyric(path: &Path, raw: &str) -> Result<(), String> {
    if path_is_occupied(path)? {
        return Err(format!("目标歌词文件已存在，拒绝覆盖：{}", path.display()));
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("lyrics");
    let temporary =
        path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));
    if let Err(error) = fs::write(&temporary, raw) {
        return Err(format!("保存歌词临时文件失败：{error}"));
    }
    match path_is_occupied(path) {
        Ok(true) => {
            let _ = fs::remove_file(&temporary);
            return Err(format!("目标歌词文件已存在，拒绝覆盖：{}", path.display()));
        }
        Ok(false) => {}
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("原子替换歌词文件失败：{error}"));
    }
    Ok(())
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
               content_hash, app_owned, available, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, unixepoch())
             ON CONFLICT(content_path) DO UPDATE SET
               title=excluded.title, artist=excluded.artist, source=excluded.source,
               original_format=excluded.original_format, manual_selected=excluded.manual_selected,
               content_hash=excluded.content_hash, app_owned=excluded.app_owned,
               available=1, updated_at=unixepoch()",
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
    connection: &mut Connection,
    legacy_dir: &Path,
    library_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // 旧目录在 V2 中只作为历史只读来源登记；迁移阶段不再复制、覆盖或删除用户文件。
    let _ = (legacy_dir, library_dir);
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

    let transaction = connection.transaction()?;
    for (track_key, title, artist, source, path, manual_selected) in rows {
        if !path.is_file() {
            continue;
        }
        let raw = read_lyric_text(&path)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        let original_format = parse_lrc_with_options(&raw, &source, manual_selected)
            .map(|document| document.metadata.original_format)
            .unwrap_or_else(|_| "lrc".into());
        upsert_file_index(
            &transaction,
            &path,
            &title,
            &artist,
            &source,
            &original_format,
            manual_selected,
            &content_hash(&raw),
        )?;
        transaction.execute(
            "UPDATE lyric_associations SET content_path=?2, updated_at=updated_at
             WHERE track_key=?1",
            params![track_key, path.to_string_lossy()],
        )?;
    }
    transaction.commit()?;
    Ok(())
}
