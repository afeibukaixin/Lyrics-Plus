import { useEffect, useMemo, useRef, useState } from "react";
import { useLyrics } from "./useLyrics";
import { usePlayback } from "../player/usePlayback";
import type { LyricsSearchResult } from "../../shared/types";
import styles from "./QuickLyricsWindow.module.scss";

function formatTime(value: number | null | undefined) {
  if (value == null) return null;
  const seconds = Math.max(0, Math.round(value / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function resultKey(result: LyricsSearchResult) {
  return `${result.providerId}:${result.id}`;
}

export default function QuickLyricsWindow() {
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
      if (saved) setNotice(`已切换为 ${result.source} 的歌词`);
    } finally {
      applying.current = false;
      setApplyingKey(null);
    }
  };

  const currentTitle = playback.snapshot.title ?? "没有正在播放的歌曲";
  const currentArtist = playback.snapshot.artist ?? "播放歌曲后可快速搜索并切换歌词";

  return (
    <main className={styles.shell}>
      <header className={styles.header}>
        <div className={styles.heading}>
          <span>QUICK SWITCH</span>
          <h1>快速切换歌词</h1>
          <p><strong>{currentTitle}</strong><i>·</i>{currentArtist}</p>
        </div>
        <button
          className={styles.refreshButton}
          disabled={!lyrics.trackKey || lyrics.searching}
          onClick={() => void refreshCurrentTrack()}
        >
          {lyrics.searching ? "搜索中…" : "重新搜索当前歌曲"}
        </button>
      </header>

      <form className={styles.search} onSubmit={(event) => { event.preventDefault(); void searchByTitle(); }}>
        <span aria-hidden="true">⌕</span>
        <input
          aria-label="按歌名搜索歌词"
          autoComplete="off"
          disabled={!lyrics.trackKey || lyrics.searching}
          placeholder="输入歌名，歌手、专辑和时长沿用当前歌曲"
          value={searchTitle}
          onChange={(event) => setSearchTitle(event.currentTarget.value)}
        />
        <button disabled={!lyrics.trackKey || lyrics.searching || !searchTitle.trim()} type="submit">搜索</button>
      </form>

      <section className={styles.workspace}>
        <div className={styles.resultsPanel}>
          <div className={styles.panelTitle}>
            <div><span>CANDIDATES</span><h2>候选歌词</h2></div>
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
                  <span className={styles.rank}>{current ? "当前" : index === 0 ? "推荐" : index + 1}</span>
                  <span className={styles.resultMeta}>
                    <strong>{result.title}</strong>
                    <small>{result.artist}{result.album ? ` · ${result.album}` : ""}{formatTime(result.durationMs) ? ` · ${formatTime(result.durationMs)}` : ""}</small>
                    <i>{result.source} · {result.synced ? "同步" : "纯文本"}{result.hasTranslation ? " · 翻译" : ""}{result.hasWordTiming ? " · 逐字" : ""}{result.hasRomanization ? " · 音译" : ""}</i>
                  </span>
                  <b>{Math.round(result.score * 100)}%</b>
                </button>
              );
            })}
            {lyrics.results.length === 0 && (
              <div className={styles.empty}>
                <span>{lyrics.searching ? "◌" : "♪"}</span>
                <strong>{lyrics.searching ? "正在搜索候选歌词" : "暂无候选歌词"}</strong>
                <p>{lyrics.error ?? "播放歌曲后会自动搜索所有已启用来源。"}</p>
              </div>
            )}
          </div>
        </div>

        <aside className={styles.previewPanel}>
          <div className={styles.panelTitle}>
            <div><span>RAW LRC</span><h2>{selected?.title ?? "歌词预览"}</h2></div>
            {selected && <em>{selected.source}</em>}
          </div>
          {selected ? (
            <>
              <pre>{selected.lyrics}</pre>
              <footer className={styles.previewFooter}>
                <span>{applyingKey === selectedKey ? "正在应用这份歌词…" : notice ?? (isCurrent(selected) ? "当前歌曲正在使用这份歌词" : "单击左侧候选即可预览并切换")}</span>
              </footer>
            </>
          ) : (
            <div className={styles.empty}><span>≋</span><strong>选择左侧候选歌词</strong><p>这里会显示未经处理的原始 LRC 内容。</p></div>
          )}
        </aside>
      </section>

      {lyrics.error && lyrics.results.length > 0 && <div className={styles.toast}>{lyrics.error}</div>}
    </main>
  );
}
