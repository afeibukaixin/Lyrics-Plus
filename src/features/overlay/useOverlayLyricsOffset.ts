import { useCallback, useEffect, useRef } from "react";
import { api } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";

/**
 * Serializes offset writes so rapid toolbar clicks keep their order while the
 * displayed value remains responsive to the user's latest adjustment.
 */
export function useOverlayLyricsOffset(trackKey: string | null, offsetMs: number) {
  const pendingOffsetRef = useRef(offsetMs);
  const offsetWriteQueue = useRef<Promise<unknown>>(Promise.resolve());

  useEffect(() => {
    pendingOffsetRef.current = offsetMs;
  }, [offsetMs]);

  const setLyricsOffset = useCallback((nextOffsetMs: number) => {
    if (!trackKey) return;
    pendingOffsetRef.current = nextOffsetMs;
    const currentTrackKey = trackKey;
    offsetWriteQueue.current = offsetWriteQueue.current
      .then(() => api.setLyricsOffset(currentTrackKey, nextOffsetMs))
      .catch((error) => reportFrontendError("Failed to update the lyrics offset", error));
  }, [trackKey]);

  const changeLyricsOffset = useCallback((deltaMs: number) => {
    setLyricsOffset(pendingOffsetRef.current + deltaMs);
  }, [setLyricsOffset]);

  return { setLyricsOffset, changeLyricsOffset };
}
