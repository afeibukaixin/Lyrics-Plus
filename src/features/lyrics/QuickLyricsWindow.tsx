import { useTranslation } from "react-i18next";
import { useLyrics } from "./useLyrics";
import { usePlayback } from "../player/usePlayback";
import { QuickLyricsPreview } from "./quickLyrics/Preview";
import { QuickLyricsResults } from "./quickLyrics/Results";
import { QuickLyricsSearchForm } from "./quickLyrics/SearchForm";
import { useQuickLyricsSearch } from "./quickLyrics/useSearch";
import { useQuickLyricsSelection } from "./quickLyrics/useSelection";
import { cn } from "@/lib/utils";
import styles from "./QuickLyricsWindow.module.scss";

export default function QuickLyricsWindow() {
  const { t } = useTranslation();
  const playback = usePlayback();
  const lyrics = useLyrics(playback.snapshot, playback.positionMs, playback.active);
  const search = useQuickLyricsSearch(playback, lyrics);
  const selection = useQuickLyricsSelection(lyrics, t);
  const isLoading = lyrics.searching || lyrics.loadState === "loading";
  const emptyDescription = lyrics.error ?? (
    !lyrics.trackKey
      ? t("quickLyrics.noTrackHint")
      : lyrics.loadState === "missing"
        ? t("quickLyrics.autoSearchHint")
        : t("quickLyrics.manualSearchHint")
  );

  return (
    <main className={styles.shell}>
      <h1 className="sr-only">{t("quickLyrics.title")}</h1>
      <header className={styles.header} aria-label={t("quickLyrics.title")}>
        <QuickLyricsSearchForm
          artistInvalid={search.artistInvalid}
          durationInvalid={search.durationInvalid}
          formDisabled={search.formDisabled}
          onSearch={() => search.searchLyrics(selection.clearNotice)}
          onUpdateField={search.updateSearchField}
          searchForm={search.searchForm}
          searching={search.searching}
          titleInvalid={search.titleInvalid}
          t={t}
        />
      </header>

      <section className={styles.workspace}>
        <QuickLyricsResults
          applyingKey={selection.applyingKey}
          candidateDetailsOpen={selection.candidateDetailsOpen}
          emptyDescription={emptyDescription}
          isCurrent={selection.isCurrent}
          isLoading={isLoading}
          localResults={selection.localResults}
          onlineResults={selection.onlineResults}
          recommendedKey={selection.recommendedKey}
          resultsCount={lyrics.results.length}
          selectedKey={selection.selectedKey}
          selectAndApply={selection.selectAndApply}
          setCandidateDetailsOpen={selection.setCandidateDetailsOpen}
          t={t}
        />

        <QuickLyricsPreview selected={selection.selected} t={t} />
      </section>

      <div className={cn(styles.status, "text-xs text-muted-foreground")} aria-live="polite">{isLoading ? t("quickLyrics.searchingCandidates") : selection.applyingKey ? t("quickLyrics.applying") : selection.notice}</div>
    </main>
  );
}
