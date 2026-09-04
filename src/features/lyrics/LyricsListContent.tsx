import type { TFunction } from "i18next";
import { ArrowDownToLine, Music2, Search } from "lucide-react";
import type { RefObject } from "react";
import type { LyricsLine, LyricsRuntimeStatus } from "../../shared/types";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import styles from "./LyricsListWindow.module.scss";

export type LyricsListAuxiliaryLine = {
  translation: LyricsLine | null;
  romanization: LyricsLine | null;
};

type LyricsListContentProps = {
  t: TFunction;
  lines: LyricsLine[];
  auxiliary: LyricsListAuxiliaryLine[];
  activeIndex: number;
  activeRef: RefObject<HTMLDivElement | null>;
  following: boolean;
  onPauseFollowing: () => void;
  onResumeFollowing: () => void;
  status: LyricsRuntimeStatus;
  error: string | null;
  canChooseLyrics: boolean;
  onChooseLyrics: () => void;
};

export function LyricsListContent({
  t,
  lines,
  auxiliary,
  activeIndex,
  activeRef,
  following,
  onPauseFollowing,
  onResumeFollowing,
  status,
  error,
  canChooseLyrics,
  onChooseLyrics,
}: LyricsListContentProps) {
  if (lines.length > 0) {
    return (
      <div className={styles.workspace}>
        <ScrollArea className={styles.scroller} onWheel={onPauseFollowing} onPointerDown={onPauseFollowing}>
          <div className={styles.lines} role="list" aria-label={t("lyricsList.lyrics")}>
            {lines.map((line, index) => {
              const active = index === activeIndex;
              const supporting = auxiliary[index];
              return (
                <div
                  className={cn(styles.line, active && styles.activeLine)}
                  data-active={active || undefined}
                  key={`${line.startMs}:${index}`}
                  ref={active ? activeRef : undefined}
                  role="listitem"
                  aria-current={active ? "true" : undefined}
                >
                  <p>{line.text || "\u00a0"}</p>
                  {supporting?.translation && <small data-kind="translation">{supporting.translation.text}</small>}
                  {supporting?.romanization && <small data-kind="romanization">{supporting.romanization.text}</small>}
                </div>
              );
            })}
          </div>
        </ScrollArea>
        {!following && (
          <Button className={styles.followButton} variant="secondary" size="sm" onClick={onResumeFollowing}>
            <ArrowDownToLine data-icon="inline-start" />{t("lyricsList.returnCurrent")}
          </Button>
        )}
      </div>
    );
  }

  return (
    <Empty className={styles.empty}>
      <EmptyHeader>
        <EmptyMedia variant="icon"><Music2 /></EmptyMedia>
        <EmptyTitle>{status === "loading" ? t("lyricsList.loading") : t("lyricsList.empty")}</EmptyTitle>
        <EmptyDescription>{error ?? t("lyricsList.emptyHint")}</EmptyDescription>
      </EmptyHeader>
      {canChooseLyrics && (
        <Button variant="outline" onClick={onChooseLyrics}>
          <Search data-icon="inline-start" />{t("lyricsList.chooseLyrics")}
        </Button>
      )}
    </Empty>
  );
}
