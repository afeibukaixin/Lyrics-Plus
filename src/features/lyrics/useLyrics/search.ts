import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { api, messageOf } from "../../../shared/api";
import type {
  LyricsSearchInput,
  LyricsSearchIntent,
  LyricsSearchResult,
  PlaybackSnapshot,
  SearchResponse,
} from "../../../shared/types";

import type { UpdateLyricsDocument } from "./document";
import type { LyricsState } from "./state";

export function useLyricsSearch(
  snapshot: PlaybackSnapshot,
  trackKey: string | null,
  state: LyricsState,
  updateDocument: UpdateLyricsDocument,
) {
  const { t } = useTranslation();

  const applySearchResponse = useCallback((response: Pick<SearchResponse, "results" | "providerStatuses" | "error">) => {
    state.setResults(response.results);
    state.setProviderStatuses(response.providerStatuses);
    if (response.error) {
      state.setError(response.error);
    } else if (response.results.length === 0) {
      state.setError(t("settings.lyrics.noResults"));
    }
  }, [t]);

  const restoreCompletedSearch = useCallback(async (key: string) => {
    const restoreGeneration = state.searchGeneration.current;
    try {
      const response = await api.getCompletedLyricsSearch(key);
      if (
        response
        && state.activeTrackKey.current === key
        && state.searchGeneration.current === restoreGeneration
      ) {
        applySearchResponse(response);
      }
      return response;
    } catch (restoreError) {
      if (state.activeTrackKey.current === key && state.searchGeneration.current === restoreGeneration) {
        state.setError(messageOf(restoreError));
      }
      return null;
    }
  }, [applySearchResponse]);

  const applyResult = useCallback(async (result: LyricsSearchResult, manualSelected = true) => {
    if (!trackKey || !snapshot.title || !snapshot.artist) return null;
    state.setError(null);
    try {
      const saved = await api.saveLyrics(
        trackKey,
        snapshot.title,
        snapshot.artist,
        snapshot.album,
        snapshot.durationMs,
        result,
        manualSelected,
      );
      if (state.activeTrackKey.current === trackKey) {
        updateDocument(saved, trackKey);
        state.setLoadState("ready");
      }
      return saved;
    } catch (saveError) {
      if (state.activeTrackKey.current === trackKey) state.setError(messageOf(saveError));
      return null;
    }
  }, [snapshot.album, snapshot.artist, snapshot.durationMs, snapshot.title, trackKey, updateDocument]);

  const search = useCallback(async (
    intent: LyricsSearchIntent = "automatic",
    override?: LyricsSearchInput,
  ) => {
    const input = override ?? {
      title: snapshot.title ?? "",
      artist: snapshot.artist ?? "",
      album: snapshot.album,
      durationMs: snapshot.durationMs,
    };
    if (!trackKey || !input.title.trim() || !input.artist.trim()) return null;
    const generation = ++state.searchGeneration.current;
    const key = trackKey;
    const isCurrent = () => state.searchGeneration.current === generation && state.activeTrackKey.current === key;
    state.setSearching(true);
    state.setError(null);
    try {
      const response = await api.searchLyrics(trackKey, input, intent);
      if (!isCurrent()) return null;
      applySearchResponse(response);
      return response;
    } catch (searchError) {
      if (isCurrent()) state.setError(messageOf(searchError));
      return null;
    } finally {
      if (isCurrent()) state.setSearching(false);
    }
  }, [applySearchResponse, snapshot.album, snapshot.artist, snapshot.durationMs, snapshot.title, trackKey]);

  return {
    search: (intent: LyricsSearchIntent = "automatic") => search(intent),
    searchWith: (input: LyricsSearchInput, intent: LyricsSearchIntent = "manual") => search(intent, input),
    applyResult,
    restoreCompletedSearch,
  };
}
