import { invoke } from "./core";
import type {
  LibraryScanStatus,
  LibraryRootView,
  LibraryPage,
  LibraryIndexStatus,
  LibrarySongSummary,
  LibrarySongDetail,
  SongSimilarityPair,
  LibraryLyricPage,
  LibraryLyricSimilarityPage,
  LibraryLyricStatus,
  LibraryLyricDetail,
  LibraryArtistSummary,
  LibraryArtistDetail,
  LyricSimilarityGroup,
  UnboundCleanupPreview,
  UnboundCleanupResult,
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
  getLibraryIndexStatus: () =>
    invoke<LibraryIndexStatus>("get_library_index_status"),
  listLibrarySongs: (query = "", page = 1, pageSize = 20) =>
    invoke<LibraryPage<LibrarySongSummary>>("list_library_songs", { query, page, pageSize }),
  getLibrarySong: (recordingId: number) =>
    invoke<LibrarySongDetail>("get_library_song", { recordingId }),
  getLibrarySongCandidates: (recordingId: number) =>
    invoke<SongAssociationCandidate[]>("get_library_song_candidates", { recordingId }),
  analyzeLibrarySongSimilarity: () =>
    invoke<SongSimilarityPair[]>("analyze_library_song_similarity"),
  listLibrarySongSimilarity: (page = 1, pageSize = 20) =>
    invoke<LibraryPage<SongSimilarityPair>>("list_library_song_similarity", { page, pageSize }),
  dismissLibrarySongSimilarity: (leftRecordingId: number, rightRecordingId: number) =>
    invoke<void>("dismiss_library_song_similarity", { leftRecordingId, rightRecordingId }),
  mergeLibrarySong: (recordingId: number, candidateRecordingId: number) =>
    invoke<LibrarySongDetail>("merge_library_song", { input: { recordingId, candidateRecordingId } }),
  splitLibrarySong: (recordingId: number, observationId: number, inheritCurrentLyrics = false) =>
    invoke<LibrarySongDetail>("split_library_song", {
      input: { recordingId, observationId, inheritCurrentLyrics },
    }),
  listLibraryLyrics: (
    query = "",
    status: LibraryLyricStatus | null = null,
    sourceKind: string | null = null,
    page = 1,
    pageSize = 20,
  ) => invoke<LibraryLyricPage>("list_library_lyrics", {
    query, status, sourceKind, page, pageSize,
  }),
  getLibraryLyric: (assetId: number) =>
    invoke<LibraryLyricDetail>("get_library_lyric", { assetId }),
  bindLibraryLyric: (recordingId: number, assetId: number, replaceDefault = false) =>
    invoke<LibrarySongDetail>("bind_library_lyric", {
      input: { recordingId, assetId, replaceDefault },
    }),
  unbindLibraryLyric: (recordingId: number, assetId: number) =>
    invoke<LibrarySongDetail>("unbind_library_lyric", { input: { recordingId, assetId } }),
  listLibraryArtists: (query = "", page = 1, pageSize = 20) =>
    invoke<LibraryPage<LibraryArtistSummary>>("list_library_artists", { query, page, pageSize }),
  getLibraryArtist: (artistId: number) =>
    invoke<LibraryArtistDetail>("get_library_artist", { artistId }),
  updateLibraryArtistName: (artistId: number, name: string) =>
    invoke<LibraryArtistDetail>("update_library_artist_name", { input: { artistId, name } }),
  setLibraryArtistAlias: (artistId: number, alias: string, confirmed: boolean) =>
    invoke<LibraryArtistDetail>("set_library_artist_alias", { input: { artistId, alias, confirmed } }),
  analyzeLibraryLyricSimilarity: () =>
    invoke<LyricSimilarityGroup[]>("analyze_library_lyric_similarity"),
  listLibraryLyricSimilarity: (page = 1, pageSize = 20) =>
    invoke<LibraryLyricSimilarityPage>("list_library_lyric_similarity", { page, pageSize }),
  dismissLibraryLyricSimilarity: (assetIds: number[]) =>
    invoke<void>("dismiss_library_lyric_similarity", { assetIds }),
  mergeLibraryLyrics: (keeperAssetId: number, redundantAssetIds: number[]) =>
    invoke<void>("merge_library_lyrics", {
      input: { keeperAssetId, redundantAssetIds },
    }),
  previewUnboundLyricsCleanup: (page = 1, pageSize = 20) =>
    invoke<UnboundCleanupPreview>("preview_unbound_lyrics_cleanup", { page, pageSize }),
  cleanupUnboundLyrics: (
    selectionMode: "selected" | "allExcept",
    assetIds: number[],
    excludedAssetIds: number[],
    revision: string,
  ) => invoke<UnboundCleanupResult>("cleanup_unbound_lyrics", {
    selectionMode, assetIds, excludedAssetIds, revision,
  }),
  deleteLibraryLyricSource: (assetId: number, sourceId: number) =>
    invoke<LibraryLyricDetail>("delete_library_lyric_source", { assetId, sourceId }),
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
  parseLyricsPreview: (source: string, lyrics: string) =>
    invoke<LyricsDocument>("parse_lyrics_preview", { source, lyrics }),
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
