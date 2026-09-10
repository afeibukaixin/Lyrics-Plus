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

export type LyricsTimingMode = "continuous" | "line";

type LyricsPresentationOptions = {
  timing?: LyricsTimingMode;
};

function estimatedPosition(snapshot: PlaybackSnapshot, fallbackPositionMs: number) {
  const base = snapshot.positionMs ?? fallbackPositionMs;
  if (!snapshot.isPlaying) return base;
  return Math.min(
    snapshot.durationMs ?? Number.MAX_SAFE_INTEGER,
    base + Math.max(0, Date.now() - snapshot.observedAtMs),
  );
}

function nextLineStart(lines: LyricsLine[], adjustedPositionMs: number) {
  return lines.find((line) => line.text.trim() && line.startMs > adjustedPositionMs)?.startMs ?? null;
}

export function useLyricsPresentation(
  snapshot: PlaybackSnapshot,
  positionMs: number,
  active = true,
  { timing = "continuous" }: LyricsPresentationOptions = {},
) {
  const [runtime, setRuntime] = useState<LyricsRuntimeSnapshot>(emptySnapshot);
  const [linePositionMs, setLinePositionMs] = useState(
    () => snapshot.positionMs ?? positionMs,
  );
  const [lineTick, setLineTick] = useState(0);
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
  useEffect(() => {
    if (timing !== "line" || !active) return;

    const currentPositionMs = estimatedPosition(snapshot, positionMs);
    setLinePositionMs(currentPositionMs);
    if (!snapshot.isPlaying || !document) return;

    const adjustedPositionMs = currentPositionMs + document.offsetMs;
    const nextStartMs = nextLineStart(document.tracks.original.lines, adjustedPositionMs);
    if (nextStartMs === null) return;

    const delayMs = Math.max(16, nextStartMs - adjustedPositionMs);
    const timer = window.setTimeout(() => {
      setLineTick((value) => value + 1);
    }, delayMs);
    return () => window.clearTimeout(timer);
  }, [active, document, lineTick, positionMs, snapshot, timing]);

  const resolvedPositionMs = timing === "line" ? linePositionMs : positionMs;
  const activeIndex = useMemo(() => {
    if (!document) return -1;
    const adjusted = resolvedPositionMs + document.offsetMs;
    let found = -1;
    for (let index = 0; index < document.tracks.original.lines.length; index += 1) {
      const line = document.tracks.original.lines[index];
      if (line.startMs > adjusted) break;
      if (!line.text.trim()) continue;
      found = index;
    }
    return found;
  }, [document, resolvedPositionMs]);

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
    positionMs: resolvedPositionMs,
    status: runtime.trackKey === trackKey ? runtime.status : "loading" as const,
    error: runtime.trackKey === trackKey ? runtime.error : null,
    activeIndex,
    currentLine,
    nextLine,
    currentTranslation,
    currentRomanization,
    adjustedPositionMs: resolvedPositionMs + (document?.offsetMs ?? 0),
  };
}
