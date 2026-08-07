import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { reportFrontendError } from "./debugLog";
import type {
  AppConfig,
  ArtworkAsset,
  ConfigExport,
  ConfigDraftValidation,
  ConfigEditorData,
  GlobalShortcutSettings,
  LyricsDocument,
  LibraryPage,
  LibraryPreview,
  LibraryScanStatus,
  LyricsSearchInput,
  LyricsSearchResult,
  OverlaySettings,
  OverlayResizeBounds,
  OverlayResizeEdge,
  OverlayStyle,
  PlaybackSnapshot,
  PlayerKind,
  PlayerSelection,
  ProviderSettings,
  ProviderSettingsView,
  ProviderStatus,
  SearchResponse,
  SettingsResetResponse,
  SettingsSection,
} from "./types";

export function isTauriRuntime() {
  if (typeof window === "undefined") return false;
  const internals = (window as Window & {
    __TAURI_INTERNALS__?: { invoke?: unknown; transformCallback?: unknown };
  }).__TAURI_INTERNALS__;
  return typeof internals?.invoke === "function" && typeof internals.transformCallback === "function";
}

function invoke<T>(command: string, args?: Record<string, unknown>) {
  return tauriInvoke<T>(command, args).catch((error) => {
    reportFrontendError(`Tauri 命令 ${command} 调用失败`, error);
    throw error;
  });
}

export const api = {
  getPlayback: () => invoke<PlaybackSnapshot>("get_playback_snapshot"),
  getTrackArtwork: (player: PlayerKind, trackId: string) =>
    invoke<ArtworkAsset | null>("get_track_artwork", { player, trackId }),
  getPlayerSelection: () => invoke<PlayerSelection>("get_player_selection"),
  setPlayerSelection: (selection: PlayerSelection) =>
    invoke<void>("set_player_selection", { selection }),
  playerAction: (action: "play_pause" | "next" | "previous" | "seek", positionMs?: number) =>
    invoke<void>("player_action", { action, positionMs }),
  getCachedLyrics: (trackKey: string) =>
    invoke<LyricsDocument | null>("get_cached_lyrics", { trackKey }),
  getLibraryPage: (query = "", offset = 0, limit = 100) =>
    invoke<LibraryPage>("get_library_page", { query, offset, limit }),
  getLibraryScanStatus: () => invoke<LibraryScanStatus>("get_library_scan_status"),
  setLyricsDirectory: (path: string) =>
    invoke<LibraryScanStatus>("set_lyrics_directory", { path }),
  rescanLyricsLibrary: () => invoke<LibraryScanStatus>("rescan_lyrics_library"),
  previewLibraryEntry: (path: string) =>
    invoke<LibraryPreview>("preview_library_entry", { path }),
  openLyricsDirectory: () => invoke<void>("open_lyrics_directory"),
  revealLibraryEntry: (path: string) => invoke<void>("reveal_library_entry", { path }),
  searchLyrics: (input: LyricsSearchInput) => invoke<SearchResponse>("search_lyrics", { input }),
  getProviderSettings: () => invoke<ProviderSettingsView>("get_provider_settings"),
  setProviderSettings: (settings: ProviderSettings) =>
    invoke<ProviderSettingsView>("set_provider_settings", { settings }),
  testProvider: (providerId: string) =>
    invoke<ProviderStatus>("test_provider", { providerId }),
  saveLyrics: (
    trackKey: string,
    title: string,
    artist: string,
    result: Pick<LyricsSearchResult, "id" | "providerId" | "source" | "lyrics">,
    manualSelected: boolean,
  ) =>
    invoke<LyricsDocument>("save_lyrics", {
      input: {
        trackKey,
        title,
        artist,
        source: result.source,
        lyrics: result.lyrics,
        providerId: result.providerId,
        providerItemId: result.id,
        manualSelected,
      },
    }),
  importLyrics: (trackKey: string, title: string, artist: string, lyrics: string) =>
    invoke<LyricsDocument>("import_lyrics", {
      input: {
        trackKey,
        title,
        artist,
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
  setOverlayVisible: (visible: boolean) => invoke<void>("set_overlay_visible", { visible }),
  getOverlayVisible: () => invoke<boolean>("get_overlay_visible"),
  getOverlaySettings: () => invoke<OverlaySettings>("get_overlay_settings"),
  setOverlayLocked: (locked: boolean) => invoke<void>("set_overlay_locked", { locked }),
  setOverlayPassthrough: (passthrough: boolean) =>
    invoke<void>("set_overlay_passthrough", { passthrough }),
  getOverlayStyle: () => invoke<OverlayStyle>("get_overlay_style"),
  setOverlayStyle: (style: OverlayStyle) =>
    invoke<OverlayStyle>("set_overlay_style", { style }),
  nudgeOverlay: (dx: number, dy: number) => invoke<void>("nudge_overlay", { dx, dy }),
  resetOverlayBounds: () => invoke<OverlayStyle>("reset_overlay_bounds"),
  resizeOverlayEdge: (edge: OverlayResizeEdge, mainSize: number, minimumMainSize: number) =>
    invoke<OverlayResizeBounds>("resize_overlay_edge", { edge, mainSize, minimumMainSize }),
  fitOverlayContent: (width: number, height: number) =>
    invoke<void>("fit_overlay_content", { width, height }),
  showMainWindow: (page?: "settings") => invoke<void>("show_main_window", { page: page ?? null }),
  showQuickLyricsWindow: () => invoke<void>("show_quick_lyrics_window"),
  resetSettingsSection: (section: SettingsSection) =>
    invoke<SettingsResetResponse>("reset_settings_section", { section }),
  getAppConfig: () => invoke<AppConfig>("get_app_config"),
  setUiFontScale: (scale: number) => invoke<AppConfig>("set_ui_font_scale", { scale }),
  setGlobalShortcuts: (shortcuts: GlobalShortcutSettings) =>
    invoke<AppConfig>("set_global_shortcuts", { shortcuts }),
  setDockIconHidden: (hidden: boolean) =>
    invoke<AppConfig>("set_dock_icon_hidden", { hidden }),
  exportAppConfig: () => invoke<ConfigExport>("export_app_config"),
  importAppConfig: (raw: string, appearanceOnly: boolean) =>
    invoke<AppConfig>("import_app_config", { raw, appearanceOnly }),
  revealConfigDirectory: () => invoke<void>("reveal_config_directory"),
  getConfigEditorData: () => invoke<ConfigEditorData>("get_config_editor_data"),
  validateAppConfigDraft: (raw: string) =>
    invoke<ConfigDraftValidation>("validate_app_config_draft", { raw }),
  saveAppConfigDraft: (raw: string, expectedRevision: number) =>
    invoke<AppConfig>("save_app_config_draft", { raw, expectedRevision }),
};

export function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function trackKeyOf(snapshot: PlaybackSnapshot): string | null {
  if (!snapshot.player || !snapshot.title || !snapshot.artist) return null;
  if (snapshot.trackId) return `${snapshot.player}:${snapshot.trackId}`;
  const fallback = `${snapshot.title}|${snapshot.artist}|${snapshot.durationMs ?? 0}`
    .toLocaleLowerCase()
    .replace(/\s+/g, " ")
    .trim();
  return `${snapshot.player}:fallback:${fallback}`;
}
