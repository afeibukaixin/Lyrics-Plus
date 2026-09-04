import { useEffect, useRef, useState } from "react";

import { isTauriRuntime, messageOf } from "../../../shared/api";
import type { PlaybackSnapshot, PlaybackSpectrumColors } from "../../../shared/types";

import { playerService } from "../playerService";

const MISSING_ARTWORK_CONFIRMATION_MS = 2_500;

function artworkBlobUrl(mimeType: string, dataBase64: string) {
  const decoded = atob(dataBase64);
  const bytes = new Uint8Array(decoded.length);
  for (let index = 0; index < decoded.length; index += 1) bytes[index] = decoded.charCodeAt(index);
  return URL.createObjectURL(new Blob([bytes], { type: mimeType }));
}

export function usePlaybackArtwork(
  active: boolean,
  loadArtwork: boolean,
  snapshot: PlaybackSnapshot,
) {
  const [artworkUrl, setArtworkUrl] = useState<string | null>(null);
  const [artworkAccentColor, setArtworkAccentColor] = useState<string | null>(null);
  const [artworkSpectrumColors, setArtworkSpectrumColors] = useState<PlaybackSpectrumColors | null>(null);
  const [artworkLoading, setArtworkLoading] = useState(false);
  const [artworkError, setArtworkError] = useState<string | null>(null);
  const artworkRequestVersionRef = useRef(0);
  const loadedArtworkIdRef = useRef<string | null>(null);
  const loadedArtworkTrackKeyRef = useRef<string | null>(null);
  const artworkUrlRef = useRef<string | null>(null);

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
      setArtworkSpectrumColors(null);
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
      setArtworkSpectrumColors(null);
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
        setArtworkSpectrumColors(null);
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
        setArtworkSpectrumColors(null);
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
      setArtworkSpectrumColors(value.spectrumColors);
      if (previousUrl) URL.revokeObjectURL(previousUrl);
    }).catch((error) => {
      if (artworkRequestVersionRef.current !== requestVersion) return;
      if (artworkUrlRef.current) URL.revokeObjectURL(artworkUrlRef.current);
      artworkUrlRef.current = null;
      loadedArtworkIdRef.current = null;
      loadedArtworkTrackKeyRef.current = null;
      setArtworkUrl(null);
      setArtworkAccentColor(null);
      setArtworkSpectrumColors(null);
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

  return {
    artworkUrl,
    artworkAccentColor,
    artworkSpectrumColors,
    artworkLoading,
    artworkError,
  };
}
