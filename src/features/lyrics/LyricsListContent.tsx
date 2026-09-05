import type { TFunction } from "i18next";
import { ArrowDownToLine, Music2, Search } from "lucide-react";
import type { RefObject } from "react";
import type { LyricsLine, LyricsRuntimeStatus, ListLyricsLineOrder } from "../../shared/types";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import styles from "./LyricsListWindow.module.scss";

const SHORT_SECTION_BREAK_MAX_DURATION_MS = 2_000;
const SHORT_SECTION_BREAK_SCALE = 0.2;

export type LyricsListAuxiliaryLine = {
  translation: LyricsLine | null;
  romanization: LyricsLine | null;
};

type LyricsListContentProps = {
  t: TFunction;
  lines: LyricsLine[];
  auxiliary: LyricsListAuxiliaryLine[];
  lineOrder: ListLyricsLineOrder;
  onLineClick?: (line: LyricsLine) => void;
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

/** Returns the visual scale for an empty segment, or null when it is not between lyrics. */
function sectionBreakScale(lines: LyricsLine[], index: number): number | null {
  const line = lines[index];
  if (!line?.text.trim() || index === 0 || !lines[index - 1]?.text.trim()) return null;
  const nextLine = lines.slice(index + 1).find((candidate) => candidate.text.trim());
  if (!nextLine) return null;
  const durationMs = nextLine.startMs - line.startMs;
  return durationMs >= 0 && durationMs <= SHORT_SECTION_BREAK_MAX_DURATION_MS
    ? SHORT_SECTION_BREAK_SCALE
    : 1;
}

export function LyricsListContent({
  t,
  lines,
  auxiliary,
  lineOrder,
  onLineClick,
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
              if (!line.text.trim()) {
                const scale = sectionBreakScale(lines, index);
                return scale === null
                  ? null
                  : (
                    <div
                      className={styles.sectionBreak}
                      data-compact={scale === SHORT_SECTION_BREAK_SCALE || undefined}
                      aria-hidden="true"
                      key={`${line.startMs}:${index}`}
                    />
                  );
              }
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
                  onClick={onLineClick ? () => onLineClick(line) : undefined}
                >
                  {lineOrder.map((kind) => {
                    if (kind === "original") return <p key={kind}>{line.text}</p>;
                    const supportingLine = supporting?.[kind];
                    return supportingLine
                      ? <small data-kind={kind} key={kind}>{supportingLine.text}</small>
                      : null;
                  })}
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
