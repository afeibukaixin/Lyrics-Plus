import { Info, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import type { QuickLyricsDetailsProps } from "./details/helpers";
import { useSongManager } from "./details/useSongManager";
import { formatSeconds } from "./details/SearchTimeline";
import { SongInformation } from "./details/SongInformation";
import { SharedLyrics } from "./details/SharedLyrics";
import { SearchDetails } from "./details/SearchDetails";
import styles from "../QuickLyricsWindow.module.scss";

export function QuickLyricsDetails(props: QuickLyricsDetailsProps) {
  const manager = useSongManager(props);
  const view = { ...props, ...manager };
  const { open, onOpenChange, playback, t } = props;
  const { context, error, setError, trackKey, trace, totalElapsedMs } = manager;
  return <>
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className={styles.detailsSheet}>
        <SheetHeader className={styles.detailsHeader}>
          <SheetTitle className={styles.detailsHeaderTitle}><Info data-icon="inline-start" />{t("quickLyrics.details.title")}</SheetTitle>
          <SheetDescription>{context?.recording.title ?? playback.title ?? t("quickLyrics.details.noTrack")}</SheetDescription>
        </SheetHeader>
        <ScrollArea className={styles.detailsScroll}>
          {error && <div className={styles.detailsError} role="alert">{error}<Button variant="ghost" size="icon-sm" onClick={() => setError(null)} aria-label={t("common.actions.close")}><X /></Button></div>}
          {!trackKey || !context ? <div className={styles.detailsEmpty}>{t("quickLyrics.details.noContext")}</div> : <div className={styles.detailsBody}>
            <SongInformation {...view} />
            <SharedLyrics {...view} />
            <SearchDetails {...view} />
          </div>}
        </ScrollArea>
        <SheetFooter className={styles.detailsFooter}>
          {trace?.finishedAt && <span>{t("quickLyrics.details.lastSearch", { time: new Date(trace.finishedAt * 1000).toLocaleString() })}</span>}
          {totalElapsedMs !== null && <span>{t("quickLyrics.details.totalDuration", { time: formatSeconds(totalElapsedMs) })}</span>}
        </SheetFooter>
      </SheetContent>
    </Sheet>
  </>;
}
