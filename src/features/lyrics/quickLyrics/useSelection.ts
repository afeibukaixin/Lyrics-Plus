import { useEffect, useMemo, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { toast } from "sonner";
import { useLyrics } from "../useLyrics";
import { localizedSource } from "../../i18n/userText";
import type { LyricsDocument, LyricsSearchResult } from "../../../shared/types";
import { resultKey } from "./utils";

type LyricsController = ReturnType<typeof useLyrics>;

export type QuickLyricsDisplayItem = {
  kind: "current" | "candidate";
  result: LyricsSearchResult;
  showScore: boolean;
};

function resultFromCurrentDocument(document: LyricsDocument): LyricsSearchResult {
  const originalLines = document.tracks.original.lines;
  return {
    // This is a UI-only identity. It must never be passed to a save command.
    id: "current-binding",
    providerId: "current-binding",
    title: document.metadata.title ?? "",
    artist: document.metadata.artist ?? "",
    album: document.metadata.album,
    durationMs: null,
    source: document.metadata.source,
    synced: originalLines.length > 0,
    hasTranslation: Boolean(document.tracks.translation?.lines.length),
    hasWordTiming: originalLines.some((line) => Boolean(line.words?.length)),
    hasRomanization: Boolean(document.tracks.romanization?.lines.length),
    score: 0,
    lyrics: document.raw,
  };
}

function matchesCurrentDocument(document: LyricsDocument, result: LyricsSearchResult) {
  if (result.providerId === "local") {
    return result.lyrics.trim() === document.raw.trim();
  }
  return document.metadata.source === result.source
    && result.lyrics.trim() === document.raw.trim();
}

export function useQuickLyricsSelection(lyrics: LyricsController, t: TFunction) {
  const applying = useRef(false);
  const [candidateDetailsOpen, setCandidateDetailsOpen] = useState(false);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [applyingKey, setApplyingKey] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const clearNotice = () => setNotice(null);

  useEffect(() => {
    setSelectedKey(null);
    setNotice(null);
  }, [lyrics.trackKey]);

  useEffect(() => {
    if (!notice) return;
    toast.success(notice);
    setNotice(null);
  }, [notice]);

  useEffect(() => {
    if (lyrics.error && lyrics.results.length > 0) toast.error(lyrics.error);
  }, [lyrics.error, lyrics.results.length]);

  const currentResult = useMemo<LyricsSearchResult | null>(() => {
    const document = lyrics.document;
    if (!document) return null;
    return lyrics.results.find((result) => matchesCurrentDocument(document, result)) ?? null;
  }, [lyrics.document, lyrics.results]);
  const matchedCurrentKey = currentResult ? resultKey(currentResult) : null;
  const currentItem = useMemo<QuickLyricsDisplayItem | null>(() => {
    if (!lyrics.document || currentResult) return null;
    return {
      kind: "current",
      result: resultFromCurrentDocument(lyrics.document),
      showScore: false,
    };
  }, [currentResult, lyrics.document]);
  const currentKey = currentResult
    ? matchedCurrentKey
    : currentItem
      ? resultKey(currentItem.result)
      : null;
  const candidateItems = useMemo<QuickLyricsDisplayItem[]>(
    () => lyrics.results.map((result) => ({
      kind: resultKey(result) === matchedCurrentKey ? "current" : "candidate",
      result,
      showScore: true,
    })),
    [matchedCurrentKey, lyrics.results],
  );
  const displayItems = useMemo(
    () => currentItem ? [currentItem, ...candidateItems] : candidateItems,
    [candidateItems, currentItem],
  );
  const selected = useMemo(
    () => displayItems.find((item) => resultKey(item.result) === selectedKey)?.result ?? null,
    [displayItems, selectedKey],
  );
  const localResults = useMemo(
    () => candidateItems.filter((item) => item.result.providerId === "local"),
    [candidateItems],
  );
  const onlineResults = useMemo(
    () => candidateItems.filter((item) => item.result.providerId !== "local"),
    [candidateItems],
  );
  const autoRecommendedKey = lyrics.autoApplyCandidate
    ? `${lyrics.autoApplyCandidate.providerId}:${lyrics.autoApplyCandidate.id}`
    : null;
  const recommendedKey = autoRecommendedKey === currentKey
    ? null
    : autoRecommendedKey && candidateItems.some((item) => resultKey(item.result) === autoRecommendedKey)
    ? autoRecommendedKey
    : candidateItems[0]
      ? resultKey(candidateItems[0].result)
      : null;

  useEffect(() => {
    setSelectedKey((current) => {
      // 自动应用完成后，优先把预览切到实际落库的候选，避免继续展示旧的预览项。
      if (currentKey && currentKey === autoRecommendedKey && displayItems.some((item) => resultKey(item.result) === currentKey)) {
        return currentKey;
      }
      if (current && displayItems.some((item) => resultKey(item.result) === current)) return current;
      if (currentKey && displayItems.some((item) => resultKey(item.result) === currentKey)) return currentKey;
      return displayItems[0] ? resultKey(displayItems[0].result) : null;
    });
  }, [autoRecommendedKey, currentKey, displayItems]);

  const selectAndApply = async (item: QuickLyricsDisplayItem) => {
    const { result } = item;
    const key = resultKey(result);
    setSelectedKey(key);
    setNotice(null);
    if (item.kind === "current" || applying.current) return;
    applying.current = true;
    setApplyingKey(key);
    try {
      const saved = await lyrics.applyResult(result);
      if (saved) {
        setNotice(t("quickLyrics.switched", { source: localizedSource(result.source, t) }));
      } else if (currentKey) {
        setSelectedKey(currentKey);
      }
    } finally {
      applying.current = false;
      setApplyingKey(null);
    }
  };

  return {
    candidateDetailsOpen,
    setCandidateDetailsOpen,
    clearNotice,
    selectedKey,
    applyingKey,
    notice,
    selected,
    currentItem,
    localResults,
    onlineResults,
    resultsCount: displayItems.length,
    recommendedKey,
    selectAndApply,
  };
}
