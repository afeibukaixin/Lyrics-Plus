import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { FileText, Music2, Search } from "lucide-react";
import { localizedSource } from "../i18n/userText";
import { useLyrics } from "./useLyrics";
import { usePlayback } from "../player/usePlayback";
import type { LyricsSearchResult } from "../../shared/types";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Item, ItemActions, ItemContent, ItemGroup } from "@/components/ui/item";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";
import styles from "./QuickLyricsWindow.module.scss";

function formatTime(value: number | null | undefined) {
  if (value == null) return null;
  const seconds = Math.max(0, Math.round(value / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function resultKey(result: LyricsSearchResult) {
  return `${result.providerId}:${result.id}`;
}

function normalized(value: string | null | undefined) {
  return value?.trim().toLocaleLowerCase().replace(/\s+/g, " ") ?? "";
}

export default function QuickLyricsWindow() {
  const { t } = useTranslation();
  const playback = usePlayback();
  const lyrics = useLyrics(playback.snapshot, playback.positionMs, playback.active);
  const searchedTrack = useRef<string | null>(null);
  const applying = useRef(false);
  const [searchTitle, setSearchTitle] = useState("");
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [applyingKey, setApplyingKey] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    setSearchTitle(playback.snapshot.title ?? "");
    setSelectedKey(null);
    setNotice(null);
    if (!lyrics.trackKey || !playback.snapshot.title || !playback.snapshot.artist) return;
    if (searchedTrack.current === lyrics.trackKey) return;
    searchedTrack.current = lyrics.trackKey;
    void lyrics.search();
  }, [lyrics.trackKey, playback.snapshot.artist, playback.snapshot.title]);

  useEffect(() => {
    if (lyrics.results.length === 0) {
      setSelectedKey(null);
      return;
    }
    setSelectedKey((current) => current && lyrics.results.some((result) => resultKey(result) === current)
      ? current
      : resultKey(lyrics.results[0]));
  }, [lyrics.results]);

  useEffect(() => {
    if (!notice) return;
    toast.success(notice);
    setNotice(null);
  }, [notice]);

  useEffect(() => {
    if (lyrics.error && lyrics.results.length > 0) toast.error(lyrics.error);
  }, [lyrics.error, lyrics.results.length]);

  const selected = useMemo(
    () => lyrics.results.find((result) => resultKey(result) === selectedKey) ?? null,
    [lyrics.results, selectedKey],
  );
  const localResults = useMemo(
    () => lyrics.results.filter((result) => result.providerId === "local"),
    [lyrics.results],
  );
  const onlineResults = useMemo(
    () => lyrics.results.filter((result) => result.providerId !== "local"),
    [lyrics.results],
  );
  const recommendedKey = lyrics.results[0] ? resultKey(lyrics.results[0]) : null;

  const isCurrent = (result: LyricsSearchResult) => {
    const document = lyrics.document;
    if (!document) return false;
    if (result.providerId === "local") {
      return result.lyrics.trim() === document.raw.trim();
    }
    return document.metadata.source === result.source
      && result.lyrics.trim() === document.raw.trim();
  };

  const searchLyrics = async () => {
    const title = searchTitle.trim();
    const artist = playback.snapshot.artist?.trim();
    if (!title || !artist || !lyrics.trackKey || lyrics.searching) return;
    setNotice(null);

    if (normalized(title) === normalized(playback.snapshot.title)) {
      await lyrics.search(true);
      return;
    }

    await lyrics.searchWith({
      title,
      artist,
      album: playback.snapshot.album ?? null,
      durationMs: playback.snapshot.durationMs ?? null,
    }, true);
  };

  const selectAndApply = async (result: LyricsSearchResult) => {
    const key = resultKey(result);
    setSelectedKey(key);
    setNotice(null);
    if (isCurrent(result) || applying.current) return;
    applying.current = true;
    setApplyingKey(key);
    try {
      const saved = await lyrics.applyResult(result);
      if (saved) setNotice(t("quickLyrics.switched", { source: localizedSource(result.source, t) }));
    } finally {
      applying.current = false;
      setApplyingKey(null);
    }
  };

  const currentTitle = playback.snapshot.title ?? t("quickLyrics.noTrack");
  const currentArtist = playback.snapshot.artist ?? t("quickLyrics.noTrackHint");

  return (
    <main className={styles.shell}>
      <h1 className="sr-only">{t("quickLyrics.title")}</h1>
      <header className={styles.header} aria-label={t("quickLyrics.title")}>
        <div className={styles.track}>
          <div><strong className="font-medium">{currentTitle}</strong><span className="text-xs text-muted-foreground">{currentArtist}</span></div>
        </div>
        <form className={styles.search} onSubmit={(event) => { event.preventDefault(); void searchLyrics(); }}>
          <InputGroup>
            <InputGroupAddon><Search aria-hidden="true" /></InputGroupAddon>
            <InputGroupInput aria-label={t("quickLyrics.searchLabel")} autoComplete="off" disabled={!lyrics.trackKey || lyrics.searching} placeholder={t("quickLyrics.searchPlaceholder")} value={searchTitle} onChange={(event) => setSearchTitle(event.currentTarget.value)} />
            <InputGroupAddon align="inline-end"><Button size="sm" disabled={!lyrics.trackKey || lyrics.searching || !searchTitle.trim()} type="submit">{lyrics.searching ? t("common.actions.searching") : t("common.actions.search")}</Button></InputGroupAddon>
          </InputGroup>
        </form>
      </header>

      <section className={styles.workspace}>
        <Card className={cn(styles.resultsPanel, "gap-0 py-0")}>
          <CardHeader className={styles.panelTitle}>
            <CardTitle>{t("quickLyrics.candidates")} <Badge variant="secondary">{lyrics.results.length}</Badge></CardTitle>
          </CardHeader>
          <CardContent className="min-h-0 px-0"><ScrollArea className="h-full min-h-0"><ItemGroup className={styles.resultList}>
            {localResults.length > 0 && <div className={styles.groupLabel}>{t("quickLyrics.localCandidates")}</div>}
            {localResults.map((result) => {
              const key = resultKey(result);
              const current = isCurrent(result);
              const recommended = !current && key === recommendedKey;
              const capabilities = [
                result.synced && t("common.feature.synced"),
                result.hasWordTiming && t("common.feature.wordTiming"),
                result.hasTranslation && t("common.feature.hasTranslation"),
                result.hasRomanization && t("common.feature.romanization"),
              ].filter(Boolean) as string[];
              const songSummary = [
                result.title,
                result.artist,
                formatTime(result.durationMs),
              ].filter(Boolean).join(" · ");
              return (
                <Item size="xs" className={cn("h-auto justify-start", styles.resultItem)} render={<Button variant="ghost" type="button" disabled={Boolean(applyingKey)} />} key={key} data-current={current} data-selected={key === selectedKey} onClick={() => void selectAndApply(result)}>
                  <ItemContent className={styles.resultContent}>
                    <div className={styles.resultHeading}><span className={styles.songSummary} title={songSummary}>{songSummary}</span>{(current || recommended) && <Badge variant={current ? "default" : "secondary"}>{current ? t("quickLyrics.current") : t("quickLyrics.recommended")}</Badge>}</div>
                    <div className={styles.resultDetails}>
                      <span className={cn(styles.sourceName, "text-xs text-muted-foreground")} title={localizedSource(result.source, t)}>{localizedSource(result.source, t)}</span>
                      {capabilities.length > 0 && <div className={styles.capabilities}>{capabilities.map((capability) => <Badge variant="outline" key={capability}>{capability}</Badge>)}</div>}
                    </div>
                  </ItemContent>
                  <ItemActions><Badge variant="outline">{Math.round(result.score * 100)}%</Badge></ItemActions>
                </Item>
              );
            })}
            {onlineResults.length > 0 && <div className={styles.groupLabel}>{t("quickLyrics.onlineCandidates")}</div>}
            {onlineResults.map((result) => {
              const key = resultKey(result);
              const current = isCurrent(result);
              const recommended = !current && key === recommendedKey;
              const capabilities = [
                result.synced && t("common.feature.synced"),
                result.hasWordTiming && t("common.feature.wordTiming"),
                result.hasTranslation && t("common.feature.hasTranslation"),
                result.hasRomanization && t("common.feature.romanization"),
              ].filter(Boolean) as string[];
              const songSummary = [
                result.title,
                result.artist,
                formatTime(result.durationMs),
              ].filter(Boolean).join(" · ");
              return (
                <Item size="xs" className={cn("h-auto justify-start", styles.resultItem)} render={<Button variant="ghost" type="button" disabled={Boolean(applyingKey)} />} key={key} data-current={current} data-selected={key === selectedKey} onClick={() => void selectAndApply(result)}>
                  <ItemContent className={styles.resultContent}>
                    <div className={styles.resultHeading}><span className={styles.songSummary} title={songSummary}>{songSummary}</span>{(current || recommended) && <Badge variant={current ? "default" : "secondary"}>{current ? t("quickLyrics.current") : t("quickLyrics.recommended")}</Badge>}</div>
                    <div className={styles.resultDetails}>
                      <span className={cn(styles.sourceName, "text-xs text-muted-foreground")} title={localizedSource(result.source, t)}>{localizedSource(result.source, t)}</span>
                      {capabilities.length > 0 && <div className={styles.capabilities}>{capabilities.map((capability) => <Badge variant="outline" key={capability}>{capability}</Badge>)}</div>}
                    </div>
                  </ItemContent>
                  <ItemActions><Badge variant="outline">{Math.round(result.score * 100)}%</Badge></ItemActions>
                </Item>
              );
            })}
            {lyrics.results.length === 0 && <Empty className={styles.empty}><EmptyHeader><EmptyMedia variant="icon">{lyrics.searching ? <Spinner /> : <Music2 />}</EmptyMedia><EmptyTitle>{lyrics.searching ? t("quickLyrics.searchingCandidates") : t("quickLyrics.noCandidates")}</EmptyTitle><EmptyDescription>{lyrics.error ?? t("quickLyrics.autoSearchHint")}</EmptyDescription></EmptyHeader></Empty>}
          </ItemGroup></ScrollArea></CardContent>
        </Card>

        <Card className={cn(styles.previewPanel, "gap-0 py-0")} role="complementary">
          <CardHeader className={styles.panelTitle}>
            <CardTitle>{t("quickLyrics.preview")}</CardTitle>
            {selected && <CardAction><Badge variant="secondary">{localizedSource(selected.source, t)}</Badge></CardAction>}
          </CardHeader>
          <CardContent className="min-h-0 px-0">{selected ? <ScrollArea className="h-full min-h-0"><pre className="font-mono text-sm leading-relaxed">{selected.lyrics}</pre></ScrollArea> : <Empty className={styles.empty}><EmptyHeader><EmptyMedia variant="icon"><FileText /></EmptyMedia><EmptyTitle>{t("quickLyrics.selectCandidate")}</EmptyTitle><EmptyDescription>{t("quickLyrics.rawHint")}</EmptyDescription></EmptyHeader></Empty>}</CardContent>
        </Card>
      </section>

      <div className={cn(styles.status, "text-xs text-muted-foreground")} aria-live="polite">{lyrics.searching ? t("quickLyrics.searchingCandidates") : applyingKey ? t("quickLyrics.applying") : notice}</div>
    </main>
  );
}
