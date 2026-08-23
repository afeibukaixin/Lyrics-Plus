import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";

type OffsetPreview = { trackKey: string; offsetMs: number };

type UseNotchLyricsOffsetOptions = {
  trackKey: string | null;
  hasDocument: boolean;
  runtimeOffsetMs: number;
};

/** Keeps Dynamic Island offset edits optimistic while persisting them in order. */
export function useNotchLyricsOffset({
  trackKey,
  hasDocument,
  runtimeOffsetMs,
}: UseNotchLyricsOffsetOptions) {
  const [offsetPreview, setOffsetPreview] = useState<OffsetPreview | null>(null);
  const pendingOffsetRef = useRef(0);
  const offsetWriteQueue = useRef<Promise<void>>(Promise.resolve());
  const offsetWriteVersionRef = useRef(0);
  const runtimeOffsetRef = useRef({ trackKey, offsetMs: runtimeOffsetMs });
  runtimeOffsetRef.current = { trackKey, offsetMs: runtimeOffsetMs };
  const offsetAvailable = Boolean(hasDocument && trackKey);
  const offsetMs = offsetPreview?.trackKey === trackKey
    ? offsetPreview.offsetMs
    : runtimeOffsetMs;

  useEffect(() => {
    offsetWriteVersionRef.current += 1;
    pendingOffsetRef.current = runtimeOffsetMs;
    setOffsetPreview(null);
  }, [trackKey]);

  useEffect(() => {
    pendingOffsetRef.current = runtimeOffsetMs;
    if (offsetPreview?.trackKey !== trackKey) return;
    if (offsetPreview.offsetMs === runtimeOffsetMs) setOffsetPreview(null);
  }, [offsetPreview, runtimeOffsetMs, trackKey]);

  const setLyricsOffset = useCallback((nextOffsetMs: number) => {
    if (!trackKey || !hasDocument) return;
    const version = offsetWriteVersionRef.current + 1;
    offsetWriteVersionRef.current = version;
    const nextOffset = Math.trunc(nextOffsetMs);
    pendingOffsetRef.current = nextOffset;
    setOffsetPreview({ trackKey, offsetMs: nextOffset });
    const operation = offsetWriteQueue.current
      .then(() => api.setLyricsOffset(trackKey, nextOffset))
      .catch((error) => {
        if (offsetWriteVersionRef.current === version) {
          setOffsetPreview(null);
          if (runtimeOffsetRef.current.trackKey === trackKey) {
            pendingOffsetRef.current = runtimeOffsetRef.current.offsetMs;
          }
        }
        reportFrontendError("Failed to update the Dynamic Island lyrics offset", error);
      });
    offsetWriteQueue.current = operation;
  }, [hasDocument, trackKey]);

  const changeLyricsOffset = useCallback((deltaMs: number) => {
    if (!offsetAvailable) return;
    setLyricsOffset(pendingOffsetRef.current + deltaMs);
  }, [offsetAvailable, setLyricsOffset]);

  const resetLyricsOffset = useCallback(() => {
    if (!offsetAvailable) return;
    setLyricsOffset(0);
  }, [offsetAvailable, setLyricsOffset]);

  return {
    changeLyricsOffset,
    offsetAvailable,
    offsetMs,
    resetLyricsOffset,
    setLyricsOffset,
  };
}
