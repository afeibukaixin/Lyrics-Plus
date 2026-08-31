import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauriRuntime, messageOf } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import { playerService } from "./playerService";
import type {
  PlaybackAction,
  PlaybackSnapshot,
  PlayerSelection,
} from "../../shared/types";
import { useSurfaceActivity } from "../window/useSurfaceActivity";

const MISSING_ARTWORK_CONFIRMATION_MS = 2_500;

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

type UsePlaybackOptions = {
  loadArtwork?: boolean;
  trackPosition?: boolean;
};

function artworkBlobUrl(mimeType: string, dataBase64: string) {
  const decoded = atob(dataBase64);
  const bytes = new Uint8Array(decoded.length);
  for (let index = 0; index < decoded.length; index += 1) bytes[index] = decoded.charCodeAt(index);
  return URL.createObjectURL(new Blob([bytes], { type: mimeType }));
}

export function usePlayback({
  loadArtwork = false,
  trackPosition = true,
}: UsePlaybackOptions = {}) {
  const active = useSurfaceActivity();
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [selection, setSelectionState] = useState<PlayerSelection>("auto");
  const [clock, setClock] = useState(Date.now());
  const [configError, setConfigError] = useState<string | null>(null);
  const [snapshotLoadError, setSnapshotLoadError] = useState<string | null>(null);
  const [artworkUrl, setArtworkUrl] = useState<string | null>(null);
  const [artworkAccentColor, setArtworkAccentColor] = useState<string | null>(null);
  const [artworkLoading, setArtworkLoading] = useState(false);
  const [artworkError, setArtworkError] = useState<string | null>(null);
  const [isControlling, setIsControlling] = useState(false);
  const [controlError, setControlError] = useState<string | null>(null);
  const artworkRequestVersionRef = useRef(0);
  const loadedArtworkIdRef = useRef<string | null>(null);
  const loadedArtworkTrackKeyRef = useRef<string | null>(null);
  const artworkUrlRef = useRef<string | null>(null);
  const controlPromiseRef = useRef<Promise<void> | null>(null);

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

  useEffect(() => {
    const trackId = snapshot.trackId;
    const artworkId = snapshot.artworkId;
    const errorCode = snapshot.errorCode;
    const artworkTrackKey = trackId && snapshot.player
      ? `${snapshot.player}:${trackId}`
      : null;
    artworkRequestVersionRef.current += 1;
    const requestVersion = artworkRequestVersionRef.current;
    if (!active || !loadArtwork || !isTauriRuntime()) {
      if (artworkUrlRef.current) URL.revokeObjectURL(artworkUrlRef.current);
      artworkUrlRef.current = null;
      loadedArtworkIdRef.current = null;
      loadedArtworkTrackKeyRef.current = null;
      setArtworkUrl(null);
      setArtworkAccentColor(null);
      setArtworkLoading(false);
      setArtworkError(null);
      return;
    }

    if (errorCode === "source_not_allowed") {
      // 被过滤的系统播放器不应覆盖当前有效播放器的封面。
      setArtworkLoading(false);
      setArtworkError(null);
      return;
    }

    if (!trackId) {
      if (artworkUrlRef.current) URL.revokeObjectURL(artworkUrlRef.current);
      artworkUrlRef.current = null;
      loadedArtworkIdRef.current = null;
      loadedArtworkTrackKeyRef.current = null;
      setArtworkUrl(null);
      setArtworkAccentColor(null);
      setArtworkLoading(false);
      setArtworkError(null);
      return;
    }

    if (!artworkId) {
      if (
        artworkUrlRef.current
        && loadedArtworkTrackKeyRef.current === artworkTrackKey
      ) {
        // 系统封面来源短暂切换时，同一播放器的同一首歌继续复用当前封面。
        setArtworkLoading(false);
        setArtworkError(null);
        return;
      }
      // 系统媒体信息切歌时可能晚于歌曲元数据到达，确认期间继续显示上一首封面。
      setArtworkLoading(true);
      setArtworkError(null);
      const confirmationTimer = window.setTimeout(() => {
        if (artworkRequestVersionRef.current !== requestVersion) return;
        if (artworkUrlRef.current) URL.revokeObjectURL(artworkUrlRef.current);
        artworkUrlRef.current = null;
        loadedArtworkIdRef.current = null;
        loadedArtworkTrackKeyRef.current = null;
        setArtworkUrl(null);
        setArtworkAccentColor(null);
        setArtworkLoading(false);
      }, MISSING_ARTWORK_CONFIRMATION_MS);
      return () => {
        window.clearTimeout(confirmationTimer);
        if (artworkRequestVersionRef.current === requestVersion) {
          artworkRequestVersionRef.current += 1;
        }
      };
    }

    if (loadedArtworkIdRef.current === artworkId && artworkUrlRef.current) {
      // 暂停或恢复播放可能只改变歌曲来源标识，同一封面无需重新生成 Blob URL。
      setArtworkLoading(false);
      setArtworkError(null);
      return;
    }

    setArtworkLoading(true);
    setArtworkError(null);
    playerService.getArtwork(artworkId).then((value) => {
      if (artworkRequestVersionRef.current !== requestVersion) return;
      if (!value || value.id !== artworkId) {
        if (artworkUrlRef.current) URL.revokeObjectURL(artworkUrlRef.current);
        artworkUrlRef.current = null;
        loadedArtworkIdRef.current = null;
        loadedArtworkTrackKeyRef.current = null;
        setArtworkUrl(null);
        setArtworkAccentColor(null);
        return;
      }
      const nextUrl = artworkBlobUrl(value.mimeType, value.dataBase64);
      if (artworkRequestVersionRef.current !== requestVersion) {
        URL.revokeObjectURL(nextUrl);
        return;
      }
      const previousUrl = artworkUrlRef.current;
      artworkUrlRef.current = nextUrl;
      loadedArtworkIdRef.current = artworkId;
      loadedArtworkTrackKeyRef.current = artworkTrackKey;
      setArtworkUrl(nextUrl);
      setArtworkAccentColor(value.accentColor);
      if (previousUrl) URL.revokeObjectURL(previousUrl);
    }).catch((error) => {
      if (artworkRequestVersionRef.current !== requestVersion) return;
      if (artworkUrlRef.current) URL.revokeObjectURL(artworkUrlRef.current);
      artworkUrlRef.current = null;
      loadedArtworkIdRef.current = null;
      loadedArtworkTrackKeyRef.current = null;
      setArtworkUrl(null);
      setArtworkAccentColor(null);
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
  }, [
    active,
    loadArtwork,
    snapshot.artworkId,
    snapshot.errorCode,
    snapshot.player,
    snapshot.trackId,
  ]);

  useEffect(() => () => {
    artworkRequestVersionRef.current += 1;
    if (artworkUrlRef.current) URL.revokeObjectURL(artworkUrlRef.current);
    artworkUrlRef.current = null;
    loadedArtworkIdRef.current = null;
    loadedArtworkTrackKeyRef.current = null;
  }, []);

  useEffect(() => {
    if (!active || !trackPosition || !snapshot.isPlaying) return;
    setClock(Date.now());
    const timer = window.setInterval(() => setClock(Date.now()), 100);
    return () => window.clearInterval(timer);
  }, [active, snapshot.isPlaying, trackPosition]);

  const positionMs = useMemo(() => {
    const base = snapshot.positionMs ?? 0;
    if (!trackPosition || !snapshot.isPlaying) return base;
    return Math.min(snapshot.durationMs ?? Number.MAX_SAFE_INTEGER, base + Math.max(0, clock - snapshot.observedAtMs));
  }, [clock, snapshot, trackPosition]);

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

  const runPlayerOperation = useCallback((task: () => Promise<void>) => {
    if (controlPromiseRef.current) return controlPromiseRef.current;
    setControlError(null);
    setIsControlling(true);
    const operation = Promise.resolve()
      .then(task)
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

  const runControl = useCallback((action: PlaybackAction) => {
    return runPlayerOperation(() => playerService.control(action));
  }, [runPlayerOperation]);

  const seekTo = useCallback((positionMs: number) => {
    return runPlayerOperation(() => playerService.seek(positionMs));
  }, [runPlayerOperation]);

  const play = useCallback(() => runControl("play"), [runControl]);
  const pause = useCallback(() => runControl("pause"), [runControl]);
  const togglePlayPause = useCallback(
    () => runControl("toggle_play_pause"),
    [runControl],
  );
  const previousTrack = useCallback(() => runControl("previous"), [runControl]);
  const nextTrack = useCallback(() => runControl("next"), [runControl]);
  const clearControlError = useCallback(() => setControlError(null), []);
  return {
    active,
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
    seekTo,
    isControlling,
    controlError,
    clearControlError,
    artworkUrl,
    artworkAccentColor,
    artworkLoading,
    artworkError,
  };
}
