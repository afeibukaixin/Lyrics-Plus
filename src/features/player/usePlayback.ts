import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauriRuntime, messageOf } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import { playerService } from "./playerService";
import type {
  PlaybackAction,
  PlaybackArtwork,
  PlaybackSnapshot,
  PlayerSelection,
} from "../../shared/types";

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

export function usePlayback() {
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [selection, setSelectionState] = useState<PlayerSelection>("auto");
  const [clock, setClock] = useState(Date.now());
  const [configError, setConfigError] = useState<string | null>(null);
  const [snapshotLoadError, setSnapshotLoadError] = useState<string | null>(null);
  const [artwork, setArtwork] = useState<PlaybackArtwork | null>(null);
  const [artworkLoading, setArtworkLoading] = useState(false);
  const [artworkError, setArtworkError] = useState<string | null>(null);
  const [isControlling, setIsControlling] = useState(false);
  const [controlError, setControlError] = useState<string | null>(null);
  const artworkRequestVersionRef = useRef(0);
  const controlPromiseRef = useRef<Promise<void> | null>(null);

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
    const artworkId = snapshot.artworkId;
    artworkRequestVersionRef.current += 1;
    const requestVersion = artworkRequestVersionRef.current;
    if (!artworkId || !isTauriRuntime()) {
      setArtwork(null);
      setArtworkLoading(false);
      setArtworkError(null);
      return;
    }

    setArtworkLoading(true);
    setArtworkError(null);
    playerService.getArtwork(artworkId).then((value) => {
      if (artworkRequestVersionRef.current !== requestVersion) return;
      setArtwork(value?.id === artworkId ? value : null);
    }).catch((error) => {
      if (artworkRequestVersionRef.current !== requestVersion) return;
      setArtwork(null);
      setArtworkError(messageOf(error));
    }).finally(() => {
      if (artworkRequestVersionRef.current === requestVersion) {
        setArtworkLoading(false);
      }
    });
    return () => {
      if (artworkRequestVersionRef.current === requestVersion) {
        artworkRequestVersionRef.current += 1;
      }
    };
  }, [snapshot.artworkId]);

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

  const runControl = useCallback((action: PlaybackAction) => {
    if (controlPromiseRef.current) return controlPromiseRef.current;
    setControlError(null);
    setIsControlling(true);
    const operation = Promise.resolve()
      .then(() => playerService.control(action))
      .catch((error) => {
        setControlError(messageOf(error));
        throw error;
      })
      .finally(() => {
        controlPromiseRef.current = null;
        setIsControlling(false);
      });
    controlPromiseRef.current = operation;
    return operation;
  }, []);

  const play = useCallback(() => runControl("play"), [runControl]);
  const pause = useCallback(() => runControl("pause"), [runControl]);
  const togglePlayPause = useCallback(
    () => runControl("toggle_play_pause"),
    [runControl],
  );
  const previousTrack = useCallback(() => runControl("previous"), [runControl]);
  const nextTrack = useCallback(() => runControl("next"), [runControl]);
  const clearControlError = useCallback(() => setControlError(null), []);
  const artworkUrl = useMemo(() => {
    if (!artwork || artwork.id !== snapshot.artworkId) return null;
    return `data:${artwork.mimeType};base64,${artwork.dataBase64}`;
  }, [artwork, snapshot.artworkId]);

  return {
    snapshot,
    positionMs,
    selection,
    setSelection,
    syncSelection: setSelectionState,
    configError,
    snapshotLoadError,
    refreshSnapshot,
    play,
    pause,
    togglePlayPause,
    previousTrack,
    nextTrack,
    isControlling,
    controlError,
    clearControlError,
    artwork,
    artworkUrl,
    artworkLoading,
    artworkError,
  };
}
