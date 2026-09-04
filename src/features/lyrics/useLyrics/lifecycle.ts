import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

import { isTauriRuntime } from "../../../shared/api";
import { createTauriListenerCleanup } from "../../../shared/tauriEvent";
import type { LyricsDocument, SearchResponse } from "../../../shared/types";

import type { LoadLyricsTrack, UpdateLyricsDocument } from "./document";
import type { LyricsState } from "./state";

type LoadLyrics = () => Promise<LyricsDocument | null>;
type RestoreCompletedSearch = (key: string) => Promise<SearchResponse | null>;

export function useLyricsLifecycle(
  active: boolean,
  trackKey: string | null,
  state: LyricsState,
  loadTrack: LoadLyricsTrack,
  load: LoadLyrics,
  restoreCompletedSearch: RestoreCompletedSearch,
  updateDocument: UpdateLyricsDocument,
) {
  useEffect(() => {
    ++state.searchGeneration.current;
    state.setSearching(false);
    state.setError(null);
    state.setResults([]);
    updateDocument(null);
    state.setLoadState(active && trackKey ? "loading" : "idle");
    if (!active || !trackKey) return;
    void loadTrack(trackKey);
    void restoreCompletedSearch(trackKey);
  }, [active, loadTrack, restoreCompletedSearch, trackKey, updateDocument]);

  useEffect(() => {
    if (!active || !isTauriRuntime()) return;
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
  }, [active, load, trackKey]);
}
