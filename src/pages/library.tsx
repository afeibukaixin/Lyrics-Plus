import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Link } from "react-router";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { localizedSource } from "../features/i18n/userText";
import { api, isTauriRuntime, messageOf } from "../shared/api";
import { createTauriListenerCleanup } from "../shared/tauriEvent";
import type {
  LibraryEntry,
  LibraryPage,
  LibraryPreview,
  LibraryScanStatus,
} from "../shared/types";
import styles from "./library.module.scss";
import { UiIcon } from "../components/UiIcon";

const PAGE_SIZE = 100;
const WINDOW_SIZE = 200;
const ROW_HEIGHT = 58;

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

function scanDescription(status: LibraryScanStatus | null, t: TFunction) {
  if (!status || status.phase === "idle") return null;
  const skipped = status.skipped ? t("library.scan.skipped", { count: status.skipped }) : "";
  if (status.phase === "discovering") {
    return t("library.scan.discovering", { discovered: status.discovered, skipped });
  }
  if (status.phase === "indexing") {
    return t("library.scan.indexing", { processed: status.processed, total: status.total ?? status.discovered, skipped });
  }
  if (status.phase === "completed") {
    return t("library.scan.completed", { total: status.total ?? status.processed, skipped });
  }
  if (status.phase === "failed") return t("library.scan.failed");
  if (status.phase === "cancelled") return t("library.scan.cancelled");
  return null;
}

export default function Library() {
  const { t } = useTranslation();
  const [page, setPage] = useState<LibraryPage | null>(null);
  const [libraryDir, setLibraryDir] = useState<string | null>(null);
  const [scanStatus, setScanStatus] = useState<LibraryScanStatus | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<LibraryPreview | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [searchInput, setSearchInput] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const listRef = useRef<HTMLDivElement>(null);
  const pageOffsetRef = useRef(0);
  const queryRef = useRef("");
  const requestIdRef = useRef(0);
  const refreshTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const scrollFrameRef = useRef<number | null>(null);

  const loadWindow = useCallback(async (offset: number, query = queryRef.current) => {
    const requestId = ++requestIdRef.current;
    const value = await api.getLibraryPage(query, offset, WINDOW_SIZE);
    if (requestId !== requestIdRef.current) return null;
    pageOffsetRef.current = value.offset;
    setPage(value);
    setLibraryDir(value.libraryDir);
    return value;
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => setSearchQuery(searchInput.trim()), 200);
    return () => clearTimeout(timer);
  }, [searchInput]);

  useEffect(() => {
    document.documentElement.dataset.window = "main";
    if (!isTauriRuntime()) return;
    queryRef.current = searchQuery;
    if (listRef.current) listRef.current.scrollTop = 0;
    setError(null);
    void loadWindow(0, searchQuery).catch((cause) => setError(messageOf(cause)));
  }, [loadWindow, searchQuery]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    void api.getLibraryScanStatus().then((status) => {
      setScanStatus(status);
      setLibraryDir(status.libraryDir);
    }).catch((cause) => setError(messageOf(cause)));

    const scheduleRefresh = () => {
      if (refreshTimerRef.current) return;
      refreshTimerRef.current = setTimeout(() => {
        refreshTimerRef.current = null;
        void loadWindow(pageOffsetRef.current).catch((cause) => setError(messageOf(cause)));
      }, 250);
    };
    const cleanup = createTauriListenerCleanup(
      listen<LibraryScanStatus>("lyrics://library-scan-progress", ({ payload }) => {
        setScanStatus(payload);
        setLibraryDir(payload.libraryDir);
        if (payload.phase === "indexing" || payload.phase === "completed") scheduleRefresh();
        if (payload.phase === "failed") setError(t("library.scan.failed"));
      }),
    );
    return () => {
      cleanup();
      if (refreshTimerRef.current) clearTimeout(refreshTimerRef.current);
    };
  }, [loadWindow, t]);

  const onListScroll = () => {
    if (scrollFrameRef.current !== null) return;
    scrollFrameRef.current = requestAnimationFrame(() => {
      scrollFrameRef.current = null;
      const scrollTop = listRef.current?.scrollTop ?? 0;
      const nextOffset = Math.floor(scrollTop / (ROW_HEIGHT * PAGE_SIZE)) * PAGE_SIZE;
      if (nextOffset === pageOffsetRef.current || nextOffset >= (page?.totalCount ?? 0)) return;
      void loadWindow(nextOffset).catch((cause) => setError(messageOf(cause)));
    });
  };

  useEffect(() => () => {
    if (scrollFrameRef.current !== null) cancelAnimationFrame(scrollFrameRef.current);
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
    setError(null);
    try {
      setScanStatus(await api.rescanLyricsLibrary());
    } catch (cause) {
      setError(messageOf(cause));
    }
  };

  const changeDirectory = async () => {
    setError(null);
    try {
      const path = await openDialog({
        directory: true,
        multiple: false,
        defaultPath: libraryDir ?? undefined,
        title: t("library.chooseFolder"),
      });
      if (!path) return;
      setBusy("directory");
      const status = await api.setLyricsDirectory(path);
      setScanStatus(status);
      setLibraryDir(status.libraryDir);
      setPage({ libraryDir: status.libraryDir, entries: [], totalCount: 0, offset: 0, limit: WINDOW_SIZE });
      setSelectedPath(null);
      setPreview(null);
      if (listRef.current) listRef.current.scrollTop = 0;
      await loadWindow(0);
    } catch (cause) {
      setError(messageOf(cause));
    } finally {
      setBusy(null);
    }
  };

  const openLibraryDirectory = async () => {
    if (!libraryDir) return;
    setError(null);
    try {
      await api.openLyricsDirectory();
    } catch (cause) {
      setError(messageOf(cause));
    }
  };

  const revealPreview = async () => {
    if (!preview) return;
    setError(null);
    try {
      await api.revealLibraryEntry(preview.entry.path);
    } catch (cause) {
      setError(messageOf(cause));
    }
  };

  const searching = searchQuery.length > 0;
  const totalCount = page?.totalCount ?? 0;
  const entries = page?.entries ?? [];
  const virtualHeight = totalCount * ROW_HEIGHT;
  const virtualTop = (page?.offset ?? 0) * ROW_HEIGHT;
  const statusText = scanDescription(scanStatus, t);
  const scanning = scanStatus?.phase === "discovering" || scanStatus?.phase === "indexing";
  const resultText = useMemo(
    () => searching ? t("library.found", { count: totalCount }) : t("common.units.song", { count: totalCount }),
    [searching, t, totalCount],
  );

  return (
    <main className={styles.shell}>
      <header className={styles.header}>
        <div>
          <span>{t("library.eyebrow").toUpperCase()}</span>
          <h1>{t("library.title")}</h1>
          <p>{libraryDir ?? t("library.loadingDirectory")}</p>
        </div>
        <div className={styles.headerActions}>
          <button onClick={() => void rescan()}>{scanning ? t("library.restartScan") : t("library.rescan")}</button>
          <Link to="/">{t("library.backPlayback")}</Link>
        </div>
      </header>

      <section className={styles.folderPanel}>
        <div className={styles.sectionTitle}>
          <div><span>{t("library.folder").toUpperCase()}</span><h2>{t("library.folder")}</h2></div>
          <div className={styles.directoryActions}>
            <button disabled={!libraryDir} onClick={() => void openLibraryDirectory()}>{t("library.openFolder")}</button>
            <button disabled={busy === "directory"} onClick={() => void changeDirectory()}>
              {busy === "directory" ? t("library.changing") : t("library.changeFolder")}
            </button>
          </div>
        </div>
        <p className={styles.folderPath}>{libraryDir ?? "—"}</p>
        {statusText && <p className={styles.scanStatus} data-error={scanStatus?.phase === "failed"}>{statusText}</p>}
      </section>

      <section className={styles.workspace}>
        <div className={styles.browser}>
          <div className={styles.listToolbar}>
            <label className={styles.searchBox}>
              <UiIcon name="search" />
              <input
                aria-label={t("library.searchLabel")}
                autoComplete="off"
                placeholder={t("library.searchPlaceholder")}
                value={searchInput}
                onChange={(event) => setSearchInput(event.currentTarget.value)}
              />
              {searchInput && <button type="button" aria-label={t("library.clearSearch")} onClick={() => setSearchInput("")}><UiIcon name="close" /></button>}
            </label>
            <span className={styles.resultCount}>{resultText}</span>
            <span className={styles.columnHint}>{t("library.durationSize")}</span>
          </div>
          <div ref={listRef} className={styles.entryList} onScroll={onListScroll}>
            {totalCount > 0 && (
              <div className={styles.virtualSpace} style={{ height: virtualHeight }}>
                <div className={styles.virtualWindow} style={{ transform: `translateY(${virtualTop}px)` }}>
                  {entries.map((entry) => (
                    <button key={entry.path} data-selected={entry.path === selectedPath} onClick={() => void selectEntry(entry)}>
                      <span><strong>{entry.title}</strong><small>{entry.artist} · {localizedSource(entry.source, t)} · {entry.format.toUpperCase()}</small></span>
                      <span className={styles.badges}>
                        {entry.duplicateCount > 1 && <b>{t("library.duplicate", { count: entry.duplicateCount })}</b>}
                        {entry.associationCount > 0 && <b>{t("library.associated", { count: entry.associationCount })}</b>}
                        {entry.hasWordTiming && <b>{t("common.feature.wordTiming")}</b>}
                        {entry.hasTranslation && <b>{t("common.feature.translation")}</b>}
                        {entry.hasRomanization && <b>{t("common.feature.romanization")}</b>}
                      </span>
                      <em>{formatDuration(entry.durationMs)}<small>{formatBytes(entry.fileSize)}</small></em>
                    </button>
                  ))}
                </div>
              </div>
            )}
            {page && totalCount === 0 && !searching && <div className={styles.empty}>{scanning ? t("library.indexingEmpty") : t("library.directoryEmpty")}</div>}
            {page && totalCount === 0 && searching && (
              <div className={styles.searchEmpty}>
                <span>{t("library.noMatches")}</span>
                <button type="button" onClick={() => setSearchInput("")}>{t("library.clearSearch")}</button>
              </div>
            )}
          </div>
        </div>

        <aside className={styles.preview}>
          {!preview ? <div className={styles.empty}>{busy === "preview" ? t("library.readingPreview") : t("library.selectPreview")}</div> : (
            <>
              <div className={styles.previewTop}>
                <div className={styles.previewHeading}>
                  <span>{t("library.preview").toUpperCase()}</span><h2>{preview.entry.title}</h2><p>{preview.entry.artist}</p>
                </div>
                <button onClick={() => void revealPreview()}>{t("library.revealFile")}</button>
              </div>
              <div className={styles.previewMeta}>
                <span>{localizedSource(preview.entry.source, t)}</span><span>{preview.entry.format.toUpperCase()}</span><span>{t("common.units.line", { count: preview.document?.tracks.original.lines.length ?? 0 })}</span>
                {preview.entry.hasWordTiming && <span>{t("common.feature.wordTiming")}</span>}
                {preview.entry.hasTranslation && <span>{t("common.feature.translation")}</span>}
                {preview.entry.hasRomanization && <span>{t("common.feature.romanization")}</span>}
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
