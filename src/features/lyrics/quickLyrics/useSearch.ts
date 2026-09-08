import { useEffect, useRef, useState } from "react";
import { useLyrics } from "../useLyrics";
import { usePlayback } from "../../player/usePlayback";
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
  const [searchForm, setSearchForm] = useState<SearchFormState>(emptySearchForm);
  const [formSubmitted, setFormSubmitted] = useState(false);

  useEffect(() => {
    // 窗口休眠时 trackKey 会清空；重新激活同一首歌需要允许重新接入搜索会话。
    searchedTrack.current = null;
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
      !playback.active
      || !lyrics.trackKey
      || !playback.snapshot.title
      || !playback.snapshot.artist
      || lyrics.loadState !== "missing"
      || lyrics.searchRestoreState !== "absent"
      || lyrics.searching
    ) return;
    if (searchedTrack.current === lyrics.trackKey) return;
    searchedTrack.current = lyrics.trackKey;
    void lyrics.search("refresh");
  }, [
    lyrics.loadState,
    lyrics.searchRestoreState,
    lyrics.searching,
    lyrics.trackKey,
    playback.active,
    playback.snapshot.artist,
    playback.snapshot.title,
  ]);

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
      platform: playback.snapshot.player,
      platformItemId: playback.snapshot.trackId,
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
