use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{Emitter, Manager};

use crate::lyrics::provider::LyricsSearchInput;
use crate::lyrics::LyricsDocument;
use crate::player::{PlaybackSnapshot, PlayerKind};
use crate::state::AppState;

use super::model::{
    LyricsRuntimeSnapshot, LyricsRuntimeStatus, LyricsSearchIntent, LYRICS_SEARCH_INVALIDATED,
};
use super::publication::publish_lyrics_runtime;
use super::search::{
    reset_lyrics_search_session, save_automatic_search_result, search_lyrics_for_session,
};

fn player_key(player: PlayerKind) -> &'static str {
    match player {
        PlayerKind::AppleMusic => "apple_music",
        PlayerKind::Spotify => "spotify",
        PlayerKind::System => "system",
    }
}

pub(crate) fn playback_track_key(snapshot: &PlaybackSnapshot) -> Option<String> {
    let player = snapshot.player?;
    let title = snapshot.title.as_deref()?.trim();
    let artist = snapshot.artist.as_deref()?.trim();
    if title.is_empty() || artist.is_empty() {
        return None;
    }
    if let Some(track_id) = snapshot
        .track_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        return Some(format!("{}:{}", player_key(player), track_id));
    }
    let fallback = format!("{title}|{artist}|{}", snapshot.duration_ms.unwrap_or(0))
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    Some(format!("{}:fallback:{fallback}", player_key(player)))
}

pub(crate) fn set_runtime_document_if_active(
    app: &tauri::AppHandle,
    track_key: &str,
    document: Option<LyricsDocument>,
) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let active = state
        .lyrics_runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .track_key
        .as_deref()
        == Some(track_key);
    if !active {
        return;
    }
    state.lyrics_generation.fetch_add(1, Ordering::SeqCst);
    publish_lyrics_runtime(
        app,
        LyricsRuntimeSnapshot {
            track_key: Some(track_key.to_owned()),
            status: if document.is_some() {
                LyricsRuntimeStatus::Ready
            } else {
                LyricsRuntimeStatus::NotFound
            },
            document,
            error: None,
        },
    );
}

pub(crate) fn reload_active_lyrics_runtime(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let playback = state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    sync_lyrics_runtime_inner(app, &playback, true);
}

pub(crate) fn sync_lyrics_runtime(app: &tauri::AppHandle, playback: &PlaybackSnapshot) {
    sync_lyrics_runtime_inner(app, playback, false);
}

fn sync_lyrics_runtime_inner(app: &tauri::AppHandle, playback: &PlaybackSnapshot, force: bool) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let next_key = playback_track_key(playback);
    let current_key = state
        .lyrics_runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .track_key
        .clone();
    if force || current_key != next_key {
        if let (Some(track_key), Some(title), Some(artist)) = (
            next_key.as_deref(),
            playback.title.as_deref(),
            playback.artist.as_deref(),
        ) {
            if let Err(error) = state.storage.ensure_track_alias(
                track_key,
                title,
                artist,
                playback.album.as_deref(),
                playback.duration_ms,
            ) {
                log::warn!("整理当前歌曲歌词关联失败：{error}");
            }
        }
    }
    if !force && current_key == next_key {
        crate::sync_lyrics_surfaces(app);
        return;
    }

    let generation = state.lyrics_generation.fetch_add(1, Ordering::SeqCst) + 1;
    reset_lyrics_search_session(&state, next_key.clone());
    let Some(track_key) = next_key else {
        publish_lyrics_runtime(app, LyricsRuntimeSnapshot::default());
        return;
    };
    publish_lyrics_runtime(
        app,
        LyricsRuntimeSnapshot {
            track_key: Some(track_key.clone()),
            document: None,
            status: LyricsRuntimeStatus::Loading,
            error: None,
        },
    );

    let playback = playback.clone();
    let worker_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = worker_app.state::<AppState>();
        let current = || state.lyrics_generation.load(Ordering::SeqCst) == generation;
        match state.storage.load_with_status(&track_key) {
            Ok(crate::storage::LyricsLoadResult::Ready(document)) => {
                if !current() {
                    return;
                }
                publish_lyrics_runtime(
                    &worker_app,
                    LyricsRuntimeSnapshot {
                        track_key: Some(track_key.clone()),
                        document: Some(document),
                        status: LyricsRuntimeStatus::Ready,
                        error: None,
                    },
                );
                return;
            }
            Ok(crate::storage::LyricsLoadResult::Invalid(error)) => {
                log::warn!("当前歌曲歌词关联内容无效，准备解除关联：{error}");
                if let Err(invalidation_error) = state.storage.remove(&track_key) {
                    if current() {
                        publish_lyrics_runtime(
                            &worker_app,
                            LyricsRuntimeSnapshot {
                                track_key: Some(track_key),
                                document: None,
                                status: LyricsRuntimeStatus::Error,
                                error: Some(format!("解除无效歌词关联失败：{invalidation_error}")),
                            },
                        );
                    }
                    return;
                }
            }
            Err(error) => {
                if current() {
                    publish_lyrics_runtime(
                        &worker_app,
                        LyricsRuntimeSnapshot {
                            track_key: Some(track_key),
                            document: None,
                            status: LyricsRuntimeStatus::Error,
                            error: Some(error),
                        },
                    );
                }
                return;
            }
            Ok(crate::storage::LyricsLoadResult::Missing) => {}
        }

        let (Some(title), Some(artist)) = (playback.title.clone(), playback.artist.clone()) else {
            if current() {
                publish_lyrics_runtime(
                    &worker_app,
                    LyricsRuntimeSnapshot {
                        track_key: Some(track_key),
                        document: None,
                        status: LyricsRuntimeStatus::NotFound,
                        error: None,
                    },
                );
            }
            return;
        };

        let input = LyricsSearchInput {
            title: title.clone(),
            artist: artist.clone(),
            album: playback.album.clone(),
            duration_ms: playback.duration_ms,
            scoring: Arc::default(),
        };
        match search_lyrics_for_session(&state, &track_key, input, LyricsSearchIntent::Automatic)
            .await
        {
            Ok(response) => {
                if !current() {
                    return;
                }
                if let Some(error) = response.error {
                    publish_lyrics_runtime(
                        &worker_app,
                        LyricsRuntimeSnapshot {
                            track_key: Some(track_key),
                            document: None,
                            status: LyricsRuntimeStatus::Error,
                            error: Some(error),
                        },
                    );
                    return;
                }
                let document = if response.auto_apply {
                    response.results.first().and_then(|result| {
                        save_automatic_search_result(&state, &track_key, &title, &artist, result)
                            .ok()
                    })
                } else {
                    None
                };
                if document.is_some() {
                    let _ = worker_app.emit("lyrics://changed", &track_key);
                }
                if current() {
                    publish_lyrics_runtime(
                        &worker_app,
                        LyricsRuntimeSnapshot {
                            track_key: Some(track_key),
                            status: if document.is_some() {
                                LyricsRuntimeStatus::Ready
                            } else {
                                LyricsRuntimeStatus::NotFound
                            },
                            document,
                            error: None,
                        },
                    );
                }
            }
            Err(error) if current() && error == LYRICS_SEARCH_INVALIDATED => {
                publish_lyrics_runtime(
                    &worker_app,
                    LyricsRuntimeSnapshot {
                        track_key: Some(track_key),
                        document: None,
                        status: LyricsRuntimeStatus::NotFound,
                        error: None,
                    },
                )
            }
            Err(error) if current() => publish_lyrics_runtime(
                &worker_app,
                LyricsRuntimeSnapshot {
                    track_key: Some(track_key),
                    document: None,
                    status: LyricsRuntimeStatus::Error,
                    error: Some(error),
                },
            ),
            Err(_) => {}
        }
    });
}
