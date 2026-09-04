import { useMemo } from "react";

import type { LyricsDocument, LyricsLine } from "../../../shared/types";

const AUXILIARY_TIMESTAMP_TOLERANCE_MS = 500;

export function findAlignedAuxiliaryLine(lines: LyricsLine[], currentLine: LyricsLine) {
  const exact = lines.find((line) => line.startMs === currentLine.startMs && line.text.trim());
  if (exact) return exact;
  let nearest: LyricsLine | null = null;
  let nearestDelta = AUXILIARY_TIMESTAMP_TOLERANCE_MS + 1;
  for (const line of lines) {
    if (!line.text.trim()) continue;
    const delta = Math.abs(line.startMs - currentLine.startMs);
    if (delta < nearestDelta) {
      nearest = line;
      nearestDelta = delta;
    }
  }
  return nearestDelta <= AUXILIARY_TIMESTAMP_TOLERANCE_MS ? nearest : null;
}

export function useLyricsDisplay(document: LyricsDocument | null, positionMs: number) {
  const activeIndex = useMemo(() => {
    if (!document) return -1;
    const adjusted = positionMs + document.offsetMs;
    let found = -1;
    const lines = document.tracks.original.lines;
    for (let index = 0; index < lines.length; index += 1) {
      if (lines[index].startMs > adjusted) break;
      found = index;
    }
    return found;
  }, [document, positionMs]);

  const originalLines = document?.tracks.original.lines;
  const currentLine: LyricsLine | null = originalLines?.[activeIndex] ?? null;
  const nextLine: LyricsLine | null = originalLines?.[activeIndex + 1] ?? null;
  const currentTranslation: LyricsLine | null = useMemo(() => {
    if (!currentLine || !document?.tracks.translation) return null;
    return findAlignedAuxiliaryLine(document.tracks.translation.lines, currentLine);
  }, [currentLine, document]);
  const currentRomanization: LyricsLine | null = useMemo(() => {
    if (!currentLine || !document?.tracks.romanization) return null;
    return findAlignedAuxiliaryLine(document.tracks.romanization.lines, currentLine);
  }, [currentLine, document]);

  return {
    activeIndex,
    currentLine,
    nextLine,
    currentTranslation,
    currentRomanization,
    adjustedPositionMs: positionMs + (document?.offsetMs ?? 0),
  };
}
