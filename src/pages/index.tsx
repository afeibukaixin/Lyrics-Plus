import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import { useTranslation } from "react-i18next";
import { findAlignedAuxiliaryLine, useLyrics } from "../features/lyrics/useLyrics";
import { useArtwork } from "../features/player/useArtwork";
import { usePlayback } from "../features/player/usePlayback";
import { api, messageOf } from "../shared/api";
import { localizedSource } from "../features/i18n/userText";
import styles from "./index.module.scss";
import appIcon from "../../src-tauri/icons/128x128.png";

function formatTime(value: number | null | undefined) {
  const seconds = Math.max(0, Math.round((value ?? 0) / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

export default function App() {
  const { t } = useTranslation();
  const playback = usePlayback();
  const artwork = useArtwork(playback.snapshot);
  const lyrics = useLyrics(playback.snapshot, playback.positionMs, true);
  const activeLineRef = useRef<HTMLButtonElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const [following, setFollowing] = useState(true);
  const [windowError, setWindowError] = useState<string | null>(null);

  useEffect(() => {
    document.documentElement.dataset.window = "main";
  }, []);

  useEffect(() => {
    if (following) activeLineRef.current?.scrollIntoView({ behavior: "smooth", block: "center" });
  }, [following, lyrics.activeIndex]);

  useEffect(() => {
    setFollowing(true);
  }, [lyrics.trackKey]);

  const currentTitle = playback.snapshot.title ?? t("home.waitingMusic");
  const currentArtist = playback.snapshot.artist ?? t("home.startPlayback");
  const duration = playback.snapshot.durationMs ?? 0;
  const progress = duration > 0 ? Math.min(100, (playback.positionMs / duration) * 100) : 0;
  const lines = lyrics.document?.tracks.original.lines ?? [];
  const translations = lyrics.document?.tracks.translation?.lines ?? [];

  const openQuickLyrics = async () => {
    setWindowError(null);
    try {
      await api.showQuickLyricsWindow();
    } catch (error) {
      setWindowError(messageOf(error));
    }
  };

  const seekToLine = (line: (typeof lines)[number], element: HTMLButtonElement) => {
    if (!playback.snapshot.canSeek) return;
    setFollowing(true);
    element.scrollIntoView({ behavior: "smooth", block: "center" });
    void playback.action("seek", Math.max(0, line.startMs - (lyrics.document?.offsetMs ?? 0)));
  };

  const pauseFollowingForKeyboard = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End", " "].includes(event.key)) setFollowing(false);
  };

  return (
    <main className={styles.appShell}>
      <header className={styles.header}>
        <div className={styles.brand}>
          <img className={styles.brandMark} src={appIcon} alt="" aria-hidden="true" />
          <div><strong>Lyrics Plus</strong><small>{t("home.brandSubtitle")}</small></div>
        </div>
        <nav className={styles.nav} aria-label={t("home.mainNavigation")}>
          <Link to="/library">{t("home.library")}</Link>
          <Link to="/settings">{t("home.settings")}</Link>
          <span className={styles.status} data-active={playback.snapshot.isPlaying}>
            <i />{playback.snapshot.isPlaying ? t("home.syncing") : t("home.waitingPlayback")}
          </span>
        </nav>
      </header>

      <div className={styles.workspace}>
        <section className={styles.nowPlaying}>
          <div className={styles.artwork} aria-hidden="true">
            <span>♪</span>
            {artwork.url && (
              <img
                alt=""
                data-loaded={artwork.loaded}
                src={artwork.url}
                onError={() => artwork.markFailed(artwork.url!)}
                onLoad={() => artwork.markLoaded(artwork.url!)}
              />
            )}
          </div>
          <div className={styles.trackMeta}>
            <span>{playback.snapshot.player === "apple_music" ? "APPLE MUSIC" : playback.snapshot.player === "spotify" ? "SPOTIFY" : t("home.nowPlaying").toUpperCase()}</span>
            <h1>{currentTitle}</h1>
            <p>{currentArtist}{playback.snapshot.album ? ` · ${playback.snapshot.album}` : ""}</p>
          </div>
          <div className={styles.transport}>
            <button aria-label={t("home.previous")} onClick={() => void playback.action("previous")}>↶</button>
            <button className={styles.playButton} aria-label={t("home.playPause")} onClick={() => void playback.action("play_pause")}>
              {playback.snapshot.isPlaying ? "Ⅱ" : "▶"}
            </button>
            <button aria-label={t("home.next")} onClick={() => void playback.action("next")}>↷</button>
          </div>
          <div className={styles.progressRow}>
            <span>{formatTime(playback.positionMs)}</span>
            <input
              aria-label={t("home.progress")}
              type="range"
              min={0}
              max={duration || 1}
              value={Math.min(playback.positionMs, duration || 1)}
              disabled={!playback.snapshot.canSeek}
              onChange={(event) => void playback.action("seek", Number(event.currentTarget.value))}
              style={{ "--progress": `${progress}%` } as React.CSSProperties}
            />
            <span>{formatTime(duration)}</span>
          </div>
          <div className={styles.sourceSummary}>
            <span>{t("home.lyricsSource")}</span>
            <strong>{lyrics.document ? localizedSource(lyrics.document.metadata.source, t) : lyrics.searching ? t("home.autoSearching") : t("home.notAssociated")}</strong>
          </div>
        </section>

        <section className={styles.lyricsPanel}>
        <div className={styles.panelHeader}>
          <div>
            <span>{t("home.syncedLyrics")}{lyrics.document?.tracks.translation ? ` · ${t("common.feature.translation")}` : ""}{lyrics.document?.tracks.romanization ? ` · ${t("common.feature.romanization")}` : ""}{lyrics.document?.tracks.original.lines.some((line) => line.words?.length) ? ` · ${t("common.feature.wordTiming")}` : ""}</span>
            <h2>{lyrics.document ? localizedSource(lyrics.document.metadata.source, t) : lyrics.searching ? t("home.searchingLyrics") : t("home.waitingLyrics")}</h2>
          </div>
          <div className={styles.lyricActions}>
            <button disabled={!lyrics.trackKey} onClick={() => void openQuickLyrics()}>
              {t("home.switchLyrics")}
            </button>
          </div>
        </div>

        <div
          className={styles.lyricsViewport}
          data-empty={lines.length === 0}
          onKeyDownCapture={pauseFollowingForKeyboard}
          onPointerDown={() => setFollowing(false)}
          onTouchStart={() => setFollowing(false)}
          onWheel={() => setFollowing(false)}
          ref={viewportRef}
          tabIndex={0}
        >
          {lines.length === 0 ? (
            <div className={styles.emptyState}>
              <span>{lyrics.searching ? "◌" : "♪"}</span>
              <strong>{lyrics.searching ? t("home.matching") : t("home.noSyncedLyrics")}</strong>
              <p>{lyrics.error ?? t("home.autoSearchHint")}</p>
            </div>
          ) : lines.map((line, index) => {
            const translation = findAlignedAuxiliaryLine(translations, line)?.text;
            return (
              <button
                type="button"
                className={styles.lyricLine}
                data-active={index === lyrics.activeIndex}
                data-past={index < lyrics.activeIndex}
                disabled={!playback.snapshot.canSeek}
                key={`${line.startMs}-${index}`}
                onClick={(event) => seekToLine(line, event.currentTarget)}
                ref={index === lyrics.activeIndex ? activeLineRef : undefined}
                title={playback.snapshot.canSeek ? t("home.seekTo", { time: formatTime(Math.max(0, line.startMs - (lyrics.document?.offsetMs ?? 0))) }) : t("home.seekUnsupported")}
              >
                <time>{formatTime(line.startMs)}</time>
                <div><p>{line.text || "…"}</p>{translation && <small>{translation}</small>}</div>
              </button>
            );
          })}
        </div>

        {!following && lyrics.activeIndex >= 0 && (
          <button className={styles.returnToCurrent} onClick={() => setFollowing(true)}>{t("home.returnCurrent")}</button>
        )}
        </section>
      </div>
      {windowError && <div className={styles.toast} onClick={() => setWindowError(null)}>{windowError}</div>}
    </main>
  );
}
