import { useEffect, useMemo, useState } from "react";

import type { PlaybackSnapshot } from "../../../shared/types";

export function usePlaybackPosition(
  active: boolean,
  trackPosition: boolean,
  snapshot: PlaybackSnapshot,
) {
  const [clock, setClock] = useState(Date.now());

  useEffect(() => {
    if (!active || !trackPosition || !snapshot.isPlaying) return;
    setClock(Date.now());
    const timer = window.setInterval(() => setClock(Date.now()), 100);
    return () => window.clearInterval(timer);
  }, [active, snapshot.isPlaying, trackPosition]);

  return useMemo(() => {
    const base = snapshot.positionMs ?? 0;
    if (!trackPosition || !snapshot.isPlaying) return base;
    return Math.min(snapshot.durationMs ?? Number.MAX_SAFE_INTEGER, base + Math.max(0, clock - snapshot.observedAtMs));
  }, [clock, snapshot, trackPosition]);
}
