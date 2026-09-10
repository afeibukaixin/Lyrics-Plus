import { useCallback, useEffect, useRef, useState } from "react";

import { api } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import type {
  CompactLyricsPresentation,
  LyricsDocument,
  LyricsLine,
  PlaybackSnapshot,
  SupportingLyricsPriority,
} from "../../shared/types";
import { findAlignedAuxiliaryLine } from "./useLyrics/display";
import { useLyricsPresentation, type LyricsTimingMode } from "./useLyricsPresentation";

export type CompactLyricsLineKind = "fallback" | "primary" | "next" | "translation" | "romanization";

export type CompactLyricsResolvedLine = {
  kind: CompactLyricsLineKind;
  line: LyricsLine | null;
  text: string;
};

export type CompactLyricsPresentationResult = {
  activeIndex: number;
  adjustedPositionMs: number;
  currentLine: LyricsLine | null;
  nextLine: LyricsLine | null;
  currentTranslation: LyricsLine | null;
  currentRomanization: LyricsLine | null;
  translationAvailable: boolean;
  romanizationAvailable: boolean;
  supportingAvailable: boolean;
  primaryLine: CompactLyricsResolvedLine;
  supportingLine: CompactLyricsResolvedLine | null;
  doubleLineOrder: "normal" | "reversed";
};

type ResolveOptions = {
  document: LyricsDocument | null;
  positionMs: number;
  title: string | null | undefined;
  artist: string | null | undefined;
  presentation: CompactLyricsPresentation;
  primaryLinePosition?: "first" | "second";
  offsetMs?: number;
};

function findActiveIndex(lines: LyricsLine[], positionMs: number) {
  let activeIndex = -1;
  for (let index = 0; index < lines.length; index += 1) {
    if (lines[index].startMs > positionMs) break;
    activeIndex = index;
  }
  return activeIndex;
}

function clean(value: string | null | undefined) {
  return value?.trim() ?? "";
}

function auxiliaryKinds(
  presentation: CompactLyricsPresentation,
): Array<"translation" | "romanization"> {
  const preferred = presentation.supportingPriority;
  const fallback = preferred === "translation" ? "romanization" : "translation";
  const enabled = new Set(
    [
      presentation.showTranslation ? "translation" : null,
      presentation.showRomanization ? "romanization" : null,
    ].filter((kind): kind is "translation" | "romanization" => kind !== null),
  );
  const orderedKinds: SupportingLyricsPriority[] = [preferred, fallback];
  return orderedKinds.filter((kind, index, kinds) => (
    enabled.has(kind) && kinds.indexOf(kind) === index
  ));
}

/**
 * Resolves the display lines shared by the desktop, status-bar and notch
 * surfaces. Rendering and carrier-specific styling stay outside this function.
 */
export function resolveCompactLyricsPresentation({
  document,
  positionMs,
  title,
  artist,
  presentation,
  primaryLinePosition = "first",
  offsetMs,
}: ResolveOptions): CompactLyricsPresentationResult {
  const effectiveOffsetMs = offsetMs ?? document?.offsetMs ?? 0;
  const adjustedPositionMs = positionMs + effectiveOffsetMs;
  const originalLines = document?.tracks.original.lines.filter((line) => line.text.trim()) ?? [];
  const translationAvailable = Boolean(document?.tracks.translation);
  const romanizationAvailable = Boolean(document?.tracks.romanization);
  const activeIndex = findActiveIndex(originalLines, adjustedPositionMs);
  const currentLine = originalLines[activeIndex] ?? null;
  const nextLine = originalLines[activeIndex + 1] ?? null;
  const currentTranslation = currentLine && document?.tracks.translation
    ? findAlignedAuxiliaryLine(document.tracks.translation.lines, currentLine)
    : null;
  const currentRomanization = currentLine && document?.tracks.romanization
    ? findAlignedAuxiliaryLine(document.tracks.romanization.lines, currentLine)
    : null;
  const primaryLine: CompactLyricsResolvedLine = currentLine
    ? { kind: "primary", line: currentLine, text: currentLine.text }
    : {
      kind: "fallback",
      line: null,
      text: clean(title) || "Lyrics Plus",
    };

  let supportingLine: CompactLyricsResolvedLine | null = null;
  if (presentation.layout === "double") {
    if (!currentLine) {
      const fallbackArtist = clean(artist);
      supportingLine = fallbackArtist
        ? { kind: "fallback", line: null, text: fallbackArtist }
        : null;
    } else {
      for (const kind of auxiliaryKinds(presentation)) {
        const line = kind === "translation" ? currentTranslation : currentRomanization;
        if (line && clean(line.text)) {
          supportingLine = { kind, line, text: line.text };
          break;
        }
      }
      if (!supportingLine && nextLine) {
        supportingLine = { kind: "next", line: nextLine, text: nextLine.text };
      }
    }
  }

  const alternating = presentation.layout === "double"
    && presentation.doubleLineMode === "alternating"
    && activeIndex >= 0
    && supportingLine?.kind === "next"
    && activeIndex % 2 === 1;
  // 歌曲标题/歌手是无当前歌词时的元数据回退，始终保持标题在前、歌手在后。
  const fixedReversed = primaryLine.kind === "primary" && primaryLinePosition === "second";
  const doubleLineOrder = fixedReversed !== alternating ? "reversed" : "normal";

  return {
    activeIndex,
    adjustedPositionMs,
    currentLine,
    nextLine,
    currentTranslation,
    currentRomanization,
    translationAvailable,
    romanizationAvailable,
    supportingAvailable: Boolean(supportingLine),
    primaryLine,
    supportingLine,
    doubleLineOrder,
  };
}

type OffsetPreview = { trackKey: string; offsetMs: number };

type LyricsOffsetOptions = {
  trackKey: string | null;
  hasDocument: boolean;
  runtimeOffsetMs: number;
  errorMessage: string;
};

function useCompactLyricsOffset({
  trackKey,
  hasDocument,
  runtimeOffsetMs,
  errorMessage,
}: LyricsOffsetOptions) {
  const [offsetPreview, setOffsetPreview] = useState<OffsetPreview | null>(null);
  const pendingOffsetRef = useRef(runtimeOffsetMs);
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
    if (offsetPreview?.trackKey !== trackKey) {
      pendingOffsetRef.current = runtimeOffsetMs;
      return;
    }
    pendingOffsetRef.current = offsetPreview.offsetMs;
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
        reportFrontendError(errorMessage, error);
      });
    offsetWriteQueue.current = operation;
  }, [errorMessage, hasDocument, trackKey]);

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

type CompactLyricsHookOptions = {
  snapshot: PlaybackSnapshot;
  positionMs: number;
  active?: boolean;
  timing?: LyricsTimingMode;
  presentation: CompactLyricsPresentation;
  title?: string | null;
  artist?: string | null;
  primaryLinePosition?: "first" | "second";
  offsetErrorMessage: string;
};

export function useCompactLyricsPresentation({
  snapshot,
  positionMs,
  active = true,
  timing = "continuous",
  presentation,
  title = snapshot.title,
  artist = snapshot.artist,
  primaryLinePosition = "first",
  offsetErrorMessage,
}: CompactLyricsHookOptions) {
  const runtime = useLyricsPresentation(snapshot, positionMs, active, { timing });
  const offset = useCompactLyricsOffset({
    trackKey: runtime.trackKey,
    hasDocument: Boolean(runtime.document),
    runtimeOffsetMs: runtime.document?.offsetMs ?? 0,
    errorMessage: offsetErrorMessage,
  });
  const resolved = resolveCompactLyricsPresentation({
    document: runtime.document,
    positionMs: runtime.positionMs,
    title,
    artist,
    presentation,
    primaryLinePosition,
    offsetMs: offset.offsetMs,
  });

  return {
    ...runtime,
    ...resolved,
    ...offset,
  };
}
