use std::path::Path;

use objc2_core_services::AEKeyword;
use objc2_foundation::{NSAppleEventDescriptor, NSData};

use super::super::{PlaybackSnapshot, PlayerKind};

const BUNDLE_ID: &str = "com.apple.Music";
const TRACK_ID: AEKeyword = u32::from_be_bytes(*b"pPIS");
const NAME: AEKeyword = u32::from_be_bytes(*b"pnam");
const ARTIST: AEKeyword = u32::from_be_bytes(*b"pArt");
const ALBUM: AEKeyword = u32::from_be_bytes(*b"pAlb");
const ARTWORKS: AEKeyword = u32::from_be_bytes(*b"cArt");
const RAW_DATA: AEKeyword = u32::from_be_bytes(*b"pRaw");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtworkExport {
    Exported,
    Missing,
    Stale,
    Unavailable,
}

pub(super) fn snapshot() -> PlaybackSnapshot {
    super::query(PlayerKind::AppleMusic, BUNDLE_ID, 1000, TRACK_ID)
}

pub(super) fn perform_action(action: &str, position_ms: Option<u64>) -> Result<(), String> {
    super::perform_action_for_app(BUNDLE_ID, action, position_ms)
}

pub(crate) fn export_artwork(
    expected_id: &str,
    title: &str,
    artist: &str,
    album: &str,
    output_path: &Path,
) -> Result<ArtworkExport, String> {
    let result = super::with_application(BUNDLE_ID, super::ARTWORK_TIMEOUT_TICKS, |session| {
        let Some(track) = session.current_track()? else {
            return Ok((ArtworkExport::Unavailable, None));
        };
        let matches = if expected_id.starts_with("fallback:") {
            session.string(&track, NAME)?.as_deref() == Some(title)
                && session.string(&track, ARTIST)?.as_deref() == Some(artist)
                && session.string(&track, ALBUM)?.as_deref() == Some(album)
        } else {
            session.string(&track, TRACK_ID)?.as_deref() == Some(expected_id)
        };
        if !matches {
            return Ok((ArtworkExport::Stale, None));
        }
        let Some(artwork) = session.first_element(&track, ARTWORKS)? else {
            return Ok((ArtworkExport::Missing, None));
        };
        let bytes = session
            .value(&artwork, RAW_DATA)?
            .and_then(|value| {
                if let Some(data) = value.downcast_ref::<NSData>() {
                    return Some(data.to_vec());
                }
                value
                    .downcast_ref::<NSAppleEventDescriptor>()
                    .map(|descriptor| descriptor.data().to_vec())
            })
            .filter(|bytes| !bytes.is_empty());
        Ok((
            if bytes.is_some() {
                ArtworkExport::Exported
            } else {
                ArtworkExport::Missing
            },
            bytes,
        ))
    });
    let (result, bytes) = match result {
        Ok(result) => result,
        Err(error) if error.playback_code() == super::PlaybackErrorCode::Unavailable => {
            (ArtworkExport::Unavailable, None)
        }
        Err(error) => return Err(error.user_message()),
    };
    if let Some(bytes) = bytes {
        std::fs::write(output_path, bytes)
            .map_err(|error| format!("Failed to export Apple Music artwork: {error}"))?;
    }
    let status = match result {
        ArtworkExport::Exported => "ok",
        ArtworkExport::Missing => "missing",
        ArtworkExport::Stale => "stale",
        ArtworkExport::Unavailable => "unavailable",
    };
    log::debug!("Track artwork source lookup completed: player=apple_music status={status}");
    Ok(result)
}
