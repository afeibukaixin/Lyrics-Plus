import { Unlink, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { assetPath } from "./helpers";
import type { SongManagerViewProps } from "./useSongManager";
import styles from "../../QuickLyricsWindow.module.scss";

export function SharedLyrics({ context, t, currentAsset, assetCapabilities, currentBinding, currentOffset, busyAction, lyrics, refresh, runAction, importInput, handleImport }: SongManagerViewProps) {
  if (!context) return null;
  return (
            <section className={styles.detailsSection}>
              <h2>{t("quickLyrics.details.currentLyrics")}</h2>
              {currentAsset ? <div className={styles.detailsGrid}>
                <span>{t("quickLyrics.details.asset")}</span><strong>{currentAsset.sourceName}</strong>
                <span>{t("quickLyrics.details.sourceKind")}</span><strong>{t(`quickLyrics.details.sourceKinds.${currentAsset.sourceKind}`, { defaultValue: currentAsset.sourceKind })}</strong>
                <span>{t("quickLyrics.details.format")}</span><strong>{currentAsset.originalFormat} · {currentAsset.language}</strong>
                <span>{t("quickLyrics.details.capabilities.title")}</span><div className={styles.capabilities}>{assetCapabilities.map((capability) => <Badge variant="outline" key={capability}>{capability}</Badge>)}</div>
                <span>{t("quickLyrics.details.path")}</span><strong>{assetPath(context) ?? t("quickLyrics.details.inMemory")}</strong>
                <span>{t("quickLyrics.details.binding")}</span><strong>{context.platformOverride?.assetId != null ? t("quickLyrics.details.platformOverride") : currentBinding?.selectionSource === "automatic" ? t("quickLyrics.details.automaticBinding") : currentBinding?.selectionSource === "migration" ? t("quickLyrics.details.migrationBinding") : currentBinding?.isDefault ? t("quickLyrics.details.defaultLyrics") : t("quickLyrics.details.manualLyrics")}</strong>
                <span>{t("quickLyrics.details.currentPlatformOffset", { defaultValue: t("quickLyrics.details.offset") })}</span><strong>{currentOffset}ms</strong>
              </div> : <>
                <p className={styles.detailsHint}>{t("quickLyrics.details.noLyrics")}</p>
                <p className={styles.detailsHint}>{t("quickLyrics.details.currentPlatformOffset", { defaultValue: t("quickLyrics.details.offset") })}: {currentOffset}ms</p>
              </>}
              <div className={styles.detailsActions}>
                <Button variant="secondary" size="sm" disabled={busyAction !== null} onClick={() => void lyrics.changeOffset(-100).then(refresh)}>-100ms</Button>
                <Button variant="secondary" size="sm" disabled={busyAction !== null} onClick={() => void lyrics.changeOffset(100).then(refresh)}>+100ms</Button>
                <Button variant="ghost" size="sm" disabled={busyAction !== null || !lyrics.document} onClick={() => void runAction("unlink", lyrics.remove)}><Unlink />{t("quickLyrics.details.unlink")}</Button>
                <div className={styles.detailsFileButton}><Button variant="ghost" size="sm" disabled={busyAction !== null} onClick={() => importInput.current?.click()}><Upload />{t("quickLyrics.details.importLyrics")}</Button><input ref={importInput} hidden type="file" accept=".lrc,.lyricsfile.yaml,text/plain,application/yaml,text/yaml" onChange={(event) => void handleImport(event)} /></div>
              </div>
            </section>
);
}
