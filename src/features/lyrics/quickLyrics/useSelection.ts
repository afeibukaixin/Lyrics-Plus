import { useEffect, useMemo, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { toast } from "sonner";
import { useLyrics } from "../useLyrics";
import { localizedSource } from "../../i18n/userText";
import type { LyricsSearchResult } from "../../../shared/types";
import { resultKey } from "./utils";

type LyricsController = ReturnType<typeof useLyrics>;

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

  return {
    candidateDetailsOpen,
    setCandidateDetailsOpen,
    clearNotice,
    selectedKey,
    applyingKey,
    notice,
    selected,
    localResults,
    onlineResults,
    recommendedKey,
    isCurrent,
    selectAndApply,
  };
}
