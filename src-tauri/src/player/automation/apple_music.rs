use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::super::{run_with_timeout, PlaybackSnapshot, PlayerKind};

const APP_PATH: &str = "/System/Applications/Music.app";
const ARTWORK_SCRIPT: &str = r#"
on run argv
  set expectedId to item 1 of argv
  set expectedTitle to item 2 of argv
  set expectedArtist to item 3 of argv
  set expectedAlbum to item 4 of argv
  set outputPath to item 5 of argv
  set fileHandle to missing value

  tell application "Music"
    if not running then return "unavailable"
    set currentTrackRef to current track
    if currentTrackRef is missing value then return "unavailable"
    if expectedId starts with "fallback:" then
      if (name of currentTrackRef as text) is not expectedTitle then return "stale"
      if (artist of currentTrackRef as text) is not expectedArtist then return "stale"
      if (album of currentTrackRef as text) is not expectedAlbum then return "stale"
    else
      if (persistent ID of currentTrackRef as text) is not expectedId then return "stale"
    end if
    if (count of artworks of currentTrackRef) is 0 then return "missing"
    set artworkData to raw data of artwork 1 of currentTrackRef
  end tell

  try
    set outputFile to POSIX file outputPath
    set fileHandle to open for access outputFile with write permission
    set eof fileHandle to 0
    write artworkData to fileHandle
    close access fileHandle
    return "ok"
  on error errorMessage
    if fileHandle is not missing value then
      try
        close access fileHandle
      end try
    end if
    return "error:" & errorMessage
  end try
end run
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtworkExport {
    Exported,
    Missing,
    Stale,
    Unavailable,
}

pub(super) fn snapshot() -> PlaybackSnapshot {
    super::query(
        PlayerKind::AppleMusic,
        "apple_music",
        "Apple Music",
        Path::new(APP_PATH),
        1000,
        "persistentID",
    )
}

pub(super) fn perform_action(action: &str, position_ms: Option<u64>) -> Result<(), String> {
    super::perform_action_for_app("Music", action, position_ms)
}

pub(crate) fn export_artwork(
    expected_id: &str,
    title: &str,
    artist: &str,
    album: &str,
    output_path: &Path,
) -> Result<ArtworkExport, String> {
    let mut command = Command::new("/usr/bin/osascript");
    command
        .args(["-e", ARTWORK_SCRIPT])
        .arg("--")
        .arg(expected_id)
        .arg(title)
        .arg(artist)
        .arg(album)
        .arg(output_path);
    let output = run_with_timeout(command, Duration::from_secs(4))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if let Some(error) = status.strip_prefix("error:") {
        return Err(error.trim().to_string());
    }
    let result = parse_artwork_status(&status, output_path)?;
    let status = match result {
        ArtworkExport::Exported => "ok",
        ArtworkExport::Missing => "missing",
        ArtworkExport::Stale => "stale",
        ArtworkExport::Unavailable => "unavailable",
    };
    log::debug!("Track artwork source lookup completed: player=apple_music status={status}");
    Ok(result)
}

fn parse_artwork_status(status: &str, output_path: &Path) -> Result<ArtworkExport, String> {
    match status {
        "ok" if output_path
            .metadata()
            .map(|metadata| metadata.is_file() && metadata.len() > 0)
            .unwrap_or(false) =>
        {
            Ok(ArtworkExport::Exported)
        }
        "ok" => Err("Apple Music artwork export produced an empty file".into()),
        "missing" => Ok(ArtworkExport::Missing),
        "stale" => Ok(ArtworkExport::Stale),
        "unavailable" => Ok(ArtworkExport::Unavailable),
        value => Err(format!(
            "Apple Music artwork script returned an unknown status: {value}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[cfg(target_os = "macos")]
    #[test]
    fn artwork_script_compiles() {
        let root = tempdir().unwrap();
        let output_path = root.path().join("apple-music-artwork.scpt");
        let output = Command::new("/usr/bin/osacompile")
            .arg("-o")
            .arg(&output_path)
            .arg("-e")
            .arg(ARTWORK_SCRIPT)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Apple Music 封面脚本编译失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        assert!(output_path.exists());
    }

    #[test]
    fn parses_artwork_export_statuses() {
        let root = tempdir().unwrap();
        let output_path = root.path().join("artwork.tmp");

        assert_eq!(
            parse_artwork_status("missing", &output_path).unwrap(),
            ArtworkExport::Missing
        );
        assert_eq!(
            parse_artwork_status("stale", &output_path).unwrap(),
            ArtworkExport::Stale
        );
        assert_eq!(
            parse_artwork_status("unavailable", &output_path).unwrap(),
            ArtworkExport::Unavailable
        );
        assert!(parse_artwork_status("ok", &output_path).is_err());
        assert!(parse_artwork_status("unexpected", &output_path).is_err());

        fs::write(&output_path, b"artwork").unwrap();
        assert_eq!(
            parse_artwork_status("ok", &output_path).unwrap(),
            ArtworkExport::Exported
        );
    }
}
