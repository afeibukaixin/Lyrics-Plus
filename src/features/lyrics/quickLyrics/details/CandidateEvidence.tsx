import type { TFunction } from "i18next";
import type { SongAssociationCandidate } from "@/shared/types";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription } from "@/components/ui/alert";

/** 关系优先级和实际匹配分分开展示，强证据不掩盖元数据冲突。 */
export function CandidateEvidence({ candidate, t }: { candidate: SongAssociationCandidate; t: TFunction }) {
  return <div className="grid gap-2 text-xs">
    {candidate.sharedIdentifier && <span>{candidate.sharedIdentifier.idKind}: {candidate.sharedIdentifier.value}</span>}
    <div className="flex flex-wrap gap-2">
      {(["title", "artist", "duration", "version"] as const).map((key) =>
        <Badge variant={candidate.gates[key] ? "outline" : "destructive"} key={key}>
          {t(`quickLyrics.details.manager.gates.${key}`)} · {t(`quickLyrics.details.manager.${candidate.gates[key] ? "passed" : "failed"}`)}
        </Badge>)}
    </div>
    <span>{t("quickLyrics.details.manager.weightedScore", { value: Math.round(candidate.score * 100) })}</span>
    <span>{t("quickLyrics.details.manager.weights", candidate.scoreWeights)}</span>
    {candidate.conflicts.length > 0 && <Alert variant="destructive"><AlertDescription>
      {candidate.conflicts.map((key) => <span key={key}>{t(`quickLyrics.details.manager.conflicts.${key}`)}</span>)}
    </AlertDescription></Alert>}
    {candidate.warnings.map((key) => <span className="text-muted-foreground" key={key}>{t(`quickLyrics.details.manager.warnings.${key}`)}</span>)}
    {candidate.lyricsContentSame
      ? <span>{t("quickLyrics.details.associationReasons.lyrics")}</span>
      : candidate.lyricsSimilarity !== null && <span>{t("quickLyrics.details.lyricsSimilarity", { value: Math.round(candidate.lyricsSimilarity * 100) })}</span>}
  </div>;
}
