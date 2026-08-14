import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauriRuntime, messageOf } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import type { PlaybackSnapshot, PlayerSelection } from "../../shared/types";

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
  durationMs: null,
  positionMs: null,
  observedAtMs: Date.now(),
  errorCode: "waiting",
  error: null,
};

export function usePlayback() {
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [selection, setSelectionState] = useState<PlayerSelection>("auto");
  const [clock, setClock] = useState(Date.now());
  const [configError, setConfigError] = useState<string | null>(null);
  const [snapshotLoadError, setSnapshotLoadError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    api.getPlayerSelection().then((value) => { setSelectionState(value); setConfigError(null); }).catch((error) => setConfigError(messageOf(error)));
    api.getPlayback().then((value) => { setSnapshot(value); setSnapshotLoadError(null); }).catch((error) => setSnapshotLoadError(messageOf(error)));
    const cleanupSnapshotListener = createTauriListenerCleanup(
      listen<PlaybackSnapshot>("playback://snapshot", ({ payload }) => { setSnapshot(payload); setSnapshotLoadError(null); }),
    );
    const cleanupSelectionListener = createTauriListenerCleanup(
      listen<PlayerSelection>("player://selection", ({ payload }) => setSelectionState(payload)),
    );
    return () => {
      cleanupSnapshotListener();
      cleanupSelectionListener();
    };
  }, []);

  useEffect(() => {
    let frame = 0;
    let previous = 0;
    const tick = (time: number) => {
      if (time - previous >= 100) {
        previous = time;
        setClock(Date.now());
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, []);

  const positionMs = useMemo(() => {
    const base = snapshot.positionMs ?? 0;
    if (!snapshot.isPlaying) return base;
    return Math.min(snapshot.durationMs ?? Number.MAX_SAFE_INTEGER, base + Math.max(0, clock - snapshot.observedAtMs));
  }, [clock, snapshot]);

  const setSelection = async (next: PlayerSelection) => {
    const previous = selection;
    setSelectionState(next);
    setConfigError(null);
    try {
      await api.setPlayerSelection(next);
    } catch (error) {
      setSelectionState(previous);
      setConfigError(messageOf(error));
      throw error;
    }
  };

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
    positionMs,
    selection,
    setSelection,
    syncSelection: setSelectionState,
    configError,
    snapshotLoadError,
    refreshSnapshot,
  };
}
