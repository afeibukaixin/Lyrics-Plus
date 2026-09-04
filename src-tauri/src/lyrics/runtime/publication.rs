use tauri::{Emitter, Manager};

use crate::state::AppState;

use super::model::LyricsRuntimeSnapshot;

pub(super) fn publish_lyrics_runtime(app: &tauri::AppHandle, snapshot: LyricsRuntimeSnapshot) {
    if let Some(state) = app.try_state::<AppState>() {
        *state
            .lyrics_runtime
            .write()
            .unwrap_or_else(|error| error.into_inner()) = snapshot.clone();

        let config = state.config.snapshot();
        let mut presented = snapshot;
        presented.document = presented.document.map(|document| {
            document.converted_for_output(
                config.lyrics.chinese_conversion,
                config.lyrics.repair_simplified_japanese,
            )
        });
        let _ = app.emit("lyrics://runtime-changed", &presented);
    } else {
        let _ = app.emit("lyrics://runtime-changed", &snapshot);
    }
    crate::sync_lyrics_surfaces(app);
}

pub(crate) fn republish_lyrics_runtime(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let snapshot = state
        .lyrics_runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    publish_lyrics_runtime(app, snapshot);
}
