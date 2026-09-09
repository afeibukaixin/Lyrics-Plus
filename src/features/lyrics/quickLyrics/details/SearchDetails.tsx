import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { resultTrace } from "./helpers";
import { SearchTimeline, formatSeconds, scoreReasons, localizedCandidateReason } from "./SearchTimeline";
import type { SongManagerViewProps } from "./useSongManager";
import styles from "../../QuickLyricsWindow.module.scss";

export function SearchDetails({ context, t, busyAction, lyrics, handleSearch, trace, progress, stageHistory, stageLabel, timingAvailable, candidateGroups, selectCandidate }: SongManagerViewProps) {
  if (!context) return null;
  return (
            <section className={styles.detailsSection}>
              <div className={styles.detailsSectionHeading}><h2>{t("quickLyrics.details.searchTrace")}</h2><Button variant="secondary" size="sm" disabled={busyAction !== null || lyrics.searching} onClick={() => void handleSearch()}><RefreshCw />{t("quickLyrics.details.searchAgain")}</Button></div>
              <p className={styles.detailsHint}>{t("quickLyrics.details.searchTraceHint")}</p>
              <h3 className={styles.detailsDiagnosticsTitle}>{t("quickLyrics.details.diagnostics")}</h3>
              <SearchTimeline trace={trace} progress={progress} stageHistory={stageHistory} t={t} />
              <p className={styles.detailsHint}>{stageLabel}</p>
              {trace?.providers.length ? <div className={styles.detailsProviders}>{trace.providers.map((provider) => <div key={provider.providerId}><span>{provider.providerId}</span><span>{provider.status} · {provider.candidateCount} · {timingAvailable ? formatSeconds(provider.elapsedMs) : "—"}</span></div>)}</div> : null}
              {candidateGroups.length > 0 && <div className={styles.detailsCandidates}>{candidateGroups.map(([version, results]) => <div className={styles.detailsCandidateGroup} key={version}><h3>{version}</h3>{results.map((result) => {
                const candidate = resultTrace(result, trace);
                const actionKey = `${result.providerId}:${result.id}`;
                return <div className={styles.detailsCandidate} key={actionKey}>
                  <div className={styles.detailsCandidateContent}>
                    <strong>{result.title}</strong>
                    <span>{result.providerId} · {Math.round(result.score * 100)}%</span>
                    {scoreReasons(candidate, t) && <small className={styles.detailsScoreReasons}>{scoreReasons(candidate, t)}</small>}
                    {candidate?.rejectionReason && <small>{t("quickLyrics.details.rejectionReasonLabel")}: {localizedCandidateReason(candidate.rejectionReason, "rejection", t)}</small>}
                    {candidate?.selectionReason && <small>{t("quickLyrics.details.selectionReasonLabel")}: {localizedCandidateReason(candidate.selectionReason, "selection", t)}</small>}
                  </div>
                  <div className={styles.detailsCandidateActions}><Button variant="secondary" size="sm" disabled={busyAction !== null} onClick={() => void selectCandidate(result)}>{t("quickLyrics.details.useAsDefault")}</Button>{busyAction?.startsWith(actionKey) && <span className="sr-only">{t("quickLyrics.applying")}</span>}</div>
                </div>;
              })}</div>)}</div>}
            </section>);
}
