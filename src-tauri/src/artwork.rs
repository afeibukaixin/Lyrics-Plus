use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use image::codecs::jpeg::JpegEncoder;
use image::ImageReader;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::player::{run_with_timeout, PlaybackSnapshot, PlayerKind};

const MAX_SOURCE_BYTES: usize = 10 * 1024 * 1024;
const MAX_ARTWORK_DIMENSION: u32 = 384;
const JPEG_QUALITY: u8 = 85;
const MAX_CACHE_FILES: usize = 200;

const SPOTIFY_ARTWORK_SCRIPT: &str = r#"
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

const APPLE_MUSIC_ARTWORK_SCRIPT: &str = r#"
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkAsset {
    pub player: PlayerKind,
    pub track_id: String,
    pub file_path: String,
}

#[derive(Debug, Deserialize)]
struct SpotifyArtworkResponse {
    status: String,
    url: Option<String>,
}

pub struct ArtworkService {
    cache_dir: PathBuf,
    operation_lock: Mutex<()>,
}

impl ArtworkService {
    pub fn new(cache_dir: PathBuf) -> std::io::Result<Self> {
        fs::create_dir_all(&cache_dir)?;
        Ok(Self {
            cache_dir,
            operation_lock: Mutex::new(()),
        })
    }

    pub async fn resolve(
        &self,
        snapshot: &PlaybackSnapshot,
        http: &reqwest::Client,
    ) -> Result<Option<ArtworkAsset>, String> {
        let Some(player) = snapshot.player else {
            return Ok(None);
        };
        let Some(track_id) = snapshot.track_id.as_deref() else {
            return Ok(None);
        };
        if track_id.trim().is_empty() {
            return Ok(None);
        }

        let _guard = self.operation_lock.lock().await;
        let final_path = self.cache_path(player, track_id);
        if is_nonempty_file(&final_path) {
            return Ok(Some(asset(player, track_id, &final_path)));
        }

        let title = snapshot.title.clone().unwrap_or_default();
        let artist = snapshot.artist.clone().unwrap_or_default();
        let album = snapshot.album.clone().unwrap_or_default();

        let source = match player {
            PlayerKind::Spotify => {
                let expected_id = track_id.to_string();
                let url = tauri::async_runtime::spawn_blocking(move || {
                    spotify_artwork_url(&expected_id, &title, &artist, &album)
                })
                .await
                .map_err(|error| format!("Spotify 封面读取任务失败：{error}"))??;
                let Some(url) = url else { return Ok(None) };
                download_artwork(http, &url).await?
            }
            PlayerKind::AppleMusic => {
                let raw_path = self.temp_path("apple-music");
                let expected_id = track_id.to_string();
                let export_path = raw_path.clone();
                let exported = tauri::async_runtime::spawn_blocking(move || {
                    export_apple_music_artwork(&expected_id, &title, &artist, &album, &export_path)
                })
                .await
                .map_err(|error| format!("Apple Music 封面读取任务失败：{error}"))??;
                if !exported {
                    let _ = fs::remove_file(&raw_path);
                    return Ok(None);
                }
                let bytes = fs::read(&raw_path)
                    .map_err(|error| format!("读取 Apple Music 封面失败：{error}"));
                let _ = fs::remove_file(&raw_path);
                bytes?
            }
        };

        let output_path = final_path.clone();
        let temp_path = self.temp_path("normalized");
        let cache_dir = self.cache_dir.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let normalized = normalize_image(&source)?;
            write_atomically(&output_path, &temp_path, &normalized)?;
            prune_cache(&cache_dir, MAX_CACHE_FILES);
            Ok::<(), String>(())
        })
        .await
        .map_err(|error| format!("封面处理任务失败：{error}"))??;

        Ok(Some(asset(player, track_id, &final_path)))
    }

    fn cache_path(&self, player: PlayerKind, track_id: &str) -> PathBuf {
        self.cache_dir
            .join(format!("{}.jpg", cache_key(player, track_id)))
    }

    fn temp_path(&self, kind: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        self.cache_dir
            .join(format!(".{kind}-{}-{nonce}.tmp", std::process::id()))
    }
}

fn player_name(player: PlayerKind) -> &'static str {
    match player {
        PlayerKind::AppleMusic => "apple_music",
        PlayerKind::Spotify => "spotify",
    }
}

fn cache_key(player: PlayerKind, track_id: &str) -> String {
    let digest = Sha256::digest(format!("{}:{track_id}", player_name(player)).as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn asset(player: PlayerKind, track_id: &str, path: &Path) -> ArtworkAsset {
    ArtworkAsset {
        player,
        track_id: track_id.to_string(),
        file_path: path.to_string_lossy().into_owned(),
    }
}

fn spotify_artwork_url(
    expected_id: &str,
    title: &str,
    artist: &str,
    album: &str,
) -> Result<Option<String>, String> {
    let app_path = spotify_app_path();
    if !app_path.exists() {
        return Ok(None);
    }
    let mut command = Command::new("/usr/bin/osascript");
    command
        .args(["-l", "JavaScript", "-e", SPOTIFY_ARTWORK_SCRIPT])
        .env("LYRICS_PLUS_APP_PATH", app_path)
        .env("LYRICS_PLUS_TRACK_ID", expected_id)
        .env("LYRICS_PLUS_TRACK_TITLE", title)
        .env("LYRICS_PLUS_TRACK_ARTIST", artist)
        .env("LYRICS_PLUS_TRACK_ALBUM", album);
    let output = run_with_timeout(command, Duration::from_secs(3))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let response: SpotifyArtworkResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("无法解析 Spotify 封面响应：{error}"))?;
    if response.status != "ok" {
        return Ok(None);
    }
    Ok(response.url.filter(|url| !url.trim().is_empty()))
}

fn spotify_app_path() -> PathBuf {
    let system = PathBuf::from("/Applications/Spotify.app");
    if system.exists() {
        return system;
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Applications/Spotify.app"))
        .unwrap_or(system)
}

fn export_apple_music_artwork(
    expected_id: &str,
    title: &str,
    artist: &str,
    album: &str,
    output_path: &Path,
) -> Result<bool, String> {
    log::debug!("开始从 Apple Music 导出歌曲封面");
    let mut command = Command::new("/usr/bin/osascript");
    command
        .args(["-e", APPLE_MUSIC_ARTWORK_SCRIPT])
        .arg("--")
        .arg(expected_id)
        .arg(title)
        .arg(artist)
        .arg(album)
        .arg(output_path);
    let output = run_with_timeout(command, Duration::from_secs(4))?;
    if !output.status.success() {
        log::debug!("Apple Music 封面脚本执行失败");
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if let Some(error) = status.strip_prefix("error:") {
        log::debug!("Apple Music 封面脚本返回错误状态");
        return Err(error.trim().to_string());
    }
    log::debug!("Apple Music 封面脚本执行完成：{status}");
    Ok(status == "ok" && is_nonempty_file(output_path))
}

async fn download_artwork(http: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let parsed = reqwest::Url::parse(url).map_err(|error| format!("封面地址无效：{error}"))?;
    if parsed.scheme() != "https" {
        return Err("封面地址不是 HTTPS".into());
    }
    let response = http
        .get(parsed)
        .send()
        .await
        .map_err(|error| format!("下载封面失败：{error}"))?
        .error_for_status()
        .map_err(|error| format!("下载封面失败：{error}"))?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SOURCE_BYTES as u64)
    {
        return Err("封面文件超过 10MB".into());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取封面响应失败：{error}"))?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err("封面文件超过 10MB".into());
    }
    Ok(bytes.to_vec())
}

fn normalize_image(source: &[u8]) -> Result<Vec<u8>, String> {
    if source.is_empty() {
        return Err("封面内容为空".into());
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err("封面文件超过 10MB".into());
    }
    let image = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|error| format!("无法识别封面格式：{error}"))?
        .decode()
        .map_err(|error| format!("无法解码封面：{error}"))?
        .thumbnail(MAX_ARTWORK_DIMENSION, MAX_ARTWORK_DIMENSION)
        .to_rgb8();
    let mut output = Vec::new();
    JpegEncoder::new_with_quality(&mut output, JPEG_QUALITY)
        .encode_image(&image)
        .map_err(|error| format!("无法编码封面：{error}"))?;
    Ok(output)
}

fn write_atomically(final_path: &Path, temp_path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(temp_path, bytes).map_err(|error| format!("写入封面缓存失败：{error}"))?;
    if let Err(error) = fs::rename(temp_path, final_path) {
        let _ = fs::remove_file(temp_path);
        return Err(format!("提交封面缓存失败：{error}"));
    }
    Ok(())
}

fn is_nonempty_file(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

fn prune_cache(cache_dir: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(cache_dir) else {
        return;
    };
    let mut files = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("jpg") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((path, modified))
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| right.0.cmp(&left.0)));
    for (path, _) in files.into_iter().skip(keep) {
        if let Err(error) = fs::remove_file(&path) {
            log::warn!("清理封面缓存失败 {}：{error}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GenericImageView, ImageFormat, RgbImage};
    use tempfile::tempdir;

    fn sample_png(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(
            width,
            height,
            image::Rgb([80, 120, 180]),
        ));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn cache_keys_are_stable_and_player_specific() {
        assert_eq!(
            cache_key(PlayerKind::Spotify, "track"),
            cache_key(PlayerKind::Spotify, "track")
        );
        assert_ne!(
            cache_key(PlayerKind::Spotify, "track"),
            cache_key(PlayerKind::AppleMusic, "track")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn apple_music_artwork_script_compiles() {
        let root = tempdir().unwrap();
        let output_path = root.path().join("apple-music-artwork.scpt");
        let output = Command::new("/usr/bin/osacompile")
            .arg("-o")
            .arg(&output_path)
            .arg("-e")
            .arg(APPLE_MUSIC_ARTWORK_SCRIPT)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Apple Music 封面脚本编译失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        assert!(output_path.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn osascript_preserves_unicode_arguments() {
        let output = Command::new("/usr/bin/osascript")
            .args([
                "-e",
                "on run argv\nreturn (item 1 of argv) & \"|\" & (item 2 of argv)\nend run",
                "--",
                "催眠",
                "王菲",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "AppleScript Unicode 参数读取失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "催眠|王菲");
    }

    #[test]
    fn normalizes_and_limits_image_dimensions() {
        let normalized = normalize_image(&sample_png(900, 600)).unwrap();
        let decoded = image::load_from_memory(&normalized).unwrap();
        assert_eq!(decoded.dimensions(), (384, 256));
    }

    #[test]
    fn rejects_oversized_and_invalid_images() {
        assert!(normalize_image(&vec![0; MAX_SOURCE_BYTES + 1]).is_err());
        assert!(normalize_image(b"not an image").is_err());
    }

    #[test]
    fn detects_existing_cache_entries() {
        let root = tempdir().unwrap();
        let service = ArtworkService::new(root.path().to_path_buf()).unwrap();
        let path = service.cache_path(PlayerKind::Spotify, "cached-track");
        fs::write(&path, b"cached").unwrap();
        assert!(is_nonempty_file(&path));
    }

    #[tokio::test]
    async fn returns_cached_artwork_without_querying_the_player() {
        let root = tempdir().unwrap();
        let service = ArtworkService::new(root.path().to_path_buf()).unwrap();
        let path = service.cache_path(PlayerKind::Spotify, "cached-track");
        fs::write(&path, b"cached").unwrap();
        let result = service
            .resolve(
                &PlaybackSnapshot {
                    player: Some(PlayerKind::Spotify),
                    track_id: Some("cached-track".into()),
                    ..PlaybackSnapshot::default()
                },
                &reqwest::Client::new(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.file_path, path.to_string_lossy());
    }

    #[test]
    fn prunes_cache_to_requested_limit() {
        let root = tempdir().unwrap();
        for index in 0..3 {
            fs::write(root.path().join(format!("{index}.jpg")), [index]).unwrap();
        }
        std::thread::sleep(Duration::from_millis(10));
        for index in 3..5 {
            fs::write(root.path().join(format!("{index}.jpg")), [index]).unwrap();
        }
        prune_cache(root.path(), 2);
        let remaining = fs::read_dir(root.path())
            .unwrap()
            .flatten()
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("jpg")
            })
            .count();
        assert_eq!(remaining, 2);
        assert!(root.path().join("3.jpg").exists());
        assert!(root.path().join("4.jpg").exists());
    }
}
