import { Scissors } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { formatDuration, observationDetails } from "./helpers";
import type { SongManagerViewProps } from "./useSongManager";
import styles from "../../QuickLyricsWindow.module.scss";

export function AssociatedPlatforms({ context, t, observations, busyAction, openSplitDialog }: SongManagerViewProps) {
  if (!context) return null;
  return (<section className={styles.detailsSection}>              <div className={styles.detailsList}>
                <strong>{t("quickLyrics.details.otherPlatforms")}</strong>
                {new Set(observations.map((item) => item.platform)).size < observations.length && <Alert variant="destructive"><AlertDescription>{t("quickLyrics.details.manager.conflicts.platform")}</AlertDescription></Alert>}
                {observations.map((observation) => {
                  const identifiers = context.recording.externalIdentifiers.filter(
                    (identifier) => identifier.namespace === observation.platform
                      && (observations.filter((item) => item.platform === observation.platform).length === 1
                        || identifier.value === observation.trackKey.slice(observation.platform.length + 1)),
                  );
                  const isCurrentPlatform = observation.platform === context.platform
                    && observation.trackKey === context.observation.trackKey;
                  return <div className={styles.detailsObservation} key={observation.observationId}>
                    <div className={styles.detailsMergeCandidateInfo}>
                      <div className={styles.detailsMergeCandidateTitle}>
                        <Badge variant="outline">{observation.platform}</Badge>
                        {isCurrentPlatform && <Badge>{t("quickLyrics.details.currentPlatform")}</Badge>}
                        <span>{observationDetails(observation)}</span>
                      </div>
                      <small>{observation.rawAlbum ?? context.recording.album ?? t("quickLyrics.unknown")} · {formatDuration(observation.durationMs ?? context.recording.durationMs)}</small>
                    {identifiers.length > 0
                      ? <small>{identifiers.map((identifier) => `${identifier.namespace}/${identifier.idKind}: ${identifier.value} · ${t("quickLyrics.details.manager.confidence", { value: identifier.confidence })} · ${identifier.confirmed ? t("quickLyrics.details.confirmed") : t("quickLyrics.details.unconfirmed")}`).join(" · ")}</small>
                        : <small>{t("quickLyrics.details.platformId")}：{observation.trackKey} · {t("quickLyrics.details.unconfirmed")}</small>}
                    </div>
                    {observations.length > 1 && <Button variant="ghost" size="sm" disabled={busyAction !== null} onClick={() => openSplitDialog(observation)}><Scissors />{t("quickLyrics.details.detachPlatformTrack")}</Button>}
                  </div>;
                })}
              </div>

</section>);
}
