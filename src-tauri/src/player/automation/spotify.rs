use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use super::super::{run_with_timeout, PlaybackSnapshot, PlayerKind};

const ARTWORK_SCRIPT: &str = r#"
ObjC.import('Foundation');
function value(callable, fallback) {
  try { const result = callable(); return result === undefined || result === null ? fallback : result; }
  catch (_) { return fallback; }
}
const environment = $.NSProcessInfo.processInfo.environment;
const appPath = environment.objectForKey('LYRICS_PLUS_APP_PATH').js;
const expectedId = environment.objectForKey('LYRICS_PLUS_TRACK_ID').js;
const expectedTitle = environment.objectForKey('LYRICS_PLUS_TRACK_TITLE').js;
const expectedArtist = environment.objectForKey('LYRICS_PLUS_TRACK_ARTIST').js;
const expectedAlbum = environment.objectForKey('LYRICS_PLUS_TRACK_ALBUM').js;
const app = Application(appPath);
if (!value(() => app.running(), false)) {
  JSON.stringify({ status: 'unavailable' });
} else {
  const track = value(() => app.currentTrack(), null);
  const trackId = track ? String(value(() => track.id(), '')) : '';
  const matches = track && (expectedId.startsWith('fallback:')
    ? String(value(() => track.name(), '')) === expectedTitle
      && String(value(() => track.artist(), '')) === expectedArtist
      && String(value(() => track.album(), '')) === expectedAlbum
    : trackId === expectedId);
  if (!matches) {
    JSON.stringify({ status: 'stale' });
  } else {
    JSON.stringify({ status: 'ok', url: value(() => track.artworkUrl(), null) });
  }
}
"#;

#[derive(Debug, Deserialize)]
struct ArtworkResponse {
    status: String,
    url: Option<String>,
}

pub(super) fn snapshot() -> PlaybackSnapshot {
    let app_path = app_path();
    super::query(
        PlayerKind::Spotify,
        "spotify",
        "Spotify",
        &app_path,
        1,
        "id",
    )
}

pub(super) fn perform_action(action: &str, position_ms: Option<u64>) -> Result<(), String> {
    super::perform_action_for_app("Spotify", action, position_ms)
}

pub(crate) fn artwork_url(
    expected_id: &str,
    title: &str,
    artist: &str,
    album: &str,
) -> Result<Option<String>, String> {
    let app_path = app_path();
    if !app_path.exists() {
        log::debug!("Track artwork source lookup completed: player=spotify status=unavailable");
        return Ok(None);
    }
    let mut command = Command::new("/usr/bin/osascript");
    command
        .args(["-l", "JavaScript", "-e", ARTWORK_SCRIPT])
        .env("LYRICS_PLUS_APP_PATH", app_path)
        .env("LYRICS_PLUS_TRACK_ID", expected_id)
        .env("LYRICS_PLUS_TRACK_TITLE", title)
        .env("LYRICS_PLUS_TRACK_ARTIST", artist)
        .env("LYRICS_PLUS_TRACK_ALBUM", album);
    let output = run_with_timeout(command, Duration::from_secs(3))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let response: ArtworkResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Failed to parse Spotify artwork response: {error}"))?;
    if response.status != "ok" {
        log::debug!(
            "Track artwork source lookup completed: player=spotify status={}",
            response.status
        );
        return Ok(None);
    }
    let url = response.url.filter(|url| !url.trim().is_empty());
    log::debug!(
        "Track artwork source lookup completed: player=spotify status={}",
        if url.is_some() { "ok" } else { "missing" }
    );
    Ok(url)
}

fn app_path() -> PathBuf {
    let system = PathBuf::from("/Applications/Spotify.app");
    if system.exists() {
        return system;
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Applications/Spotify.app"))
        .unwrap_or(system)
}
