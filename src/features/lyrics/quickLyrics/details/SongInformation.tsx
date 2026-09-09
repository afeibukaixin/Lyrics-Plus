import { formatDuration } from "./helpers";
import type { SongManagerViewProps } from "./useSongManager";
import styles from "../../QuickLyricsWindow.module.scss";

export function SongInformation({ context, t, currentPlatformId }: SongManagerViewProps) {
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
            </section>);
}
