import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { api, isTauriRuntime, messageOf } from "../../../shared/api";
import { createTauriListenerCleanup } from "../../../shared/tauriEvent";
import type { PlaybackSnapshot, PlayerSelection } from "../../../shared/types";

const initialSnapshot: PlaybackSnapshot = {
  player: null,
  isRunning: false,
  isPlaying: false,
  trackId: null,
  title: null,
  artist: null,
  album: null,
  sourceAppName: null,
  sourceAppBundleId: null,
  artworkId: null,
  durationMs: null,
  positionMs: null,
  observedAtMs: Date.now(),
  errorCode: "waiting",
  error: null,
};

export function usePlaybackEvents(active: boolean) {
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [selection, setSelectionState] = useState<PlayerSelection>("auto");
  const [configError, setConfigError] = useState<string | null>(null);
  const [snapshotLoadError, setSnapshotLoadError] = useState<string | null>(null);

  useEffect(() => {
    if (!active || !isTauriRuntime()) {
      setSnapshot(initialSnapshot);
      return;
    }
    let disposed = false;
    api.getPlayerSelection().then((value) => {
      if (disposed) return;
      setSelectionState(value);
      setConfigError(null);
    }).catch((error) => {
      if (!disposed) setConfigError(messageOf(error));
    });
    api.getPlayback().then((value) => {
      if (disposed) return;
      setSnapshot(value);
      setSnapshotLoadError(null);
    }).catch((error) => {
      if (!disposed) setSnapshotLoadError(messageOf(error));
    });
    const cleanupSnapshotListener = createTauriListenerCleanup(
      listen<PlaybackSnapshot>("playback://snapshot", ({ payload }) => {
        if (disposed) return;
        setSnapshot(payload);
        setSnapshotLoadError(null);
      }),
    );
    const cleanupSelectionListener = createTauriListenerCleanup(
      listen<PlayerSelection>("player://selection", ({ payload }) => {
        if (!disposed) setSelectionState(payload);
      }),
    );
    return () => {
      disposed = true;
      cleanupSnapshotListener();
      cleanupSelectionListener();
    };
  }, [active]);

  const refreshSnapshot = async () => {
    setSnapshotLoadError(null);
    try {
      setSnapshot(await api.getPlayback());
    } catch (error) {
      setSnapshotLoadError(messageOf(error));
    }
  };

  return {
    snapshot,
    selection,
    syncSelection: setSelectionState,
    setConfigError,
    configError,
    snapshotLoadError,
    refreshSnapshot,
  };
}
