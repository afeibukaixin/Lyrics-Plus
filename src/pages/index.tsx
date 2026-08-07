import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import { findAlignedAuxiliaryLine, useLyrics } from "../features/lyrics/useLyrics";
import { useArtwork } from "../features/player/useArtwork";
import { usePlayback } from "../features/player/usePlayback";
import { api, messageOf } from "../shared/api";
import styles from "./index.module.scss";
import appIcon from "../../src-tauri/icons/128x128.png";

function formatTime(value: number | null | undefined) {
  const seconds = Math.max(0, Math.round((value ?? 0) / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

export default function App() {
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

  const currentTitle = playback.snapshot.title ?? "等待音乐开始";
  const currentArtist = playback.snapshot.artist ?? "打开 Apple Music 或 Spotify 播放一首歌";
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
          <div><strong>Lyrics Plus</strong><small>桌面同步歌词</small></div>
        </div>
        <nav className={styles.nav} aria-label="主导航">
          <Link to="/library">歌词库</Link>
          <Link to="/settings">设置</Link>
          <span className={styles.status} data-active={playback.snapshot.isPlaying}>
            <i />{playback.snapshot.isPlaying ? "同步中" : "等待播放"}
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
            <span>{playback.snapshot.player === "apple_music" ? "APPLE MUSIC" : playback.snapshot.player === "spotify" ? "SPOTIFY" : "NOW PLAYING"}</span>
            <h1>{currentTitle}</h1>
            <p>{currentArtist}{playback.snapshot.album ? ` · ${playback.snapshot.album}` : ""}</p>
          </div>
          <div className={styles.transport}>
            <button aria-label="上一首" onClick={() => void playback.action("previous")}>↶</button>
            <button className={styles.playButton} aria-label="播放或暂停" onClick={() => void playback.action("play_pause")}>
              {playback.snapshot.isPlaying ? "Ⅱ" : "▶"}
            </button>
            <button aria-label="下一首" onClick={() => void playback.action("next")}>↷</button>
          </div>
          <div className={styles.progressRow}>
            <span>{formatTime(playback.positionMs)}</span>
            <input
              aria-label="播放进度"
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
            <span>歌词来源</span>
            <strong>{lyrics.document?.metadata.source ?? (lyrics.searching ? "正在自动搜索…" : "尚未关联")}</strong>
          </div>
        </section>

        <section className={styles.lyricsPanel}>
        <div className={styles.panelHeader}>
          <div>
            <span>同步歌词{lyrics.document?.tracks.translation ? " · 翻译" : ""}{lyrics.document?.tracks.romanization ? " · 音译" : ""}{lyrics.document?.tracks.original.lines.some((line) => line.words?.length) ? " · 逐字" : ""}</span>
            <h2>{lyrics.document ? lyrics.document.metadata.source : lyrics.searching ? "正在自动搜索歌词…" : "等待歌词"}</h2>
          </div>
          <div className={styles.lyricActions}>
            <button disabled={!lyrics.trackKey} onClick={() => void openQuickLyrics()}>
              切换歌词 ↗
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
              <strong>{lyrics.searching ? "正在从多个歌词源匹配" : "还没有可显示的同步歌词"}</strong>
              <p>{lyrics.error ?? "播放歌曲后会自动搜索，达到设置相似度的同步歌词将直接采用。"}</p>
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
                title={playback.snapshot.canSeek ? `跳转到 ${formatTime(Math.max(0, line.startMs - (lyrics.document?.offsetMs ?? 0)))}` : "当前播放器不支持跳转"}
              >
                <time>{formatTime(line.startMs)}</time>
                <div><p>{line.text || "…"}</p>{translation && <small>{translation}</small>}</div>
              </button>
            );
          })}
        </div>

        {!following && lyrics.activeIndex >= 0 && (
          <button className={styles.returnToCurrent} onClick={() => setFollowing(true)}>回到当前歌词</button>
        )}
        </section>
      </div>
      {windowError && <div className={styles.toast} onClick={() => setWindowError(null)}>{windowError}</div>}
    </main>
  );
}
