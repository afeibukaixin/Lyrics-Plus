import { useCallback, useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, isTauriRuntime } from "../../shared/api";
import { useAppConfig } from "../config/AppConfigProvider";
import { resolveLanguage } from "../i18n/i18n";
import { itunesCountryForLanguage } from "../../shared/languages";
import type { PlaybackSnapshot } from "../../shared/types";

type ArtworkState = {
  url: string;
  loaded: boolean;
  source: string;
  sourceLink: string | null;
};

export function useArtwork(snapshot: PlaybackSnapshot) {
  const { config } = useAppConfig();
  const [artwork, setArtwork] = useState<ArtworkState | null>(null);
  const player = snapshot.player;
  const trackId = snapshot.trackId;
  const itunesCountry = config.artwork.itunesStorefront === "auto"
    ? itunesCountryForLanguage(resolveLanguage(config.app.language))
    : config.artwork.itunesStorefront;

  useEffect(() => {
    setArtwork(null);
    if (!isTauriRuntime() || !player || !trackId) return;

    let current = true;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let attempts = 0;
    const load = () => {
      attempts += 1;
      void api.getTrackArtwork(player, trackId, player !== "system" || attempts >= 5, itunesCountry).then((asset) => {
        if (!current) return;
        if (!asset || asset.player !== player || asset.trackId !== trackId) {
          if (player === "system" && attempts < 5) retry = setTimeout(load, 1_000);
          return;
        }
        setArtwork({ url: convertFileSrc(asset.filePath), loaded: false, source: asset.source, sourceLink: asset.sourceLink });
      }).catch(() => {
        if (current) setArtwork(null);
      });
    };
    load();

    return () => {
      current = false;
      if (retry) clearTimeout(retry);
    };
  }, [player, trackId, itunesCountry]);

  const markLoaded = useCallback((url: string) => {
    setArtwork((current) => current?.url === url ? { ...current, loaded: true } : current);
  }, []);

  const markFailed = useCallback((url: string) => {
    setArtwork((current) => current?.url === url ? null : current);
  }, []);

  return {
    url: artwork?.url ?? null,
    loaded: artwork?.loaded ?? false,
    source: artwork?.source ?? null,
    sourceLink: artwork?.sourceLink ?? null,
    markLoaded,
    markFailed,
  };
}
