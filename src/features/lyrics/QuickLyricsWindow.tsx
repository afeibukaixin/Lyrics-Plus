import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { localizedSource } from "../i18n/userText";
import { useLyrics } from "./useLyrics";
import { usePlayback } from "../player/usePlayback";
import type { LyricsSearchResult } from "../../shared/types";
import styles from "./QuickLyricsWindow.module.scss";
import { FileText, LoaderCircle, Music2, Search } from "lucide-react";

function formatTime(value: number | null | undefined) {
  if (value == null) return null;
  const seconds = Math.max(0, Math.round(value / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function resultKey(result: LyricsSearchResult) {
  return `${result.providerId}:${result.id}`;
}

export default function QuickLyricsWindow() {
  const { t } = useTranslation();
  const playback = usePlayback();
  const lyrics = useLyrics(playback.snapshot, playback.positionMs, false);
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
    const timer = setTimeout(() => setNotice(null), 2800);
    return () => clearTimeout(timer);
  }, [notice]);

  const selected = useMemo(
    () => lyrics.results.find((result) => resultKey(result) === selectedKey) ?? null,
    [lyrics.results, selectedKey],
  );

  const isCurrent = (result: LyricsSearchResult) => (
    result.lyrics.trim() === lyrics.document?.raw.trim()
  );

  const searchByTitle = async () => {
    const title = searchTitle.trim();
    const artist = playback.snapshot.artist?.trim();
    if (!title || !artist || !lyrics.trackKey || lyrics.searching) return;
    setNotice(null);
    await lyrics.searchWith({
      title,
      artist,
      album: playback.snapshot.album ?? null,
      durationMs: playback.snapshot.durationMs ?? null,
    });
  };

  const refreshCurrentTrack = async () => {
    setSearchTitle(playback.snapshot.title ?? "");
    setNotice(null);
    await lyrics.search();
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
      <header className={styles.header}>
        <div className={styles.heading}>
          <span>{t("quickLyrics.eyebrow").toUpperCase()}</span>
          <h1>{t("quickLyrics.title")}</h1>
          <p><strong>{currentTitle}</strong><i>·</i>{currentArtist}</p>
        </div>
        <button
          className={styles.refreshButton}
          disabled={!lyrics.trackKey || lyrics.searching}
          onClick={() => void refreshCurrentTrack()}
        >
          {lyrics.searching ? t("common.actions.searching") : t("quickLyrics.refresh")}
        </button>
      </header>

      <form className={styles.search} onSubmit={(event) => { event.preventDefault(); void searchByTitle(); }}>
        <Search />
        <input
          aria-label={t("quickLyrics.searchLabel")}
          autoComplete="off"
          disabled={!lyrics.trackKey || lyrics.searching}
          placeholder={t("quickLyrics.searchPlaceholder")}
          value={searchTitle}
          onChange={(event) => setSearchTitle(event.currentTarget.value)}
        />
        <button disabled={!lyrics.trackKey || lyrics.searching || !searchTitle.trim()} type="submit">{t("common.actions.search")}</button>
      </form>

      <section className={styles.workspace}>
        <div className={styles.resultsPanel}>
          <div className={styles.panelTitle}>
            <div><span>{t("quickLyrics.candidates").toUpperCase()}</span><h2>{t("quickLyrics.candidates")}</h2></div>
            <b>{lyrics.results.length}</b>
          </div>
          <div className={styles.resultList}>
            {lyrics.results.map((result, index) => {
              const key = resultKey(result);
              const current = isCurrent(result);
              return (
                <button
                  type="button"
                  key={key}
                  data-current={current}
                  data-selected={key === selectedKey}
                  disabled={Boolean(applyingKey)}
                  onClick={() => void selectAndApply(result)}
                >
                  <span className={styles.rank}>{current ? t("quickLyrics.current") : index === 0 ? t("quickLyrics.recommended") : index + 1}</span>
                  <span className={styles.resultMeta}>
                    <strong>{result.title}</strong>
                    <small>{result.artist}{result.album ? ` · ${result.album}` : ""}{formatTime(result.durationMs) ? ` · ${formatTime(result.durationMs)}` : ""}</small>
                    <i>{localizedSource(result.source, t)} · {result.synced ? t("common.feature.synced") : t("common.feature.plainText")} · {result.hasTranslation ? t("common.feature.hasTranslation") : t("quickLyrics.sourceNoTranslation")}{result.hasWordTiming ? ` · ${t("common.feature.wordTiming")}` : ""}{result.hasRomanization ? ` · ${t("common.feature.romanization")}` : ""}</i>
                  </span>
                  <b>{Math.round(result.score * 100)}%</b>
                </button>
              );
            })}
            {lyrics.results.length === 0 && (
              <div className={styles.empty}>
                {lyrics.searching ? <LoaderCircle className="animate-spin" /> : <Music2 />}
                <strong>{lyrics.searching ? t("quickLyrics.searchingCandidates") : t("quickLyrics.noCandidates")}</strong>
                <p>{lyrics.error ?? t("quickLyrics.autoSearchHint")}</p>
              </div>
            )}
          </div>
        </div>

        <aside className={styles.previewPanel}>
          <div className={styles.panelTitle}>
            <div><span>{t("library.rawLrc").toUpperCase()}</span><h2>{selected?.title ?? t("quickLyrics.preview")}</h2></div>
            {selected && <em>{localizedSource(selected.source, t)}</em>}
          </div>
          {selected ? (
            <>
              <pre>{selected.lyrics}</pre>
              <footer className={styles.previewFooter}>
                <span>{applyingKey === selectedKey ? t("quickLyrics.applying") : notice ?? (isCurrent(selected) ? t("quickLyrics.inUse") : t("quickLyrics.clickToApply"))}</span>
                {!selected.hasTranslation && <span>{t("quickLyrics.translationUnavailable")}</span>}
              </footer>
            </>
          ) : (
            <div className={styles.empty}><FileText /><strong>{t("quickLyrics.selectCandidate")}</strong><p>{t("quickLyrics.rawHint")}</p></div>
          )}
        </aside>
      </section>

      {lyrics.error && lyrics.results.length > 0 && <div className={styles.toast}>{lyrics.error}</div>}
    </main>
  );
}
