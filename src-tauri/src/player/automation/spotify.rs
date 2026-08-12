use objc2_core_services::AEKeyword;

use super::super::{PlaybackSnapshot, PlayerKind};

const BUNDLE_ID: &str = "com.spotify.client";
const TRACK_ID: AEKeyword = u32::from_be_bytes(*b"ID  ");
const NAME: AEKeyword = u32::from_be_bytes(*b"pnam");
const ARTIST: AEKeyword = u32::from_be_bytes(*b"pArt");
const ALBUM: AEKeyword = u32::from_be_bytes(*b"pAlb");
const ARTWORK_URL: AEKeyword = u32::from_be_bytes(*b"aUrl");

pub(super) fn snapshot() -> PlaybackSnapshot {
    super::query(PlayerKind::Spotify, BUNDLE_ID, 1, TRACK_ID)
}

pub(super) fn perform_action(action: &str, position_ms: Option<u64>) -> Result<(), String> {
    super::perform_action_for_app(BUNDLE_ID, action, position_ms)
}

pub(crate) fn artwork_url(
    expected_id: &str,
    title: &str,
    artist: &str,
    album: &str,
) -> Result<Option<String>, String> {
    let result = super::with_application(BUNDLE_ID, super::ARTWORK_TIMEOUT_TICKS, |session| {
        let Some(track) = session.current_track()? else {
            return Ok(("unavailable", None));
        };
        let matches = if expected_id.starts_with("fallback:") {
            session.string(&track, NAME)?.as_deref() == Some(title)
                && session.string(&track, ARTIST)?.as_deref() == Some(artist)
                && session.string(&track, ALBUM)?.as_deref() == Some(album)
        } else {
            session.string(&track, TRACK_ID)?.as_deref() == Some(expected_id)
        };
        if !matches {
            return Ok(("stale", None));
        }
        let url = session
            .string(&track, ARTWORK_URL)?
            .filter(|url| !url.trim().is_empty());
        Ok((if url.is_some() { "ok" } else { "missing" }, url))
    });
    let (status, url) = match result {
        Ok(result) => result,
        Err(error) if error.playback_code() == super::PlaybackErrorCode::Unavailable => {
            ("unavailable", None)
        }
        Err(error) => return Err(error.user_message()),
    };
    log::debug!("Track artwork source lookup completed: player=spotify status={status}");
    Ok(url)
}
