import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import { useTranslation } from "react-i18next";
import { findAlignedAuxiliaryLine, useLyrics } from "../features/lyrics/useLyrics";
import { useArtwork } from "../features/player/useArtwork";
import { usePlayback } from "../features/player/usePlayback";
import { api, messageOf } from "../shared/api";
import { localizedSource } from "../features/i18n/userText";
import { UiIcon } from "../components/UiIcon";
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
  const lyricFeatures = lyrics.document
    ? [
        lyrics.document.tracks.translation ? t("common.feature.translation") : null,
        lyrics.document.tracks.romanization ? t("common.feature.romanization") : null,
        lyrics.document.tracks.original.lines.some((line) => line.words?.length) ? t("common.feature.wordTiming") : null,
      ].filter((feature): feature is string => Boolean(feature))
    : [];
  const lyricHeaderMeta = lyrics.document
    ? [t("home.source", { source: localizedSource(lyrics.document.metadata.source, t) }), ...lyricFeatures].join(" · ")
    : lyrics.searching
      ? t("home.searchingLyrics")
      : t("home.waitingLyrics");

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
          <strong>Lyrics Plus</strong>
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
            <span className={styles.artworkPlaceholder} data-loading={artwork.loading}>
              <UiIcon className={styles.artworkPlaceholderNote} name="musicNote" />
              {artwork.loading && <UiIcon className={styles.artworkSpinner} name="spinner" spin />}
            </span>
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
            <span>{(playback.snapshot.player === "system" ? t("home.systemMedia") : playback.snapshot.player === "apple_music" ? "Apple Music" : playback.snapshot.player === "spotify" ? "Spotify" : t("home.nowPlaying")).toUpperCase()}</span>
            <h1>{currentTitle}</h1>
            <p>{currentArtist}{playback.snapshot.album ? ` · ${playback.snapshot.album}` : ""}</p>
            {artwork.source === "itunes" && artwork.sourceLink && (
              <a className={styles.artworkSource} href={artwork.sourceLink} target="_blank" rel="noreferrer">{t("home.artworkCourtesy")}</a>
            )}
          </div>
          <div className={styles.transport}>
            <button aria-label={t("home.previous")} onClick={() => void playback.action("previous")}><UiIcon name="skipBackFill" /></button>
            <button className={styles.playButton} aria-label={t("home.playPause")} onClick={() => void playback.action("play_pause")}>
              {playback.snapshot.isPlaying ? <UiIcon name="pauseFill" /> : <UiIcon name="playFill" />}
            </button>
            <button aria-label={t("home.next")} onClick={() => void playback.action("next")}><UiIcon name="skipForwardFill" /></button>
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
        </section>

        <section className={styles.lyricsPanel}>
        <div className={styles.panelHeader}>
          <div>
            <h2>{t("home.lyrics")}</h2>
            <span>{lyricHeaderMeta}</span>
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
        tabIndex={0}
        >
          {lines.length === 0 ? (
            <div className={styles.emptyState}>
              {lyrics.searching ? <UiIcon name="spinner" spin /> : <UiIcon name="musicNotes" />}
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
