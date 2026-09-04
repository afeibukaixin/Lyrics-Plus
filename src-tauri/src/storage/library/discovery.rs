use std::fs;
use std::path::{Path, PathBuf};

use super::scan::{LibraryScanCoordinator, LibraryScanStatus};

pub(super) const MAX_LYRIC_FILES: usize = 50_000;

pub(super) fn collect_lyric_files(
    root: &Path,
    output: &mut Vec<PathBuf>,
    skipped: &mut u64,
    coordinator: &LibraryScanCoordinator,
    scan_id: u64,
    publish: &mut impl FnMut(&LibraryScanStatus),
) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|error| format!("无法读取歌词文件夹：{error}"))?;
    for entry in entries {
        if !coordinator.is_current(scan_id) {
            return Ok(());
        }
        let Ok(entry) = entry else {
            *skipped += 1;
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            *skipped += 1;
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if let Err(error) = collect_lyric_files(
                &entry.path(),
                output,
                skipped,
                coordinator,
                scan_id,
                publish,
            ) {
                if error.contains("最多索引 50000 个文件") {
                    return Err(error);
                }
                *skipped += 1;
            }
        } else if file_type.is_file() && has_supported_extension(&entry.path()) {
            push_discovered_file(output, entry.path())?;
            if output.len() % 250 == 0 {
                if let Some(status) = coordinator.update(scan_id, |status| {
                    status.discovered = output.len() as u64;
                    status.skipped = *skipped;
                }) {
                    publish(&status);
                }
            }
        }
    }
    Ok(())
}

pub(super) fn push_discovered_file(output: &mut Vec<PathBuf>, path: PathBuf) -> Result<(), String> {
    if output.len() >= MAX_LYRIC_FILES {
        return Err("单个歌词文件夹最多索引 50000 个文件".into());
    }
    output.push(path);
    Ok(())
}

pub(super) fn has_supported_extension(path: &Path) -> bool {
    if path
        .file_name()
        .and_then(|value| value.to_str())
        .and_then(strip_lyricsfile_suffix)
        .is_some()
    {
        return true;
    }
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "lrc" | "yrc" | "qrc" | "krc" | "ttml" | "lrcx"
            )
        })
}

pub(super) fn strip_lyricsfile_suffix(file_name: &str) -> Option<&str> {
    const SUFFIX: &str = ".lyricsfile.yaml";
    let suffix_start = file_name.len().checked_sub(SUFFIX.len())?;
    let suffix = file_name.get(suffix_start..)?;
    if suffix.eq_ignore_ascii_case(SUFFIX) {
        file_name.get(..suffix_start)
    } else {
        None
    }
}

pub(super) fn lyric_filename_stem(path: &Path) -> Option<&str> {
    let file_name = path.file_name()?.to_str()?;
    strip_lyricsfile_suffix(file_name).or_else(|| path.file_stem()?.to_str())
}

pub(super) fn canonical_directory(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        return Err("请选择歌词文件夹".into());
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("无法访问歌词文件夹：{error}"))?;
    if !canonical.is_dir() {
        return Err("所选路径不是文件夹".into());
    }
    Ok(canonical)
}
