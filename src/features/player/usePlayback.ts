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
  durationMs: null,
  positionMs: null,
  canSeek: false,
  observedAtMs: Date.now(),
  errorCode: "waiting",
  error: null,
};

export function usePlayback() {
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [selection, setSelectionState] = useState<PlayerSelection>("auto");
  const [clock, setClock] = useState(Date.now());
  const [commandError, setCommandError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    api.getPlayerSelection().then(setSelectionState).catch((error) => setCommandError(messageOf(error)));
    api.getPlayback().then(setSnapshot).catch((error) => setCommandError(messageOf(error)));
    const cleanupSnapshotListener = createTauriListenerCleanup(
      listen<PlaybackSnapshot>("playback://snapshot", ({ payload }) => setSnapshot(payload)),
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

  const setSelection = (next: PlayerSelection) => {
    setSelectionState(next);
    api.setPlayerSelection(next).catch((error) => setCommandError(messageOf(error)));
  };

  const action = async (name: "play_pause" | "next" | "previous" | "seek", value?: number) => {
    setCommandError(null);
    try {
      await api.playerAction(name, value);
    } catch (error) {
      setCommandError(messageOf(error));
    }
  };

  return {
    snapshot,
    positionMs,
    selection,
    setSelection,
    syncSelection: setSelectionState,
    action,
    commandError,
  };
}
