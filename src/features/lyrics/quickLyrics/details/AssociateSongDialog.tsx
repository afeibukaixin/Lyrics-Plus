import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import type { SongManagerViewProps } from "./useSongManager";
import { CandidateEvidence } from "./CandidateEvidence";

import { formatDuration, associationCandidateDuration, observationDetails, associationReasonLabel } from "./helpers";
export function AssociateSongDialog({ associationCandidate, setAssociationCandidate, context, t,
  busyAction, handleAssociateCandidate }: SongManagerViewProps) {
  return (    <AlertDialog open={associationCandidate !== null} onOpenChange={(isOpen) => { if (!isOpen) setAssociationCandidate(null); }}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("quickLyrics.details.mergeRecordingTitle")}</AlertDialogTitle>
          <AlertDialogDescription>{t("quickLyrics.details.mergeRecordingDescription", {
            currentPlatform: context?.platform,
            candidatePlatform: associationCandidate?.observations.map((observation) => observation.platform).join(" / "),
          })}</AlertDialogDescription>
        </AlertDialogHeader>
        {associationCandidate && <div className="grid gap-2 text-sm">
          <div><strong>{t("quickLyrics.details.mergeCurrentLabel")}</strong> {context?.recording.title} · {context?.recording.artistCredits.map((credit) => credit.canonicalName).join(" / ")} · {formatDuration(context?.recording.durationMs ?? null)}</div>
          <div><strong>{t("quickLyrics.details.mergeCandidateLabel")}</strong> {associationCandidate.title} · {associationCandidate.observations.map((observation) => observation.platform).join(" / ")} · {formatDuration(associationCandidateDuration(associationCandidate))}</div>
          <div className="grid gap-1 pt-2 text-muted-foreground">
            {associationCandidate.observations.map((observation) => <span key={observation.observationId}>{observation.platform}: {observationDetails(observation)} · {observation.trackKey}</span>)}
          </div>
          <p className="pt-2 text-muted-foreground">{associationCandidate.matchReasons.map((reason) => associationReasonLabel(reason, t)).join(" · ")}</p>
          <CandidateEvidence candidate={associationCandidate} t={t} />
        </div>}
        <AlertDialogFooter>
          <AlertDialogCancel disabled={busyAction !== null}>{t("common.actions.cancel")}</AlertDialogCancel>
          <AlertDialogAction disabled={busyAction !== null || !associationCandidate?.canAssociate} onClick={handleAssociateCandidate}>{t("quickLyrics.details.mergeRecording")}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>);
}
