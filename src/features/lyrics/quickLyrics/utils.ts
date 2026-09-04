import type { LyricsSearchResult } from "../../../shared/types";

export function formatTime(value: number | null | undefined) {
  if (value == null || !Number.isFinite(value) || value <= 0) return "--:--";
  const seconds = Math.max(0, Math.round(value / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

export function resultKey(result: LyricsSearchResult) {
  return `${result.providerId}:${result.id}`;
}

export type DurationParts = {
  durationMinutes: string;
  durationSeconds: string;
};

export type SearchFormState = {
  title: string;
  artist: string;
  album: string;
} & DurationParts;

export function formatDurationParts(value: number | null | undefined): DurationParts {
  if (value == null) return { durationMinutes: "", durationSeconds: "" };
  const totalSeconds = Math.max(0, Math.round(value / 1000));
  return {
    durationMinutes: String(Math.floor(totalSeconds / 60)),
    durationSeconds: String(totalSeconds % 60).padStart(2, "0"),
  };
}

export function parseDuration(minutesValue: string, secondsValue: string): number | null | undefined {
  const minutesText = minutesValue.trim();
  const secondsText = secondsValue.trim();
  if (!minutesText && !secondsText) return null;
  if ((minutesText && !/^\d+$/.test(minutesText)) || (secondsText && !/^\d+$/.test(secondsText))) return undefined;

  const minutes = minutesText ? Number(minutesText) : 0;
  const seconds = secondsText ? Number(secondsText) : 0;
  if (!Number.isSafeInteger(minutes) || !Number.isSafeInteger(seconds) || seconds > 59) return undefined;
  const durationMs = (minutes * 60 + seconds) * 1000;
  return Number.isSafeInteger(durationMs) ? durationMs : undefined;
}
