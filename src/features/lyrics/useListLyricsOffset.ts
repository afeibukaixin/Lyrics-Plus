import { useEffect, useRef } from "react";
import { api } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";

type UseListLyricsOffsetOptions = {
  trackKey: string | null;
  hasDocument: boolean;
  offsetMs: number;
};

export function useListLyricsOffset({
  trackKey,
  hasDocument,
  offsetMs,
}: UseListLyricsOffsetOptions) {
  const pendingOffsetRef = useRef(0);
  const offsetWriteQueue = useRef<Promise<unknown>>(Promise.resolve());
  const offsetAvailable = Boolean(hasDocument && trackKey);

  useEffect(() => {
    pendingOffsetRef.current = offsetMs;
  }, [offsetMs]);

  const setLyricsOffset = (nextOffsetMs: number) => {
    if (!trackKey) return;
    pendingOffsetRef.current = nextOffsetMs;
    const currentTrackKey = trackKey;
    offsetWriteQueue.current = offsetWriteQueue.current
      .then(() => api.setLyricsOffset(currentTrackKey, nextOffsetMs))
      .catch((error) => reportFrontendError("Failed to update the lyrics window offset", error));
  };

  const changeLyricsOffset = (deltaMs: number) => {
    setLyricsOffset(pendingOffsetRef.current + deltaMs);
  };

  return {
    offsetAvailable,
    offsetMs,
    setLyricsOffset,
    changeLyricsOffset,
  };
}
