import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { FileText, Music2 } from "lucide-react";
import { localizedSource } from "../i18n/userText";
import { useLyrics } from "./useLyrics";
import { usePlayback } from "../player/usePlayback";
import { isTauriRuntime } from "../../shared/api";
import { createTauriListenerCleanup, QUICK_LYRICS_REFRESH_EVENT } from "../../shared/tauriEvent";
import type { LyricsSearchResult } from "../../shared/types";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { InputGroup, InputGroupInput, InputGroupText } from "@/components/ui/input-group";
import { Input } from "@/components/ui/input";
import { Item, ItemActions, ItemContent, ItemGroup } from "@/components/ui/item";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
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

type DurationParts = {
  durationMinutes: string;
  durationSeconds: string;
};

type SearchFormState = {
  title: string;
  artist: string;
  album: string;
} & DurationParts;

function formatDurationParts(value: number | null | undefined): DurationParts {
  if (value == null) return { durationMinutes: "", durationSeconds: "" };
  const totalSeconds = Math.max(0, Math.round(value / 1000));
  return {
    durationMinutes: String(Math.floor(totalSeconds / 60)),
    durationSeconds: String(totalSeconds % 60).padStart(2, "0"),
  };
}

function parseDuration(minutesValue: string, secondsValue: string): number | null | undefined {
  const minutesText = minutesValue.trim();
  const secondsText = secondsValue.trim();
  if (!minutesText && !secondsText) return null;
  if ((minutesText && !/^\d+$/.test(minutesText)) || (secondsText && !/^\d+$/.test(secondsText))) return undefined;

  const minutes = minutesText ? Number(minutesText) : 0;
  const seconds = secondsText ? Number(secondsText) : 0;
  if (!Number.isSafeInteger(minutes) || !Number.isSafeInteger(seconds) || seconds > 59) return undefined;
  const durationMs = (minutes * 60 + seconds) * 1000;
  return Number.isSafeInteger(durationMs) ? durationMs : undefined;
}

export default function QuickLyricsWindow() {
  const { t } = useTranslation();
  const playback = usePlayback();
  const lyrics = useLyrics(playback.snapshot, playback.positionMs, playback.active);
  const searchedTrack = useRef<string | null>(null);
  const searchRef = useRef(lyrics.search);
  searchRef.current = lyrics.search;
  const searchStateRef = useRef({
    trackKey: lyrics.trackKey,
    title: playback.snapshot.title,
    artist: playback.snapshot.artist,
    searching: lyrics.searching,
  });
  searchStateRef.current = {
    trackKey: lyrics.trackKey,
    title: playback.snapshot.title,
    artist: playback.snapshot.artist,
    searching: lyrics.searching,
  };
  const applying = useRef(false);
  const [searchForm, setSearchForm] = useState<SearchFormState>({
    title: "",
    artist: "",
    album: "",
    durationMinutes: "",
    durationSeconds: "",
  });
  const [formSubmitted, setFormSubmitted] = useState(false);
  const [candidateDetailsOpen, setCandidateDetailsOpen] = useState(false);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [applyingKey, setApplyingKey] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const updateSearchField = (field: keyof SearchFormState, value: string) => {
    setSearchForm((current) => ({ ...current, [field]: value }));
  };

  useEffect(() => {
    const duration = formatDurationParts(playback.snapshot.durationMs);
    setSearchForm({
      title: playback.snapshot.title ?? "",
      artist: playback.snapshot.artist ?? "",
      album: playback.snapshot.album ?? "",
      ...duration,
    });
    setFormSubmitted(false);
    setSelectedKey(null);
    setNotice(null);
  }, [lyrics.trackKey]);

  useEffect(() => {
    if (
      !lyrics.trackKey
      || !playback.snapshot.title
      || !playback.snapshot.artist
      || (lyrics.loadState !== "ready" && lyrics.loadState !== "missing")
    ) return;
    if (searchedTrack.current === lyrics.trackKey) return;
    searchedTrack.current = lyrics.trackKey;
    void lyrics.search("refresh");
  }, [lyrics.loadState, lyrics.trackKey, playback.snapshot.artist, playback.snapshot.title]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    return createTauriListenerCleanup(listen(QUICK_LYRICS_REFRESH_EVENT, () => {
      const current = searchStateRef.current;
      if (!current.trackKey || !current.title || !current.artist || current.searching) return;
      void searchRef.current("refresh");
    }));
  }, []);

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
  const parsedDuration = parseDuration(searchForm.durationMinutes, searchForm.durationSeconds);
  const titleInvalid = formSubmitted && !searchForm.title.trim();
  const artistInvalid = formSubmitted && !searchForm.artist.trim();
  const durationInvalid = formSubmitted && parsedDuration === undefined;

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
    setFormSubmitted(true);
    const title = searchForm.title.trim();
    const artist = searchForm.artist.trim();
    const durationMs = parseDuration(searchForm.durationMinutes, searchForm.durationSeconds);
    if (!title || !artist || durationMs === undefined || !lyrics.trackKey || lyrics.searching) return;
    setNotice(null);
    await lyrics.searchWith({
      title,
      artist,
      album: searchForm.album.trim() || null,
      durationMs,
    }, "manual");
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

  const isLoading = lyrics.searching || lyrics.loadState === "loading";
  const formDisabled = !lyrics.trackKey || lyrics.searching;
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
        <form className={styles.search} onSubmit={(event) => { event.preventDefault(); void searchLyrics(); }}>
          <FieldGroup className={styles.formGrid}>
            <Field data-invalid={titleInvalid}>
              <FieldLabel htmlFor="quick-lyrics-title">{t("quickLyrics.titleField")}</FieldLabel>
              <Input
                aria-invalid={titleInvalid}
                autoComplete="off"
                disabled={formDisabled}
                id="quick-lyrics-title"
                placeholder={t("quickLyrics.titlePlaceholder")}
                value={searchForm.title}
                onChange={(event) => updateSearchField("title", event.currentTarget.value)}
              />
              <FieldError>{titleInvalid ? t("quickLyrics.titleRequired") : null}</FieldError>
            </Field>
            <Field data-invalid={artistInvalid}>
              <FieldLabel htmlFor="quick-lyrics-artist">{t("quickLyrics.artistField")}</FieldLabel>
              <Input
                aria-invalid={artistInvalid}
                autoComplete="off"
                disabled={formDisabled}
                id="quick-lyrics-artist"
                placeholder={t("quickLyrics.artistPlaceholder")}
                value={searchForm.artist}
                onChange={(event) => updateSearchField("artist", event.currentTarget.value)}
              />
              <FieldError>{artistInvalid ? t("quickLyrics.artistRequired") : null}</FieldError>
            </Field>
            <Field>
              <FieldLabel htmlFor="quick-lyrics-album">{t("quickLyrics.albumField")}</FieldLabel>
              <Input
                autoComplete="off"
                disabled={formDisabled}
                id="quick-lyrics-album"
                placeholder={t("quickLyrics.albumPlaceholder")}
                value={searchForm.album}
                onChange={(event) => updateSearchField("album", event.currentTarget.value)}
              />
            </Field>
            <Field data-invalid={durationInvalid}>
              <FieldLabel htmlFor="quick-lyrics-duration-minutes">{t("quickLyrics.durationField")}</FieldLabel>
              <InputGroup className={styles.durationInput}>
                <InputGroupInput
                  aria-invalid={durationInvalid}
                  aria-label={t("quickLyrics.durationMinutes")}
                  autoComplete="off"
                  className={styles.durationSegment}
                  disabled={formDisabled}
                  id="quick-lyrics-duration-minutes"
                  inputMode="numeric"
                  placeholder={t("quickLyrics.durationMinutesPlaceholder")}
                  value={searchForm.durationMinutes}
                  onChange={(event) => updateSearchField("durationMinutes", event.currentTarget.value)}
                />
                <InputGroupText aria-hidden="true" className={styles.durationSeparator}>:</InputGroupText>
                <InputGroupInput
                  aria-invalid={durationInvalid}
                  aria-label={t("quickLyrics.durationSeconds")}
                  autoComplete="off"
                  className={styles.durationSegment}
                  disabled={formDisabled}
                  id="quick-lyrics-duration-seconds"
                  inputMode="numeric"
                  maxLength={2}
                  placeholder={t("quickLyrics.durationSecondsPlaceholder")}
                  value={searchForm.durationSeconds}
                  onChange={(event) => updateSearchField("durationSeconds", event.currentTarget.value)}
                />
              </InputGroup>
              <FieldError>{durationInvalid ? t("quickLyrics.durationInvalid") : null}</FieldError>
            </Field>
            <Button className={styles.searchButton} disabled={formDisabled} type="submit">{lyrics.searching ? t("common.actions.searching") : t("common.actions.search")}</Button>
          </FieldGroup>
          <FieldDescription className={styles.searchHint}>{t("quickLyrics.searchRuleHint")} {t("quickLyrics.durationFuzzyHint")}</FieldDescription>
        </form>
      </header>

      <section className={styles.workspace}>
        <Card className={cn(styles.resultsPanel, "gap-0 py-0")}>
          <CardHeader className={styles.panelTitle}>
            <CardTitle>{t("quickLyrics.candidates")} <Badge variant="secondary">{lyrics.results.length}</Badge></CardTitle>
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
              const songSummary = candidateDetailsOpen
                ? result.title
                : [result.title, result.artist].filter(Boolean).join(" · ");
              return (
                <Item size="xs" className={cn("h-auto justify-start", styles.resultItem)} render={<Button variant="ghost" type="button" disabled={Boolean(applyingKey)} />} key={key} data-current={current} data-selected={key === selectedKey} onClick={() => void selectAndApply(result)}>
                  <ItemContent className={styles.resultContent}>
                    <div className={styles.resultHeading}><span className={styles.songSummary} title={songSummary}>{songSummary}</span>{(current || recommended) && <Badge variant={current ? "default" : "secondary"}>{current ? t("quickLyrics.current") : t("quickLyrics.recommended")}</Badge>}</div>
                    <div className={styles.resultDetails}>
                      <span className={cn(styles.sourceName, "text-xs text-muted-foreground")} title={localizedSource(result.source, t)}>{localizedSource(result.source, t)}</span>
                      {candidateDetailsOpen && <div className={styles.resultMetadata}>
                        <span>{t("quickLyrics.artistField")}: {result.artist || t("quickLyrics.unknown")}</span>
                        <span>{t("quickLyrics.albumField")}: {result.album || t("quickLyrics.unknown")}</span>
                        <span>{t("quickLyrics.durationField")}: {formatTime(result.durationMs) ?? t("quickLyrics.unknown")}</span>
                      </div>}
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
              const songSummary = candidateDetailsOpen
                ? result.title
                : [result.title, result.artist].filter(Boolean).join(" · ");
              return (
                <Item size="xs" className={cn("h-auto justify-start", styles.resultItem)} render={<Button variant="ghost" type="button" disabled={Boolean(applyingKey)} />} key={key} data-current={current} data-selected={key === selectedKey} onClick={() => void selectAndApply(result)}>
                  <ItemContent className={styles.resultContent}>
                    <div className={styles.resultHeading}><span className={styles.songSummary} title={songSummary}>{songSummary}</span>{(current || recommended) && <Badge variant={current ? "default" : "secondary"}>{current ? t("quickLyrics.current") : t("quickLyrics.recommended")}</Badge>}</div>
                    <div className={styles.resultDetails}>
                      <span className={cn(styles.sourceName, "text-xs text-muted-foreground")} title={localizedSource(result.source, t)}>{localizedSource(result.source, t)}</span>
                      {candidateDetailsOpen && <div className={styles.resultMetadata}>
                        <span>{t("quickLyrics.artistField")}: {result.artist || t("quickLyrics.unknown")}</span>
                        <span>{t("quickLyrics.albumField")}: {result.album || t("quickLyrics.unknown")}</span>
                        <span>{t("quickLyrics.durationField")}: {formatTime(result.durationMs) ?? t("quickLyrics.unknown")}</span>
                      </div>}
                      {capabilities.length > 0 && <div className={styles.capabilities}>{capabilities.map((capability) => <Badge variant="outline" key={capability}>{capability}</Badge>)}</div>}
                    </div>
                  </ItemContent>
                  <ItemActions><Badge variant="outline">{Math.round(result.score * 100)}%</Badge></ItemActions>
                </Item>
              );
            })}
            {lyrics.results.length === 0 && <Empty className={styles.empty}><EmptyHeader><EmptyMedia variant="icon">{isLoading ? <Spinner /> : <Music2 />}</EmptyMedia><EmptyTitle>{isLoading ? t("quickLyrics.searchingCandidates") : t("quickLyrics.noCandidates")}</EmptyTitle><EmptyDescription>{emptyDescription}</EmptyDescription></EmptyHeader></Empty>}
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

      <div className={cn(styles.status, "text-xs text-muted-foreground")} aria-live="polite">{isLoading ? t("quickLyrics.searchingCandidates") : applyingKey ? t("quickLyrics.applying") : notice}</div>
    </main>
  );
}
