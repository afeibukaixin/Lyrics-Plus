import { Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { formatDuration } from "./helpers";
import type { SongManagerViewProps } from "./useSongManager";
import styles from "../../QuickLyricsWindow.module.scss";

export function SongInformation({ context, t, currentPlatformId, aliasDrafts, aliasErrors, busyAction, handleAliasRemove, handleAliasAdd, handleAliasDraftChange }: SongManagerViewProps) {
  if (!context) return null;
  return (            <section className={styles.detailsSection}>
              <h2>{t("quickLyrics.details.recording")}</h2>
              <div className={styles.detailsGrid}>
                <span>{t("quickLyrics.titleField")}</span><strong>{context.recording.title}</strong>
                <span>{t("quickLyrics.artistField")}</span><strong>{context.recording.artistCredits.map((credit) => credit.rawName).join(" / ") || context.observation.rawArtists.join(" / ")}</strong>
                <span>{t("quickLyrics.albumField")}</span><strong>{context.recording.album || t("quickLyrics.unknown")}</strong>
                <span>{t("quickLyrics.durationField")}</span><strong>{formatDuration(context.recording.durationMs)}</strong>
                <span>{t("quickLyrics.details.version")}</span><strong>{context.recording.versionTags.join(" / ") || t("quickLyrics.details.unspecified")}</strong>
              </div>
              <p className={styles.detailsHint}>{t("quickLyrics.details.rawCredits")}: {context.observation.rawArtists.join(" / ")}<br />{t("quickLyrics.details.platformId")}: {currentPlatformId}</p>
              <div className={styles.detailsAliasEditor}>
                <h2>{t("quickLyrics.details.manager.matchingEvidence")}</h2>
                <div className={styles.detailsAliasEditorHeading}>
                  <strong>{t("quickLyrics.details.artistAliasEditor.title")}</strong>
                  <p className={styles.detailsHint}>{t("quickLyrics.details.artistAliasEditor.hint")}</p>
                </div>
                {context.recording.artistCredits.length > 0
                  ? context.recording.artistCredits.map((credit) => {
                    const artistId = credit.artistId;
                    const key = artistId === null ? `raw:${credit.creditOrder}` : String(artistId);
                    const draft = artistId === null ? "" : aliasDrafts[key] ?? "";
                    const aliasError = artistId === null ? null : aliasErrors[key];
                    return <div className={styles.detailsAliasArtist} key={`${credit.creditOrder}:${key}`}>
                      <strong className={styles.detailsAliasArtistName}>{credit.rawName}</strong>
                      {credit.confirmedAliases.length > 0
                        ? <div className={styles.detailsAliasList}>{credit.confirmedAliases.map((alias) => <div className={styles.detailsAliasItem} key={`${key}:${alias}`}><span>{alias}</span><Button type="button" variant="ghost" size="icon-sm" disabled={busyAction !== null} onClick={() => handleAliasRemove(credit, alias)} aria-label={t("quickLyrics.details.artistAliasEditor.remove", { alias })} title={t("quickLyrics.details.artistAliasEditor.remove", { alias })}><Trash2 /></Button></div>)}</div>
                        : <p className={styles.detailsAliasEmpty}>{t("quickLyrics.details.artistAliasEditor.empty")}</p>}
                      {artistId !== null
                        ? <form className={styles.detailsAliasAdd} onSubmit={(event) => handleAliasAdd(event, credit)}>
                          <Input
                            aria-invalid={Boolean(aliasError)}
                            aria-label={t("quickLyrics.details.artistAliasEditor.inputLabel", { artist: credit.rawName })}
                            autoComplete="off"
                            disabled={busyAction !== null}
                            placeholder={t("quickLyrics.details.artistAliasEditor.placeholder")}
                            value={draft}
                            onChange={(event) => handleAliasDraftChange(artistId, event.currentTarget.value)}
                          />
                          <Button type="submit" variant="secondary" size="sm" disabled={busyAction !== null}><Plus />{t("quickLyrics.details.artistAliasEditor.add")}</Button>
                        </form>
                        : <p className={styles.detailsAliasUnavailable}>{t("quickLyrics.details.artistAliasEditor.unavailable")}</p>}
                      {aliasError && <p className={styles.detailsFieldError} role="alert">{aliasError}</p>}
                    </div>;
                  })
                  : <p className={styles.detailsAliasEmpty}>{t("quickLyrics.details.artistAliasEditor.empty")}</p>}
              </div>

            </section>);
}
