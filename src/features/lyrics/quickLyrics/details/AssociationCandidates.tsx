import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { CandidateEvidence } from "./CandidateEvidence";
import { associationCandidateDuration, observationDetails, associationReasonLabel } from "./helpers";
import { formatDuration } from "./helpers";
import type { SongManagerViewProps } from "./useSongManager";
import styles from "../../QuickLyricsWindow.module.scss";

export function AssociationCandidates({ context, t, associationCandidates, busyAction, openAssociationDialog }: SongManagerViewProps) {
  if (!context || associationCandidates.length === 0) return null;
  return (<section className={styles.detailsSection}>              {associationCandidates.length > 0 && <div className={styles.detailsList}>
                <strong>{t("quickLyrics.details.mergeCandidates")}</strong>
                {associationCandidates.map((candidate) => <div className={cn(styles.detailsObservation, styles.detailsMergeCandidate)} key={candidate.recordingId}>
                  <div className={styles.detailsMergeCandidateInfo}>
                    <div className={styles.detailsMergeCandidateTitle}>
                      {candidate.isOriginalRecording && <Badge>{t("quickLyrics.details.originalRelation")}</Badge>}
                      <span>{candidate.title} · {candidate.observations.map((observation) => observation.platform).join(" / ")}</span>
                    </div>
                    {candidate.observations.map((observation) => <small key={observation.observationId}>{observation.platform}: {observationDetails(observation)} · {observation.rawAlbum ?? candidate.album ?? t("quickLyrics.unknown")} · {formatDuration(observation.durationMs ?? candidate.durationMs)} · {observation.trackKey}</small>)}
                    <small>{candidate.album ?? candidate.observations.find((observation) => observation.rawAlbum)?.rawAlbum ?? t("quickLyrics.unknown")} · {formatDuration(associationCandidateDuration(candidate))} · {Math.round(candidate.score * 100)}%</small>
                    <small>{t("quickLyrics.details.scoreReasons.duration", {
                      value: candidate.durationDeltaMs === null
                        ? t("quickLyrics.details.scoreReasons.durationUnknown")
                        : `${(candidate.durationDeltaMs / 1000).toFixed(1)}s`,
                    })}</small>
                    <small>{t("quickLyrics.details.version")}: {candidate.versionTags.join(" / ") || t("quickLyrics.details.unspecified")}</small>
                    <small>{candidate.externalIdentifiers.length > 0
                      ? candidate.externalIdentifiers.map((identifier) => `${identifier.namespace}/${identifier.idKind}: ${identifier.value} · ${identifier.confirmed ? t("quickLyrics.details.confirmed") : t("quickLyrics.details.unconfirmed")}`).join(" · ")
                      : `${t("quickLyrics.details.platformId")}：${candidate.observations.map((observation) => `${observation.platform}/${observation.trackKey}`).join(" · ")} · ${t("quickLyrics.details.unconfirmed")}`}</small>
                    {candidate.isrc && <small>{t("quickLyrics.details.associationIsrc", { value: candidate.isrc })}</small>}
                    <small>{t("quickLyrics.details.associationSimilarity", { title: Math.round(candidate.titleSimilarity * 100), artist: Math.round(candidate.artistSimilarity * 100), album: Math.round(candidate.albumSimilarity * 100) })}</small>
                    <small>{candidate.matchReasons.map((reason) => associationReasonLabel(reason, t)).join(" · ")}</small>
                    {candidate.confirmedArtistAliases.length > 0 && <small>{t("quickLyrics.details.artistAliases")}: {candidate.confirmedArtistAliases.join(" / ")}</small>}
                    <CandidateEvidence candidate={candidate} t={t} />
                  </div>
                  <Button variant="secondary" size="sm" disabled={busyAction !== null || !candidate.canAssociate} onClick={() => openAssociationDialog(candidate)}>{t("quickLyrics.details.mergeRecording")}</Button>
                </div>)}
              </div>}</section>);
}
