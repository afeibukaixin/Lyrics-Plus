import { invoke } from "./core";
import type {
  LibraryScanStatus,
  LyricsDocument,
  LyricsLoadResponse,
  LyricsMonitor,
  LyricsRuntimeSnapshot,
  LyricsSearchInput,
  LyricsSearchResult,
  MusixmatchTokenType,
  NotchLayoutMetrics,
  ProviderCredentialUpdate,
  ProviderCredentialView,
  ProviderSettings,
  ProviderSettingsView,
  ProviderStatus,
  SearchResponse,
  LyricsSearchIntent,
 } from "./types";

export const lyricsApi = {
  getCachedLyrics: (trackKey: string) =>
    invoke<LyricsLoadResponse>("get_cached_lyrics", { trackKey }),
  getCompletedLyricsSearch: (trackKey: string) =>
    invoke<SearchResponse | null>("get_completed_lyrics_search", { trackKey }),
  getLyricsRuntimeSnapshot: () =>
    invoke<LyricsRuntimeSnapshot>("get_lyrics_runtime_snapshot"),
  getNotchLayoutMetrics: () => invoke<NotchLayoutMetrics>("get_notch_layout_metrics"),
  getLyricsMonitors: () => invoke<LyricsMonitor[]>("get_lyrics_monitors"),
  getLibraryScanStatus: () => invoke<LibraryScanStatus>("get_library_scan_status"),
  rescanLyricsLibrary: () => invoke<LibraryScanStatus>("rescan_lyrics_library"),
  setLyricsDirectory: (path: string) =>
    invoke<LibraryScanStatus>("set_lyrics_directory", { path }),
  openLyricsDirectory: () => invoke<void>("open_lyrics_directory"),
  searchLyrics: (
    trackKey: string,
    input: LyricsSearchInput,
    intent: LyricsSearchIntent = "automatic",
  ) => invoke<SearchResponse>("search_lyrics", { trackKey, input, intent }),
  getProviderSettings: () => invoke<ProviderSettingsView>("get_provider_settings"),
  getProviderCredentials: () => invoke<ProviderCredentialView>("get_provider_credentials"),
  setProviderSettings: (settings: ProviderSettings) =>
    invoke<ProviderSettingsView>("set_provider_settings", { settings }),
  setMusixmatchToken: (tokenType: MusixmatchTokenType, token: string) =>
    invoke<ProviderCredentialUpdate>("set_musixmatch_token", { tokenType, token }),
  clearMusixmatchToken: () =>
    invoke<ProviderCredentialUpdate>("clear_musixmatch_token"),
  testProvider: (providerId: string) =>
    invoke<ProviderStatus>("test_provider", { providerId }),
  saveLyrics: (
    trackKey: string,
    title: string,
    artist: string,
    album: string | null,
    durationMs: number | null,
    result: Pick<LyricsSearchResult, "id" | "providerId" | "source" | "lyrics">,
    manualSelected: boolean,
  ) =>
    invoke<LyricsDocument>("save_lyrics", {
      input: {
        trackKey,
        title,
        artist,
        album,
        durationMs,
        source: result.source,
        lyrics: result.lyrics,
        providerId: result.providerId,
        providerItemId: result.id,
        manualSelected,
      },
    }),
  importLyrics: (trackKey: string, title: string, artist: string, album: string | null, durationMs: number | null, lyrics: string) =>
    invoke<LyricsDocument>("import_lyrics", {
      input: {
        trackKey,
        title,
        artist,
        album,
        durationMs,
        source: "本地导入",
        lyrics,
        providerId: null,
        providerItemId: null,
        manualSelected: true,
      },
    }),
  setLyricsOffset: (trackKey: string, offsetMs: number) =>
    invoke<void>("set_lyrics_offset", { trackKey, offsetMs }),
  removeLyricsAssociation: (trackKey: string) =>
    invoke<void>("remove_lyrics_association", { trackKey }),
};
