import { useCallback, useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api, isTauriRuntime } from "../../shared/api";
import type { PlaybackSnapshot } from "../../shared/types";

type ArtworkState = {
  url: string;
  loaded: boolean;
};

export function useArtwork(snapshot: PlaybackSnapshot) {
  const [artwork, setArtwork] = useState<ArtworkState | null>(null);
  const player = snapshot.player;
  const trackId = snapshot.trackId;

  useEffect(() => {
    setArtwork(null);
    if (!isTauriRuntime() || !player || !trackId) return;

    let current = true;
    void api.getTrackArtwork(player, trackId).then((asset) => {
      if (!current || !asset || asset.player !== player || asset.trackId !== trackId) return;
      setArtwork({ url: convertFileSrc(asset.filePath), loaded: false });
    }).catch(() => {
      if (current) setArtwork(null);
    });

    return () => {
      current = false;
    };
  }, [player, trackId]);

  const markLoaded = useCallback((url: string) => {
    setArtwork((current) => current?.url === url ? { ...current, loaded: true } : current);
  }, []);

  const markFailed = useCallback((url: string) => {
    setArtwork((current) => current?.url === url ? null : current);
  }, []);

  return {
    url: artwork?.url ?? null,
    loaded: artwork?.loaded ?? false,
    markLoaded,
    markFailed,
  };
}
