import { invoke } from "./core";
import type {
  LibraryScanStatus,
  LibraryRootView,
  LyricsDocument,
  LyricsLoadResponse,
  LyricsContext,
  LyricsMonitor,
  LyricsRuntimeSnapshot,
  RecordingView,
  LyricsSearchInput,
  LyricsSearchTrace,
  LyricsSearchResult,
  MusixmatchTokenType,
  NotchLayoutMetrics,
  ProviderCredentialUpdate,
  ProviderCredentialView,
  ProviderDescriptor,
  ProviderSettings,
  ProviderSettingsView,
  ProviderStatus,
  SearchResponse,
  LyricsSearchIntent,
  SongAssociationCandidate,
} from "./types";

export const lyricsApi = {
  getCachedLyrics: (trackKey: string) =>
    invoke<LyricsLoadResponse>("get_cached_lyrics", { trackKey }),
  getCompletedLyricsSearch: (trackKey: string) =>
    invoke<SearchResponse | null>("get_completed_lyrics_search", { trackKey }),
  getLyricsSearchTrace: (trackKey: string) =>
    invoke<LyricsSearchTrace | null>("get_lyrics_search_trace", { trackKey }),
  getCurrentLyricsContext: (trackKey: string) =>
    invoke<LyricsContext | null>("get_current_lyrics_context", { trackKey }),
  setArtistAliasConfirmation: (
    trackKey: string,
    artistId: number,
    alias: string,
    confirmed: boolean,
  ) => invoke<LyricsContext>("set_artist_alias_confirmation", {
    input: { trackKey, artistId, alias, confirmed },
  }),
  getLyricsRuntimeSnapshot: () =>
    invoke<LyricsRuntimeSnapshot>("get_lyrics_runtime_snapshot"),
  getNotchLayoutMetrics: () => invoke<NotchLayoutMetrics>("get_notch_layout_metrics"),
  getLyricsMonitors: () => invoke<LyricsMonitor[]>("get_lyrics_monitors"),
  getLibraryScanStatus: () => invoke<LibraryScanStatus>("get_library_scan_status"),
  rescanLyricsLibrary: () => invoke<LibraryScanStatus>("rescan_lyrics_library"),
  getLibraryRoots: () => invoke<LibraryRootView[]>("list_library_roots"),
  addLibraryRoot: (path: string, displayName?: string) =>
    invoke<LibraryRootView>("add_library_root", { path, displayName: displayName ?? null }),
  setLibraryRootEnabled: (rootId: string, enabled: boolean) =>
    invoke<LibraryRootView>("set_library_root_enabled", { rootId, enabled }),
  removeLibraryRoot: (rootId: string) =>
    invoke<void>("remove_library_root", { rootId }),
  rescanLibraryRoot: (rootId: string) =>
    invoke<LibraryScanStatus>("rescan_library_root", { rootId }),
  setLyricsDirectory: (path: string) =>
    invoke<LibraryScanStatus>("set_lyrics_directory", { path }),
  openLyricsDirectory: () => invoke<void>("open_lyrics_directory"),
  searchLyrics: (
    trackKey: string,
    input: LyricsSearchInput,
    intent: LyricsSearchIntent = "automatic",
  ) => invoke<SearchResponse>("search_lyrics_v2", { trackKey, input, intent }),
  getProviderCatalog: () => invoke<ProviderDescriptor[]>("get_provider_catalog"),
  getProviderSettings: () => invoke<ProviderSettingsView>("get_provider_settings"),
  getProviderCredentials: () => invoke<ProviderCredentialView>("get_provider_credentials"),
  setProviderSettings: (settings: ProviderSettings) =>
    invoke<ProviderSettingsView>("update_provider_policy", { settings }),
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
  ) => {
    const input = {
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
    };
    return manualSelected
      ? invoke<LyricsDocument>("select_lyrics_candidate", { input })
      : invoke<LyricsDocument>("save_lyrics", { input });
  },
  selectLyricsCandidate: (
    input: {
      trackKey: string;
      title: string;
      artist: string;
      album: string | null;
      durationMs: number | null;
      source: string;
      lyrics: string;
      providerId: string | null;
      providerItemId: string | null;
    },
  ) => invoke<LyricsDocument>("select_lyrics_candidate", {
    input: { ...input, manualSelected: true },
  }),
  importLyrics: (trackKey: string, title: string, artist: string, album: string | null, durationMs: number | null, lyrics: string) =>
    invoke<LyricsDocument>("import_local_lyrics", {
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
    invoke<void>("set_lyrics_offset_v2", { trackKey, offsetMs }),
  removeLyricsAssociation: (trackKey: string) =>
    invoke<void>("clear_lyrics_binding", { trackKey }),
  getSongAssociationCandidates: (trackKey: string, platform: string) =>
    invoke<SongAssociationCandidate[]>("get_song_association_candidates", { trackKey, platform }),
  associateSongCandidate: (
    trackKey: string,
    platform: string,
    candidateRecordingId: number,
  ) => invoke<RecordingView>("associate_song_candidate", {
    input: { trackKey, platform, candidateRecordingId },
  }),
  detachPlatformTrack: (
    trackKey: string,
    platform: string,
    lyricsMode: "inheritCurrent" | "unbound",
  ) =>
    invoke<RecordingView>("detach_platform_track", {
      input: { trackKey, platform, lyricsMode },
    }),
};
