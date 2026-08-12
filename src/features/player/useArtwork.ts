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

function preloadArtwork(url: string) {
  return new Promise<void>((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve();
    image.onerror = () => reject(new Error("artwork decode failed"));
    image.src = url;
  });
}

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
    if (!isTauriRuntime() || !player || !trackId || !cacheKey) {
      setArtwork(null);
      return;
    }
    cachedArtwork = null;
    setArtwork((current) => current?.url
      ? current
      : { key: cacheKey, url: null, loaded: false, loading: true, source: null, sourceLink: null });

    let current = true;
    let hasNewArtwork = false;
    const sleep = (delay: number) => new Promise<void>((resolve) => setTimeout(resolve, delay));
    const placeholderTimer = setTimeout(() => {
      if (!current || hasNewArtwork) return;
      setArtwork({ key: cacheKey, url: null, loaded: false, loading: true, source: null, sourceLink: null });
    }, 1_200);
    const show = async (asset: Awaited<ReturnType<typeof api.getTrackArtwork>>) => {
      if (!asset || asset.player !== player || asset.trackId !== trackId) return false;
      const url = convertFileSrc(asset.filePath);
      try {
        await preloadArtwork(url);
      } catch {
        return false;
      }
      if (!current) return false;
      const next = { key: cacheKey, url, loaded: true, loading: false, source: asset.source, sourceLink: asset.sourceLink };
      hasNewArtwork = true;
      cachedArtwork = next;
      setArtwork(next);
      return true;
    };
    const load = async (allowNetwork: boolean) => {
      try {
        return await show(await api.getTrackArtwork(player, trackId, allowNetwork, itunesCountry));
      } catch {
        return false;
      }
    };
    const finishMissing = () => {
      if (!current || hasNewArtwork) return;
      setArtwork({ key: cacheKey, url: null, loaded: false, loading: false, source: null, sourceLink: null });
    };

    if (player === "system") {
      void load(false);
      void (async () => {
        await sleep(300);
        if (current && !hasNewArtwork) await load(false);
      })();
      void (async () => {
        await sleep(900);
        if (current && !await load(true)) finishMissing();
      })();
    } else {
      void (async () => {
        const finalDelay = sleep(1_200);
        const directAttempts = await Promise.all([
          load(false),
          (async () => {
            await sleep(400);
            return current && !hasNewArtwork ? load(false) : false;
          })(),
        ]);
        await finalDelay;
        if (!current || directAttempts.some(Boolean) || hasNewArtwork) return;
        if (!await load(true)) finishMissing();
      })();
    }

    return () => {
      current = false;
      clearTimeout(placeholderTimer);
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
    source: artwork?.key === cacheKey ? artwork.source : null,
    sourceLink: artwork?.key === cacheKey ? artwork.sourceLink : null,
    markLoaded,
    markFailed,
  };
}
