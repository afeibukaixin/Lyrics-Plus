import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauriRuntime, messageOf, trackKeyOf } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import type {
  LyricsDocument,
  LyricsLine,
  LyricsSearchInput,
  LyricsSearchResult,
  PlaybackSnapshot,
  ProviderStatus,
} from "../../shared/types";

const AUXILIARY_TIMESTAMP_TOLERANCE_MS = 500;

type PendingOffsetWrite = {
  desiredOffsetMs: number;
  count: number;
};

export function findAlignedAuxiliaryLine(lines: LyricsLine[], currentLine: LyricsLine) {
  const exact = lines.find((line) => line.startMs === currentLine.startMs && line.text.trim());
  if (exact) return exact;
  let nearest: LyricsLine | null = null;
  let nearestDelta = AUXILIARY_TIMESTAMP_TOLERANCE_MS + 1;
  for (const line of lines) {
    if (!line.text.trim()) continue;
    const delta = Math.abs(line.startMs - currentLine.startMs);
    if (delta < nearestDelta) {
      nearest = line;
      nearestDelta = delta;
    }
  }
  return nearestDelta <= AUXILIARY_TIMESTAMP_TOLERANCE_MS ? nearest : null;
}

export function useLyrics(snapshot: PlaybackSnapshot, positionMs: number, autoSearch: boolean) {
  const trackKey = useMemo(() => trackKeyOf(snapshot), [snapshot]);
  const [document, setDocument] = useState<LyricsDocument | null>(null);
  const [results, setResults] = useState<LyricsSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [providerStatuses, setProviderStatuses] = useState<ProviderStatus[]>([]);
  const [error, setError] = useState<string | null>(null);
  const attempted = useRef(new Set<string>());
  const activeTrackKey = useRef(trackKey);
  const documentRef = useRef<LyricsDocument | null>(null);
  const documentTrackKey = useRef<string | null>(null);
  const pendingOffsetWrites = useRef(new Map<string, PendingOffsetWrite>());
  const offsetWriteQueue = useRef<Promise<void>>(Promise.resolve());
  activeTrackKey.current = trackKey;

  const updateDocument = useCallback((next: LyricsDocument | null, key: string | null = activeTrackKey.current) => {
    documentRef.current = next;
    documentTrackKey.current = next ? key : null;
    setDocument(next);
  }, []);

  const loadTrack = useCallback(async (key: string) => {
    try {
      const cached = await api.getCachedLyrics(key);
      const pending = pendingOffsetWrites.current.get(key);
      const next = cached && pending
        ? { ...cached, offsetMs: pending.desiredOffsetMs }
        : cached;
      if (activeTrackKey.current === key) updateDocument(next, key);
      return next;
    } catch (loadError) {
      if (activeTrackKey.current === key) setError(messageOf(loadError));
      return null;
    }
  }, [updateDocument]);

  const load = useCallback(async () => {
    if (!trackKey) {
      updateDocument(null);
      setResults([]);
      return null;
    }
    return loadTrack(trackKey);
  }, [loadTrack, trackKey, updateDocument]);

  const applyResult = useCallback(async (result: LyricsSearchResult, manualSelected = true) => {
    if (!trackKey || !snapshot.title || !snapshot.artist) return null;
    setError(null);
    try {
      const saved = await api.saveLyrics(trackKey, snapshot.title, snapshot.artist, result, manualSelected);
      if (activeTrackKey.current === trackKey) updateDocument(saved, trackKey);
      return saved;
    } catch (saveError) {
      setError(messageOf(saveError));
      return null;
    }
  }, [snapshot.artist, snapshot.title, trackKey, updateDocument]);

  const search = useCallback(async (
    allowAutoApply = false,
    override?: LyricsSearchInput,
  ) => {
    const input = override ?? {
      title: snapshot.title ?? "",
      artist: snapshot.artist ?? "",
      album: snapshot.album,
      durationMs: snapshot.durationMs,
    };
    if (!input.title.trim() || !input.artist.trim()) return;
    setSearching(true);
    setError(null);
    try {
      const response = await api.searchLyrics(input);
      setResults(response.results);
      setProviderStatuses(response.providerStatuses);
      if (allowAutoApply && response.autoApply && response.results[0]) {
        await applyResult(response.results[0], false);
      } else if (response.results.length === 0) {
        setError("已启用的歌词源暂时没有找到同步歌词");
      }
    } catch (searchError) {
      setError(messageOf(searchError));
    } finally {
      setSearching(false);
    }
  }, [applyResult, snapshot.album, snapshot.artist, snapshot.durationMs, snapshot.title]);

  useEffect(() => {
    setError(null);
    setResults([]);
    updateDocument(null);
    void load().then((cached) => {
      if (autoSearch && trackKey && !cached && !attempted.current.has(trackKey)) {
        attempted.current.add(trackKey);
        void search(true);
      }
    });
  }, [autoSearch, load, search, trackKey, updateDocument]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    const cleanupLyricsListener = createTauriListenerCleanup(listen<string>("lyrics://changed", ({ payload }) => {
      if (payload === trackKey) void load();
    }));
    const cleanupLibraryListener = createTauriListenerCleanup(
      listen("lyrics://library-changed", () => void load()),
    );
    return () => {
      cleanupLyricsListener();
      cleanupLibraryListener();
    };
  }, [load, trackKey]);

  const activeIndex = useMemo(() => {
    if (!document) return -1;
    const adjusted = positionMs + document.offsetMs;
    let found = -1;
    const lines = document.tracks.original.lines;
    for (let index = 0; index < lines.length; index += 1) {
      if (lines[index].startMs > adjusted) break;
      found = index;
    }
    return found;
  }, [document, positionMs]);

  const originalLines = document?.tracks.original.lines;
  const currentLine: LyricsLine | null = originalLines?.[activeIndex] ?? null;
  const nextLine: LyricsLine | null = originalLines?.[activeIndex + 1] ?? null;
  const currentTranslation: LyricsLine | null = useMemo(() => {
    if (!currentLine || !document?.tracks.translation) return null;
    return findAlignedAuxiliaryLine(document.tracks.translation.lines, currentLine);
  }, [currentLine, document]);
  const currentRomanization: LyricsLine | null = useMemo(() => {
    if (!currentLine || !document?.tracks.romanization) return null;
    return findAlignedAuxiliaryLine(document.tracks.romanization.lines, currentLine);
  }, [currentLine, document]);

  const importRaw = async (raw: string) => {
    if (!trackKey || !snapshot.title || !snapshot.artist) return;
    setError(null);
    try {
      const imported = await api.importLyrics(trackKey, snapshot.title, snapshot.artist, raw);
      if (activeTrackKey.current === trackKey) updateDocument(imported, trackKey);
    } catch (importError) {
      setError(messageOf(importError));
    }
  };

  const enqueueOffsetWrite = (resolveNext: (currentOffsetMs: number) => number) => {
    const key = trackKey;
    const current = documentRef.current;
    if (!key || !current || documentTrackKey.current !== key) return Promise.resolve();

    const existing = pendingOffsetWrites.current.get(key);
    const next = Math.trunc(resolveNext(existing?.desiredOffsetMs ?? current.offsetMs));
    pendingOffsetWrites.current.set(key, {
      desiredOffsetMs: next,
      count: (existing?.count ?? 0) + 1,
    });
    updateDocument({ ...current, offsetMs: next }, key);
    setError(null);

    let writeError: unknown = null;
    const write = offsetWriteQueue.current
      .then(() => api.setLyricsOffset(key, next))
      .catch((offsetError: unknown) => {
        writeError = offsetError;
      })
      .then(async () => {
        const pending = pendingOffsetWrites.current.get(key);
        if (!pending) return;
        if (pending.count > 1) {
          pendingOffsetWrites.current.set(key, { ...pending, count: pending.count - 1 });
          return;
        }
        pendingOffsetWrites.current.delete(key);
        await loadTrack(key);
        if (writeError && activeTrackKey.current === key) setError(messageOf(writeError));
      });
    offsetWriteQueue.current = write;
    return write;
  };

  const changeOffset = (delta: number) => enqueueOffsetWrite((current) => current + delta);
  const setOffset = (offsetMs: number) => enqueueOffsetWrite(() => offsetMs);

  const remove = async () => {
    if (!trackKey) return;
    try {
      await api.removeLyricsAssociation(trackKey);
      if (activeTrackKey.current === trackKey) updateDocument(null);
    } catch (removeError) {
      setError(messageOf(removeError));
    }
  };

  return {
    trackKey,
    document,
    results,
    providerStatuses,
    searching,
    error,
    activeIndex,
    currentLine,
    nextLine,
    currentTranslation,
    currentRomanization,
    adjustedPositionMs: positionMs + (document?.offsetMs ?? 0),
    search: () => search(false),
    searchWith: (input: LyricsSearchInput) => search(false, input),
    applyResult,
    importRaw,
    changeOffset,
    setOffset,
    remove,
  };
}
