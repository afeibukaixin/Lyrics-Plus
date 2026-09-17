use crate::config::{is_dedicated_player_bundle_id, RegisteredApplication, SystemMediaFilterMode};

use super::{
    automation, normalized_track_component, PlaybackErrorCode, PlaybackSnapshot, PlayerKind,
    PlayerSelection, SystemMediaService,
};

fn attach_system_artwork(snapshot: &mut PlaybackSnapshot, system: &PlaybackSnapshot) {
    if snapshot.player == Some(PlayerKind::System)
        || system.player != Some(PlayerKind::System)
        || system.artwork_id.is_none()
    {
        return;
    }
    let expected_bundle_id = match snapshot.player {
        Some(PlayerKind::AppleMusic) => "com.apple.Music",
        Some(PlayerKind::Spotify) => "com.spotify.client",
        _ => return,
    };
    if system.source_app_bundle_id.as_deref() != Some(expected_bundle_id) {
        return;
    }
    let same_title = match (snapshot.title.as_deref(), system.title.as_deref()) {
        (Some(left), Some(right)) => {
            normalized_track_component(left) == normalized_track_component(right)
        }
        _ => false,
    };
    let same_artist = match (snapshot.artist.as_deref(), system.artist.as_deref()) {
        (Some(left), Some(right)) => {
            normalized_track_component(left) == normalized_track_component(right)
        }
        _ => false,
    };
    if same_title && same_artist {
        snapshot.artwork_id = system.artwork_id.clone();
    }
}

pub fn query_selected_player(
    selection: PlayerSelection,
    previous_auto_player: Option<PlayerKind>,
    previous_system_bundle_id: Option<&str>,
    system_media: &SystemMediaService,
    system_media_filter_mode: SystemMediaFilterMode,
    system_media_applications: &[RegisteredApplication],
) -> (PlaybackSnapshot, Option<PlayerKind>) {
    let current_system_snapshot = system_media.snapshot();
    let system_snapshot = if matches!(selection, PlayerSelection::System | PlayerSelection::Auto)
        && system_media_filter_mode == SystemMediaFilterMode::Allowlist
        && !(current_system_snapshot.is_playing
            && current_system_snapshot
                .source_app_bundle_id
                .as_deref()
                .is_some_and(is_dedicated_player_bundle_id))
    {
        choose_allowlisted_source(
            current_system_snapshot.clone(),
            previous_system_bundle_id,
            system_media,
            system_media_applications,
        )
    } else {
        current_system_snapshot.clone()
    };
    let (mut snapshot, next_auto_player) = match selection {
        PlayerSelection::AppleMusic => (automation::snapshot(PlayerKind::AppleMusic), None),
        PlayerSelection::Spotify => (automation::snapshot(PlayerKind::Spotify), None),
        PlayerSelection::System => (
            filter_system_source(
                system_snapshot.clone(),
                system_media_filter_mode,
                system_media_applications,
            ),
            None,
        ),
        PlayerSelection::Auto => query_auto_player(
            system_snapshot.clone(),
            previous_auto_player,
            system_media_filter_mode,
            system_media_applications,
            automation::snapshot,
        ),
    };
    snapshot.system_control_available = snapshot.same_system_media(&current_system_snapshot);
    attach_system_artwork(&mut snapshot, &current_system_snapshot);
    (snapshot, next_auto_player)
}

fn choose_allowlisted_source(
    current: PlaybackSnapshot,
    previous_bundle_id: Option<&str>,
    system_media: &SystemMediaService,
    applications: &[RegisteredApplication],
) -> PlaybackSnapshot {
    let current_allowed = current.is_running
        && system_source_allowed(&current, SystemMediaFilterMode::Allowlist, applications);
    let (candidates, had_error) = match system_media.allowlisted_snapshots(applications) {
        Ok(result) => result,
        Err(error) => {
            return PlaybackSnapshot::unavailable_with_code(
                Some(PlayerKind::System),
                PlaybackErrorCode::Unavailable,
                error,
            );
        }
    };

    let selected = candidates
        .iter()
        .find(|(snapshot, _)| {
            current_allowed
                && current.is_playing
                && snapshot.is_playing
                && snapshot.source_app_bundle_id == current.source_app_bundle_id
        })
        .or_else(|| {
            candidates.iter().find(|(snapshot, _)| {
                snapshot.is_playing
                    && snapshot.source_app_bundle_id.as_deref() == previous_bundle_id
            })
        })
        .or_else(|| {
            most_recent(
                candidates
                    .iter()
                    .filter(|(snapshot, _)| snapshot.is_playing),
            )
        })
        .or_else(|| {
            candidates.iter().find(|(snapshot, _)| {
                snapshot.source_app_bundle_id.as_deref() == previous_bundle_id
            })
        })
        .or_else(|| {
            candidates.iter().find(|(snapshot, _)| {
                current_allowed && snapshot.source_app_bundle_id == current.source_app_bundle_id
            })
        })
        .or_else(|| {
            previous_bundle_id
                .is_none()
                .then(|| most_recent(candidates.iter()))
                .flatten()
        });
    if let Some((snapshot, _)) = selected {
        return snapshot
            .source_app_bundle_id
            .as_deref()
            .and_then(|bundle_id| system_media.targeted_snapshot(bundle_id))
            .unwrap_or_else(|| {
                PlaybackSnapshot::unavailable_with_code(
                    Some(PlayerKind::System),
                    PlaybackErrorCode::Unavailable,
                    "指定播放器状态已过期".into(),
                )
            });
    }
    if current_allowed
        && current
            .source_app_bundle_id
            .as_deref()
            .is_some_and(is_dedicated_player_bundle_id)
    {
        return current;
    }
    if had_error || current_allowed || previous_bundle_id.is_some() {
        return PlaybackSnapshot::unavailable_with_code(
            Some(PlayerKind::System),
            PlaybackErrorCode::Unavailable,
            "读取允许列表中的播放器失败".into(),
        );
    }
    current
}

fn most_recent<'a>(
    candidates: impl Iterator<Item = &'a (PlaybackSnapshot, Option<f64>)>,
) -> Option<&'a (PlaybackSnapshot, Option<f64>)> {
    candidates.reduce(|best, next| {
        if next.1.unwrap_or(0.0) > best.1.unwrap_or(0.0) {
            next
        } else {
            best
        }
    })
}

pub(super) fn query_auto_player(
    system: PlaybackSnapshot,
    previous_auto_player: Option<PlayerKind>,
    system_media_filter_mode: SystemMediaFilterMode,
    system_media_applications: &[RegisteredApplication],
    query: impl Fn(PlayerKind) -> PlaybackSnapshot,
) -> (PlaybackSnapshot, Option<PlayerKind>) {
    if system.is_playing {
        match system.source_app_bundle_id.as_deref() {
            Some("com.apple.Music") => {
                let music = query(PlayerKind::AppleMusic);
                return (music, Some(PlayerKind::AppleMusic));
            }
            Some("com.spotify.client") => {
                let spotify = query(PlayerKind::Spotify);
                return (spotify, Some(PlayerKind::Spotify));
            }
            _ => {
                let system = filter_system_source(
                    system.clone(),
                    system_media_filter_mode,
                    system_media_applications,
                );
                if system.error_code != Some(PlaybackErrorCode::SourceNotAllowed) {
                    return (system, Some(PlayerKind::System));
                }
            }
        }
    }
    let system = filter_system_source(system, system_media_filter_mode, system_media_applications);
    if previous_auto_player == Some(PlayerKind::System)
        && system.is_running
        && system.title.is_some()
        && system_source_allowed(&system, system_media_filter_mode, system_media_applications)
    {
        return (system, previous_auto_player);
    }
    let music = query(PlayerKind::AppleMusic);
    let spotify = query(PlayerKind::Spotify);
    if music.is_playing && spotify.is_playing {
        (
            PlaybackSnapshot::unavailable_with_code(
                None,
                PlaybackErrorCode::MultiplePlaying,
                "Apple Music 与 Spotify 同时在播放，请手动选择播放器".into(),
            ),
            None,
        )
    } else if music.is_playing {
        (music, Some(PlayerKind::AppleMusic))
    } else if spotify.is_playing {
        (spotify, Some(PlayerKind::Spotify))
    } else if previous_auto_player == Some(PlayerKind::AppleMusic)
        && music.is_running
        && music.title.is_some()
    {
        (music, previous_auto_player)
    } else if previous_auto_player == Some(PlayerKind::Spotify)
        && spotify.is_running
        && spotify.title.is_some()
    {
        (spotify, previous_auto_player)
    } else if matches!(
        system.error_code,
        Some(PlaybackErrorCode::SourceNotAllowed | PlaybackErrorCode::Unavailable)
    ) {
        (system, Some(PlayerKind::System))
    } else {
        (
            PlaybackSnapshot::unavailable_with_code(
                None,
                PlaybackErrorCode::NoUniquePlayer,
                "未检测到唯一正在播放的 Apple Music 或 Spotify".into(),
            ),
            None,
        )
    }
}

pub(super) fn system_source_allowed(
    snapshot: &PlaybackSnapshot,
    mode: SystemMediaFilterMode,
    applications: &[RegisteredApplication],
) -> bool {
    let Some(bundle_id) = snapshot.source_app_bundle_id.as_deref() else {
        return mode == SystemMediaFilterMode::Blocklist;
    };
    if is_dedicated_player_bundle_id(bundle_id) {
        return true;
    }
    let listed = applications
        .iter()
        .any(|application| application.bundle_id == bundle_id);
    listed == (mode == SystemMediaFilterMode::Allowlist)
}

pub(super) fn filter_system_source(
    snapshot: PlaybackSnapshot,
    mode: SystemMediaFilterMode,
    applications: &[RegisteredApplication],
) -> PlaybackSnapshot {
    if !snapshot.is_running || system_source_allowed(&snapshot, mode, applications) {
        snapshot
    } else {
        source_not_allowed(&snapshot, mode)
    }
}

fn source_not_allowed(
    snapshot: &PlaybackSnapshot,
    mode: SystemMediaFilterMode,
) -> PlaybackSnapshot {
    let mut unavailable = PlaybackSnapshot::unavailable_with_code(
        Some(PlayerKind::System),
        PlaybackErrorCode::SourceNotAllowed,
        match mode {
            SystemMediaFilterMode::Allowlist => "当前系统播放应用不在允许列表中",
            SystemMediaFilterMode::Blocklist => "当前系统播放应用在排除列表中",
        }
        .into(),
    );
    unavailable.source_app_name = snapshot.source_app_name.clone();
    unavailable.source_app_bundle_id = snapshot.source_app_bundle_id.clone();
    unavailable
}
