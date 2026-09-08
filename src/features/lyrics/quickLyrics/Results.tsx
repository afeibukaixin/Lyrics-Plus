import type { TFunction } from "i18next";
import { Music2 } from "lucide-react";
import { localizedSource } from "../../i18n/userText";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Item, ItemActions, ItemContent, ItemGroup } from "@/components/ui/item";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import type { QuickLyricsDisplayItem } from "./useSelection";
import { formatTime, resultKey } from "./utils";
import styles from "../QuickLyricsWindow.module.scss";

type QuickLyricsResultsProps = {
  candidateDetailsOpen: boolean;
  setCandidateDetailsOpen: (open: boolean) => void;
  currentItem: QuickLyricsDisplayItem | null;
  localResults: QuickLyricsDisplayItem[];
  onlineResults: QuickLyricsDisplayItem[];
  resultsCount: number;
  selectedKey: string | null;
  applyingKey: string | null;
  recommendedKey: string | null;
  selectAndApply: (item: QuickLyricsDisplayItem) => void | Promise<void>;
  isLoading: boolean;
  emptyDescription: string;
  t: TFunction;
};

export function QuickLyricsResults({
  candidateDetailsOpen,
  setCandidateDetailsOpen,
  currentItem,
  localResults,
  onlineResults,
  resultsCount,
  selectedKey,
  applyingKey,
  recommendedKey,
  selectAndApply,
  isLoading,
  emptyDescription,
  t,
}: QuickLyricsResultsProps) {
  const renderResult = (item: QuickLyricsDisplayItem) => {
    const { result, kind, showScore } = item;
    const key = resultKey(result);
    const current = kind === "current";
    const recommended = !current && key === recommendedKey;
    const applying = key === applyingKey;
    const capabilities = [
      result.synced && t("common.feature.synced"),
      result.hasWordTiming && t("common.feature.wordTiming"),
      result.hasTranslation && t("common.feature.hasTranslation"),
      result.hasRomanization && t("common.feature.romanization"),
    ].filter(Boolean) as string[];
    const songSummary = candidateDetailsOpen
      ? result.title
      : [result.title, result.artist].filter(Boolean).join(" · ");
    return (
      <Item
        size="xs"
        className={cn("h-auto justify-start", styles.resultItem)}
        render={<Button variant="ghost" type="button" aria-busy={applying} disabled={Boolean(applyingKey) || isLoading} />}
        key={key}
        data-current={current}
        data-selected={key === selectedKey}
        onClick={() => void selectAndApply(item)}
      >
        <ItemContent className={styles.resultContent}>
          <div className={styles.resultHeading}>
            <span className={styles.songSummary} title={songSummary}>{songSummary}</span>
            {(current || recommended) && <Badge variant={current ? "default" : "secondary"}>{current ? t("quickLyrics.current") : t("quickLyrics.recommended")}</Badge>}
          </div>
          <div className={styles.resultDetails}>
            <span className={cn(styles.sourceName, "text-xs text-muted-foreground")} title={localizedSource(result.source, t)}>{localizedSource(result.source, t)}</span>
            {candidateDetailsOpen && <div className={styles.resultMetadata}>
              <span>{t("quickLyrics.artistField")}: {result.artist || t("quickLyrics.unknown")}</span>
              <span>{t("quickLyrics.albumField")}: {result.album || t("quickLyrics.unknown")}</span>
              <span>{t("quickLyrics.durationField")}: {formatTime(result.durationMs)}</span>
            </div>}
            {capabilities.length > 0 && <div className={styles.capabilities}>{capabilities.map((capability) => <Badge variant="outline" key={capability}>{capability}</Badge>)}</div>}
          </div>
        </ItemContent>
        {showScore && <ItemActions>
          {applying
            ? <Spinner aria-label={t("quickLyrics.applying")} />
            : <Badge variant="outline">{Math.round(result.score * 100)}%</Badge>}
        </ItemActions>}
      </Item>
    );
  };

  return (
    <Card className={cn(styles.resultsPanel, "gap-0 py-0")}>
      <CardHeader className={styles.panelTitle}>
        <CardTitle>{t("quickLyrics.candidates")} <Badge variant="secondary">{resultsCount}</Badge></CardTitle>
        <CardAction className={styles.detailsControl}>
          <span>{t("quickLyrics.candidateDetails")}</span>
          <Switch
            aria-label={t("quickLyrics.candidateDetails")}
            checked={candidateDetailsOpen}
            onCheckedChange={setCandidateDetailsOpen}
          />
        </CardAction>
      </CardHeader>
      <CardContent className="min-h-0 px-0"><ScrollArea className="h-full min-h-0"><ItemGroup className={styles.resultList}>
        {currentItem && <>
          <div className={styles.groupLabel}>{t("quickLyrics.current")}</div>
          {renderResult(currentItem)}
        </>}
        {localResults.length > 0 && <div className={styles.groupLabel}>{t("quickLyrics.localCandidates")}</div>}
        {localResults.map(renderResult)}
        {onlineResults.length > 0 && <div className={styles.groupLabel}>{t("quickLyrics.onlineCandidates")}</div>}
        {onlineResults.map(renderResult)}
        {resultsCount === 0 && <Empty className={styles.empty}><EmptyHeader><EmptyMedia variant="icon">{isLoading ? <Spinner /> : <Music2 />}</EmptyMedia><EmptyTitle>{isLoading ? t("quickLyrics.searchingCandidates") : t("quickLyrics.noCandidates")}</EmptyTitle><EmptyDescription>{emptyDescription}</EmptyDescription></EmptyHeader></Empty>}
      </ItemGroup></ScrollArea></CardContent>
    </Card>
  );
}
