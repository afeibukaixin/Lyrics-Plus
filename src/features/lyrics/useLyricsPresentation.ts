import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauriRuntime, trackKeyOf } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import type { LyricsLine, LyricsRuntimeSnapshot, PlaybackSnapshot } from "../../shared/types";
import { findAlignedAuxiliaryLine } from "./useLyrics";

const emptySnapshot: LyricsRuntimeSnapshot = {
  trackKey: null,
  document: null,
  status: "idle",
  error: null,
};

export function useLyricsPresentation(snapshot: PlaybackSnapshot, positionMs: number, active = true) {
  const [runtime, setRuntime] = useState<LyricsRuntimeSnapshot>(emptySnapshot);
  const trackKey = useMemo(() => trackKeyOf(snapshot), [snapshot]);

  useEffect(() => {
    if (!active || !isTauriRuntime()) {
      setRuntime(emptySnapshot);
      return;
    }
    let disposed = false;
    void api.getLyricsRuntimeSnapshot().then((next) => {
      if (!disposed) setRuntime(next);
    }).catch(() => undefined);
    const cleanup = createTauriListenerCleanup(
      listen<LyricsRuntimeSnapshot>("lyrics://runtime-changed", ({ payload }) => {
        if (!disposed) setRuntime(payload);
      }),
    );
    return () => {
      disposed = true;
      cleanup();
    };
  }, [active]);

  const document = runtime.trackKey === trackKey ? runtime.document : null;
  const activeIndex = useMemo(() => {
    if (!document) return -1;
    const adjusted = positionMs + document.offsetMs;
    let found = -1;
    for (let index = 0; index < document.tracks.original.lines.length; index += 1) {
      if (document.tracks.original.lines[index].startMs > adjusted) break;
      found = index;
    }
    return found;
  }, [document, positionMs]);

  const currentLine: LyricsLine | null = document?.tracks.original.lines[activeIndex] ?? null;
  const nextLine: LyricsLine | null = document?.tracks.original.lines[activeIndex + 1] ?? null;
  const currentTranslation = useMemo(
    () => currentLine && document?.tracks.translation
      ? findAlignedAuxiliaryLine(document.tracks.translation.lines, currentLine)
      : null,
    [currentLine, document],
  );
  const currentRomanization = useMemo(
    () => currentLine && document?.tracks.romanization
      ? findAlignedAuxiliaryLine(document.tracks.romanization.lines, currentLine)
      : null,
    [currentLine, document],
  );

  return {
    trackKey,
    document,
    status: runtime.trackKey === trackKey ? runtime.status : "loading" as const,
    error: runtime.trackKey === trackKey ? runtime.error : null,
    activeIndex,
    currentLine,
    nextLine,
    currentTranslation,
    currentRomanization,
    adjustedPositionMs: positionMs + (document?.offsetMs ?? 0),
  };
}
