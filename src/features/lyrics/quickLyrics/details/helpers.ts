import type { TFunction } from "i18next";
import type { useLyrics } from "../../useLyrics";
import type { LyricsContext, LyricsSearchResult, LyricsSearchTrace, PlaybackSnapshot, SongAssociationCandidate, TrackObservation } from "@/shared/types";
export type LyricsController = ReturnType<typeof useLyrics>;

export type QuickLyricsDetailsProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  playback: PlaybackSnapshot;
  lyrics: LyricsController;
  onSearch: () => void | Promise<unknown>;
  t: TFunction;
};

export function formatDuration(durationMs: number | null) {
  if (durationMs === null || !Number.isFinite(durationMs)) return "—";
  const totalSeconds = Math.max(0, Math.round(durationMs / 1000));
  return `${Math.floor(totalSeconds / 60)}:${String(totalSeconds % 60).padStart(2, "0")}`;
}

export function associationCandidateDuration(candidate: SongAssociationCandidate) {
  // 平台观察可能没有时长，但歌曲实体仍可能保留首次观察到的时长。
  return candidate.observations
    .slice()
    .sort((left, right) => right.observedAt - left.observedAt)[0]?.durationMs
    ?? candidate.observations.find((observation) => observation.durationMs !== null)?.durationMs
    ?? candidate.durationMs;
}

export function observationDetails(observation: TrackObservation) {
  return `${observation.rawTitle} · ${observation.rawArtists.join(" / ")}`;
}

export function assetPath(context: LyricsContext) {
  const asset = context.currentAsset;
  if (!asset) return null;
  return [asset.rootName, asset.relativePath].filter(Boolean).join(" / ") || asset.sourceName;
}

export function resultTrace(result: LyricsSearchResult, trace: LyricsSearchTrace | null) {
  return trace?.candidates.find((candidate) => candidate.providerId === result.providerId && candidate.providerItemId === result.id);
}

export type RecordingAction = { kind: "split"; observation: TrackObservation } | null;
export type DetachLyricsMode = "inheritCurrent" | "unbound";

export function associationReasonLabel(reason: string, t: TFunction) {
  return t(`quickLyrics.details.associationReasons.${reason}`, {
    reason,
    defaultValue: t("quickLyrics.details.associationEvidenceItem", { reason, defaultValue: reason }),
  });
}

export function normalizeArtistName(value: string) {
  return value.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");
}
