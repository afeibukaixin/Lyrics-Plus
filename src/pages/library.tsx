import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Link } from "react-router";
import { api, isTauriRuntime, messageOf } from "../shared/api";
import type { LibraryEntry, LibraryOverview, LibraryPreview } from "../shared/types";
import styles from "./library.module.scss";

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function formatDuration(value: number | null) {
  if (value == null) return "—";
  const seconds = Math.round(value / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

export default function Library() {
  const [overview, setOverview] = useState<LibraryOverview | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<LibraryPreview | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const applyOverview = (value: LibraryOverview) => {
    setOverview(value);
    if (selectedPath && !value.entries.some((entry) => entry.path === selectedPath)) {
      setSelectedPath(null);
      setPreview(null);
    }
  };

  const load = async () => {
    const value = await api.getLibraryOverview();
    applyOverview(value);
    return value;
  };

  useEffect(() => {
    document.documentElement.dataset.window = "main";
    if (!isTauriRuntime()) return;
    setError(null);
    void load().catch((cause) => setError(messageOf(cause)));
  }, []);

  const selectEntry = async (entry: LibraryEntry) => {
    setSelectedPath(entry.path);
    setBusy("preview");
    setError(null);
    try {
      setPreview(await api.previewLibraryEntry(entry.path));
    } catch (cause) {
      setError(messageOf(cause));
    } finally {
      setBusy(null);
    }
  };

  const rescan = async () => {
    setBusy("rescan");
    setError(null);
    try {
      applyOverview(await api.rescanLyricsLibrary());
    } catch (cause) {
      setError(messageOf(cause));
    } finally {
      setBusy(null);
    }
  };

  const changeDirectory = async () => {
    setError(null);
    try {
      const path = await open({
        directory: true,
        multiple: false,
        defaultPath: overview?.libraryDir,
        title: "选择歌词目录",
      });
      if (!path) return;
      setBusy("directory");
      applyOverview(await api.setLyricsDirectory(path));
    } catch (cause) {
      setError(messageOf(cause));
    } finally {
      setBusy(null);
    }
  };

  return (
    <main className={styles.shell}>
      <header className={styles.header}>
        <div>
          <span>LOCAL LIBRARY</span>
          <h1>本地歌词库</h1>
          <p>{overview?.libraryDir ?? "正在读取歌词目录…"}</p>
        </div>
        <div className={styles.headerActions}>
          <button disabled={busy !== null} onClick={() => void rescan()}>
            {busy === "rescan" ? "扫描中…" : "重新扫描"}
          </button>
          <Link to="/">返回播放页</Link>
        </div>
      </header>

      <section className={styles.folderPanel}>
        <div className={styles.sectionTitle}>
          <div><span>FOLDER</span><h2>歌词目录</h2></div>
          <button disabled={busy !== null} onClick={() => void changeDirectory()}>
            {busy === "directory" ? "切换中…" : "修改目录"}
          </button>
        </div>
        <p className={styles.folderPath}>{overview?.libraryDir ?? "—"}</p>
      </section>

      <section className={styles.workspace}>
        <div className={styles.browser}>
          <div className={styles.listHeader}><span>{overview?.entries.length ?? 0} 首歌词</span><span>时长 / 大小</span></div>
          <div className={styles.entryList}>
            {overview?.entries.map((entry) => (
              <button key={entry.path} data-selected={entry.path === selectedPath} onClick={() => void selectEntry(entry)}>
                <span><strong>{entry.title}</strong><small>{entry.artist} · {entry.source} · {entry.format.toUpperCase()}</small></span>
                <span className={styles.badges}>
                  {entry.duplicateCount > 1 && <b>重复 ×{entry.duplicateCount}</b>}
                  {entry.associationCount > 0 && <b>已关联 {entry.associationCount}</b>}
                  {entry.hasWordTiming && <b>逐字</b>}
                  {entry.hasTranslation && <b>翻译</b>}
                  {entry.hasRomanization && <b>音译</b>}
                </span>
                <em>{formatDuration(entry.durationMs)}<small>{formatBytes(entry.fileSize)}</small></em>
              </button>
            ))}
            {overview && overview.entries.length === 0 && <div className={styles.empty}>当前歌词目录中没有歌词</div>}
          </div>
        </div>

        <aside className={styles.preview}>
          {!preview ? <div className={styles.empty}>{busy === "preview" ? "正在读取歌词…" : "选择一首歌词查看原文"}</div> : (
            <>
              <div className={styles.previewHeading}>
                <span>PREVIEW</span><h2>{preview.entry.title}</h2><p>{preview.entry.artist}</p>
              </div>
              <div className={styles.previewMeta}>
                <span>{preview.entry.source}</span><span>{preview.entry.format.toUpperCase()}</span><span>{preview.document?.tracks.original.lines.length ?? 0} 行</span>
                {preview.entry.hasWordTiming && <span>逐字</span>}
                {preview.entry.hasTranslation && <span>翻译</span>}
                {preview.entry.hasRomanization && <span>音译</span>}
              </div>
              <pre>{preview.raw}</pre>
            </>
          )}
        </aside>
      </section>
      {error && <div className={styles.toast} onClick={() => setError(null)}>{error}</div>}
    </main>
  );
}
