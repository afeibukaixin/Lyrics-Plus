use std::process::{Command, Output, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::config::{RegisteredApplication, SystemMediaFilterMode};

use super::routing::system_source_allowed;
use super::{
    automation, PlaybackAction, PlaybackSnapshot, PlayerKind, PlayerSelection, SystemMediaService,
};

const PROCESS_TIMEOUT_ERROR: &str = "Process timed out";
const SYSTEM_MEDIA_NOT_CURRENT: &str = "所选歌曲不是系统当前媒体，无法控制";

pub(crate) fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<Output, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    match child
        .wait_timeout(timeout)
        .map_err(|error| error.to_string())?
    {
        Some(_) => child.wait_with_output().map_err(|error| error.to_string()),
        None => {
            let _ = child.kill();
            let _ = child.wait();
            Err(PROCESS_TIMEOUT_ERROR.into())
        }
    }
}

pub(crate) fn control_playback(
    action: PlaybackAction,
    selection: PlayerSelection,
    snapshot: &PlaybackSnapshot,
    system_media: &SystemMediaService,
    filter_mode: SystemMediaFilterMode,
    applications: &[RegisteredApplication],
) -> Result<(), String> {
    let player = selection.preferred_kind().or(snapshot.player);
    let Some(player) = player else {
        return Err("当前没有可控制的播放器".into());
    };
    if (selection == PlayerSelection::Auto || player == PlayerKind::System)
        && (!snapshot.is_running || snapshot.error_code.is_some())
    {
        return Err(snapshot
            .error
            .clone()
            .unwrap_or_else(|| "当前播放器不可用".into()));
    }

    match player {
        PlayerKind::AppleMusic | PlayerKind::Spotify => automation::control(player, action),
        PlayerKind::System => {
            if snapshot.player != Some(PlayerKind::System)
                || !system_source_allowed(snapshot, filter_mode, applications)
            {
                return Err(SYSTEM_MEDIA_NOT_CURRENT.into());
            }
            system_media.control_current(snapshot, action)
        }
    }
}

pub(crate) fn seek_playback(
    position_ms: u64,
    selection: PlayerSelection,
    snapshot: &PlaybackSnapshot,
    system_media: &SystemMediaService,
    filter_mode: SystemMediaFilterMode,
    applications: &[RegisteredApplication],
) -> Result<(), String> {
    let player = selection.preferred_kind().or(snapshot.player);
    let Some(player) = player else {
        return Err("当前没有可控制的播放器".into());
    };
    if (selection == PlayerSelection::Auto || player == PlayerKind::System)
        && (!snapshot.is_running || snapshot.error_code.is_some())
    {
        return Err(snapshot
            .error
            .clone()
            .unwrap_or_else(|| "当前播放器不可用".into()));
    }
    let duration_ms = snapshot
        .duration_ms
        .filter(|duration| *duration > 0)
        .ok_or_else(|| "当前媒体没有可用的播放时长".to_string())?;
    let position_ms = position_ms.min(duration_ms);

    match player {
        PlayerKind::AppleMusic | PlayerKind::Spotify => automation::seek(player, position_ms),
        PlayerKind::System => {
            if snapshot.player != Some(PlayerKind::System)
                || !system_source_allowed(snapshot, filter_mode, applications)
            {
                return Err(SYSTEM_MEDIA_NOT_CURRENT.into());
            }
            system_media.seek_current(snapshot, position_ms)
        }
    }
}
