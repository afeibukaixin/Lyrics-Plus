import { useCallback, useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, isTauriRuntime } from "../../shared/api";
import { useAppConfig } from "../config/AppConfigProvider";
import { resolveLanguage } from "../i18n/i18n";
import { itunesCountryForLanguage } from "../../shared/languages";
import type { PlaybackSnapshot } from "../../shared/types";

type ArtworkState = {
  key: string;
  url: string | null;
  loaded: boolean;
  loading: boolean;
  source: string | null;
  sourceLink: string | null;
};

let cachedArtwork: ArtworkState | null = null;

export function invalidateArtworkCache() {
  cachedArtwork = null;
}

export function useArtwork(snapshot: PlaybackSnapshot) {
  const { config } = useAppConfig();
  const [artwork, setArtwork] = useState<ArtworkState | null>(null);
  const player = snapshot.player;
  const trackId = snapshot.trackId;
  const itunesCountry = config.artwork.itunesStorefront === "auto"
    ? itunesCountryForLanguage(resolveLanguage(config.app.language))
    : config.artwork.itunesStorefront;
  const cacheKey = player && trackId
    ? JSON.stringify([player, trackId, config.artwork, itunesCountry])
    : null;

  useEffect(() => {
    if (cacheKey && cachedArtwork?.key === cacheKey) {
      setArtwork(cachedArtwork);
      return;
    }
    setArtwork(null);
    if (!isTauriRuntime() || !player || !trackId || !cacheKey) return;
    cachedArtwork = null;
    setArtwork({ key: cacheKey, url: null, loaded: false, loading: true, source: null, sourceLink: null });

    let current = true;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let attempts = 0;
    const load = () => {
      attempts += 1;
      void api.getTrackArtwork(player, trackId, player !== "system" || attempts >= 5, itunesCountry).then((asset) => {
        if (!current) return;
        if (!asset || asset.player !== player || asset.trackId !== trackId) {
          if (player === "system" && attempts < 5) retry = setTimeout(load, 1_000);
          else setArtwork(null);
          return;
        }
        const next = { key: cacheKey, url: convertFileSrc(asset.filePath), loaded: false, loading: true, source: asset.source, sourceLink: asset.sourceLink };
        cachedArtwork = next;
        setArtwork(next);
      }).catch(() => {
        if (current) setArtwork(null);
      });
    };
    load();

    return () => {
      current = false;
      if (retry) clearTimeout(retry);
    };
  }, [cacheKey, player, trackId, itunesCountry]);

  const markLoaded = useCallback((url: string) => {
    setArtwork((current) => {
      if (!cacheKey || current?.key !== cacheKey || current.url !== url) return current;
      const next = { ...current, loaded: true, loading: false };
      if (cachedArtwork?.key === cacheKey && cachedArtwork.url === url) cachedArtwork = next;
      return next;
    });
  }, [cacheKey]);

  const markFailed = useCallback((url: string) => {
    setArtwork((current) => {
      if (!cacheKey || current?.key !== cacheKey || current.url !== url) return current;
      if (cachedArtwork?.key === cacheKey && cachedArtwork.url === url) cachedArtwork = null;
      return null;
    });
  }, [cacheKey]);

  return {
    url: artwork?.url ?? null,
    loaded: artwork?.loaded ?? false,
    loading: artwork?.loading ?? false,
    source: artwork?.source ?? null,
    sourceLink: artwork?.sourceLink ?? null,
    markLoaded,
    markFailed,
  };
}
