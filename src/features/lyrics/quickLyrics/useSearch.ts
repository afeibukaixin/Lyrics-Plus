import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useLyrics } from "../useLyrics";
import { usePlayback } from "../../player/usePlayback";
import { isTauriRuntime } from "../../../shared/api";
import { createTauriListenerCleanup, QUICK_LYRICS_REFRESH_EVENT } from "../../../shared/tauriEvent";
import {
  formatDurationParts,
  parseDuration,
  type SearchFormState,
} from "./utils";

type PlaybackController = ReturnType<typeof usePlayback>;
type LyricsController = ReturnType<typeof useLyrics>;

const emptySearchForm: SearchFormState = {
  title: "",
  artist: "",
  album: "",
  durationMinutes: "",
  durationSeconds: "",
};

export function useQuickLyricsSearch(
  playback: PlaybackController,
  lyrics: LyricsController,
) {
  const searchedTrack = useRef<string | null>(null);
  const searchRef = useRef(lyrics.search);
  searchRef.current = lyrics.search;
  const searchStateRef = useRef({
    trackKey: lyrics.trackKey,
    title: playback.snapshot.title,
    artist: playback.snapshot.artist,
    searching: lyrics.searching,
  });
  searchStateRef.current = {
    trackKey: lyrics.trackKey,
    title: playback.snapshot.title,
    artist: playback.snapshot.artist,
    searching: lyrics.searching,
  };
  const [searchForm, setSearchForm] = useState<SearchFormState>(emptySearchForm);
  const [formSubmitted, setFormSubmitted] = useState(false);

  useEffect(() => {
    const duration = formatDurationParts(playback.snapshot.durationMs);
    setSearchForm({
      title: playback.snapshot.title ?? "",
      artist: playback.snapshot.artist ?? "",
      album: playback.snapshot.album ?? "",
      ...duration,
    });
    setFormSubmitted(false);
  }, [lyrics.trackKey]);

  useEffect(() => {
    if (
      !lyrics.trackKey
      || !playback.snapshot.title
      || !playback.snapshot.artist
      || (lyrics.loadState !== "ready" && lyrics.loadState !== "missing")
    ) return;
    if (searchedTrack.current === lyrics.trackKey) return;
    searchedTrack.current = lyrics.trackKey;
    void lyrics.search("refresh");
  }, [lyrics.loadState, lyrics.trackKey, playback.snapshot.artist, playback.snapshot.title]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    return createTauriListenerCleanup(listen(QUICK_LYRICS_REFRESH_EVENT, () => {
      const current = searchStateRef.current;
      if (!current.trackKey || !current.title || !current.artist || current.searching) return;
      void searchRef.current("refresh");
    }));
  }, []);

  const updateSearchField = (field: keyof SearchFormState, value: string) => {
    setSearchForm((current) => ({ ...current, [field]: value }));
  };

  const searchLyrics = async (onValidSearch?: () => void) => {
    setFormSubmitted(true);
    const title = searchForm.title.trim();
    const artist = searchForm.artist.trim();
    const durationMs = parseDuration(searchForm.durationMinutes, searchForm.durationSeconds);
    if (!title || !artist || durationMs === undefined || !lyrics.trackKey || lyrics.searching) return;
    onValidSearch?.();
    await lyrics.searchWith({
      title,
      artist,
      album: searchForm.album.trim() || null,
      durationMs,
    }, "manual");
  };

  const parsedDuration = parseDuration(searchForm.durationMinutes, searchForm.durationSeconds);
  const titleInvalid = formSubmitted && !searchForm.title.trim();
  const artistInvalid = formSubmitted && !searchForm.artist.trim();
  const durationInvalid = formSubmitted && parsedDuration === undefined;
  const formDisabled = !lyrics.trackKey || lyrics.searching;

  return {
    searchForm,
    searching: lyrics.searching,
    updateSearchField,
    searchLyrics,
    titleInvalid,
    artistInvalid,
    durationInvalid,
    formDisabled,
  };
}
