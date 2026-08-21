import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { appI18n } from "../features/i18n/i18n";
import { reportFrontendError } from "./debugLog";
export { isTauriRuntime } from "./tauriEvent";
import type {
  AppConfig,
  ConfigExport,
  ConfigDraftValidation,
  ConfigEditorData,
  GlobalShortcutSettings,
  GlobalShortcutStatus,
  LanguagePreference,
  LegalNoticeStatus,
  NativeLanguage,
  NotchLayoutMetrics,
  LyricsDocument,
  LyricsDisplayPreferences,
  LyricsBaseAppearance,
  LyricsModeStyleInheritance,
  LyricsMonitor,
  LyricsStyleMode,
  MusixmatchTokenType,
  LyricsRuntimeSnapshot,
  LibraryScanStatus,
  LyricsSearchInput,
  LyricsSearchResult,
  OverlaySettings,
  OverlayResizeBounds,
  OverlayResizeEdge,
  OverlayStyle,
  PlaybackAction,
  PlaybackArtwork,
  PlaybackSnapshot,
  PlaybackSpectrumState,
  PlayerFollowerServiceState,
  PlayerSelection,
  ProviderSettings,
  ProviderSettingsView,
  ProviderStatus,
  ProviderCredentialView,
  ProviderCredentialUpdate,
  SearchResponse,
  SettingsResetResponse,
  SettingsSection,
  SystemMediaFilterMode,
  ThemePreference,
  ToolbarPlacement,
  RegisteredApplication,
} from "./types";

export type AppErrorCode = `command.${string}` | "config.conflict" | "unknown";

export class AppOperationError extends Error {
  readonly code: AppErrorCode;
  readonly command: string;
  readonly cause: unknown;

  constructor(command: string, cause: unknown) {
    const detail = cause instanceof Error ? cause.message : String(cause);
    super(detail);
    this.name = "AppOperationError";
    this.command = command;
    this.cause = cause;
    this.code = command === "save_app_config_draft" && detail.startsWith("config.conflict:")
      ? "config.conflict"
      : `command.${command}`;
  }
}

function invoke<T>(command: string, args?: Record<string, unknown>) {
  return tauriInvoke<T>(command, args).catch((error) => {
    reportFrontendError(`Tauri command '${command}' failed`, error);
    throw new AppOperationError(command, error);
  });
}

export const api = {
  getLegalNoticeStatus: () => invoke<LegalNoticeStatus>("get_legal_notice_status"),
  acceptLegalNotice: () => invoke<void>("accept_legal_notice"),
  quitApplication: () => invoke<void>("quit_application"),
  getPlayback: () => invoke<PlaybackSnapshot>("get_playback_snapshot"),
  controlPlayback: (action: PlaybackAction) =>
    invoke<void>("control_playback", { action }),
  getPlaybackArtwork: (artworkId: string) =>
    invoke<PlaybackArtwork | null>("get_playback_artwork", { artworkId }),
  startPlaybackSpectrum: () =>
    invoke<PlaybackSpectrumState>("start_playback_spectrum"),
  stopPlaybackSpectrum: () => invoke<void>("stop_playback_spectrum"),
  getPlaybackSpectrumState: () =>
    invoke<PlaybackSpectrumState>("get_playback_spectrum_state"),
  getPlayerSelection: () => invoke<PlayerSelection>("get_player_selection"),
  setPlayerSelection: (selection: PlayerSelection) =>
    invoke<void>("set_player_selection", { selection }),
  getCachedLyrics: (trackKey: string) =>
    invoke<LyricsDocument | null>("get_cached_lyrics", { trackKey }),
  getLyricsRuntimeSnapshot: () =>
    invoke<LyricsRuntimeSnapshot>("get_lyrics_runtime_snapshot"),
  getNotchLayoutMetrics: () => invoke<NotchLayoutMetrics>("get_notch_layout_metrics"),
  getLyricsMonitors: () => invoke<LyricsMonitor[]>("get_lyrics_monitors"),
  getLibraryScanStatus: () => invoke<LibraryScanStatus>("get_library_scan_status"),
  rescanLyricsLibrary: () => invoke<LibraryScanStatus>("rescan_lyrics_library"),
  setLyricsDirectory: (path: string) =>
    invoke<LibraryScanStatus>("set_lyrics_directory", { path }),
  openLyricsDirectory: () => invoke<void>("open_lyrics_directory"),
  searchLyrics: (trackKey: string, input: LyricsSearchInput, force = false) =>
    invoke<SearchResponse>("search_lyrics", { trackKey, input, force }),
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
  getOverlaySettings: () => invoke<OverlaySettings>("get_overlay_settings"),
  setOverlayLocked: (locked: boolean) => invoke<void>("set_overlay_locked", { locked }),
  getOverlayStyle: () => invoke<OverlayStyle>("get_overlay_style"),
  getOverlayToolbarPlacement: () =>
    invoke<ToolbarPlacement>("get_overlay_toolbar_placement"),
  setOverlayStyle: (style: OverlayStyle) =>
    invoke<OverlayStyle>("set_overlay_style", { style }),
  nudgeOverlay: (dx: number, dy: number) => invoke<void>("nudge_overlay", { dx, dy }),
  resetOverlayBounds: () => invoke<OverlayStyle>("reset_overlay_bounds"),
  resizeOverlayEdge: (edge: OverlayResizeEdge, mainSize: number, minimumMainSize: number) =>
    invoke<OverlayResizeBounds>("resize_overlay_edge", { edge, mainSize, minimumMainSize }),
  fitOverlayContent: (width: number, height: number) =>
    invoke<boolean>("fit_overlay_content", { width, height }),
  fitNotchLyricsContent: (width: number, height: number) =>
    invoke<void>("fit_notch_lyrics_content", { width, height }),
  showMainWindow: () => invoke<void>("show_main_window"),
  showLyricsStyleSettings: (mode: LyricsStyleMode) =>
    invoke<void>("show_lyrics_style_settings", { mode }),
  showQuickLyricsWindow: () => invoke<void>("show_quick_lyrics_window"),
  resetSettingsSection: (section: SettingsSection) =>
    invoke<SettingsResetResponse>("reset_settings_section", { section }),
  getAppConfig: () => invoke<AppConfig>("get_app_config"),
  setTheme: (theme: ThemePreference) => invoke<AppConfig>("set_theme", { theme }),
  resolveSystemMediaApplications: (paths: string[]) =>
    invoke<RegisteredApplication[]>("resolve_system_media_applications", { paths }),
  setSystemMediaFilterMode: (mode: SystemMediaFilterMode) =>
    invoke<AppConfig>("set_system_media_filter_mode", { mode }),
  setSystemMediaApplications: (applications: RegisteredApplication[]) =>
    invoke<AppConfig>("set_system_media_applications", { applications }),
  resolvePlayerFollowerApplication: (path: string) =>
    invoke<RegisteredApplication>("resolve_player_follower_application", { path }),
  setPlayerFollowerApplication: (application: RegisteredApplication | null) =>
    invoke<AppConfig>("set_player_follower_application", { application }),
  getPlayerFollowerServiceStatus: () =>
    invoke<PlayerFollowerServiceState>("get_player_follower_service_status"),
  openPlayerFollowerSystemSettings: () =>
    invoke<void>("open_player_follower_system_settings"),
  openAutomationSystemSettings: () => invoke<void>("open_automation_system_settings"),
  getApplicationIcons: (bundleIds: string[]) =>
    invoke<Record<string, string>>("get_application_icons", { bundleIds }),
  resolveApplicationByBundleId: (bundleId: string) =>
    invoke<RegisteredApplication>("resolve_application_by_bundle_id", { bundleId }),
  setLanguage: (language: LanguagePreference) =>
    invoke<AppConfig>("set_language", { language }),
  setNativeLanguage: (language: NativeLanguage) =>
    invoke<void>("set_native_language", { language }),
  getGlobalShortcutStatus: () => invoke<GlobalShortcutStatus>("get_global_shortcut_status"),
  setGlobalShortcuts: (shortcuts: GlobalShortcutSettings) =>
    invoke<AppConfig>("set_global_shortcuts", { shortcuts }),
  setDockIconHidden: (hidden: boolean) =>
    invoke<AppConfig>("set_dock_icon_hidden", { hidden }),
  setSilentStartup: (enabled: boolean) =>
    invoke<AppConfig>("set_silent_startup", { enabled }),
  setAutoCheckUpdates: (enabled: boolean) =>
    invoke<AppConfig>("set_auto_check_updates", { enabled }),
  setOverlayHideWhenNotPlaying: (hidden: boolean) =>
    invoke<AppConfig>("set_overlay_hide_when_not_playing", { hidden }),
  setStatusBarLyricsEnabled: (enabled: boolean) =>
    invoke<AppConfig>("set_status_bar_lyrics_enabled", { enabled }),
  setListLyricsVisible: (visible: boolean) =>
    invoke<AppConfig>("set_list_lyrics_visible", { visible }),
  setListLyricsOptions: (showTranslation: boolean, showRomanization: boolean) =>
    invoke<AppConfig>("set_list_lyrics_options", { showTranslation, showRomanization }),
  setNotchLyricsVisible: (visible: boolean) =>
    invoke<AppConfig>("set_notch_lyrics_visible", { visible }),
  setLyricsDisplayPreferences: <Mode extends Exclude<LyricsStyleMode, "desktop">>(
    mode: Mode,
    preferences: LyricsDisplayPreferences[Mode],
  ) => invoke<AppConfig>("set_lyrics_display_preferences", { mode, preferences }),
  setLyricsBaseAppearance: (appearance: LyricsBaseAppearance) =>
    invoke<AppConfig>("set_lyrics_base_appearance", { appearance }),
  setLyricsStyleInheritance: (mode: LyricsStyleMode, inheritance: LyricsModeStyleInheritance) =>
    invoke<AppConfig>("set_lyrics_style_inheritance", { mode, inheritance }),
  resetLyricsBaseAppearance: () => invoke<AppConfig>("reset_lyrics_base_appearance"),
  resetLyricsStyleMode: (mode: LyricsStyleMode) =>
    invoke<SettingsResetResponse>("reset_lyrics_style_mode", { mode }),
  resetLyricsDisplayPosition: (mode: "statusBar" | "listWindow" | "notch") =>
    invoke<void>("reset_lyrics_display_position", { mode }),
  resetListLyricsWindowSize: () => invoke<void>("reset_list_lyrics_window_size"),
  exportAppConfig: () => invoke<ConfigExport>("export_app_config"),
  revealConfigDirectory: () => invoke<void>("reveal_config_directory"),
  getConfigEditorData: () => invoke<ConfigEditorData>("get_config_editor_data"),
  validateAppConfigDraft: (raw: string) =>
    invoke<ConfigDraftValidation>("validate_app_config_draft", { raw }),
  saveAppConfigDraft: (raw: string, expectedRevision: number) =>
    invoke<AppConfig>("save_app_config_draft", { raw, expectedRevision }),
};

export function messageOf(error: unknown): string {
  if (error instanceof AppOperationError) {
    if (["set_global_shortcuts", "set_provider_settings", "set_system_media_filter_mode", "set_system_media_applications", "resolve_system_media_applications", "resolve_player_follower_application", "set_player_follower_application", "resolve_application_by_bundle_id", "control_playback", "get_playback_artwork", "start_playback_spectrum", "stop_playback_spectrum", "get_playback_spectrum_state"].includes(error.command) && error.message) return error.message;
    return error.code === "config.conflict"
      ? appI18n.t("errors.configConflict")
      : appI18n.t("errors.command");
  }
  return appI18n.t("errors.unknown");
}

export function errorCodeOf(error: unknown): AppErrorCode {
  return error instanceof AppOperationError ? error.code : "unknown";
}

export function trackKeyOf(snapshot: PlaybackSnapshot): string | null {
  const title = snapshot.title?.trim();
  const artist = snapshot.artist?.trim();
  const trackId = snapshot.trackId?.trim();
  if (!snapshot.player || !title || !artist) return null;
  if (trackId) return `${snapshot.player}:${trackId}`;
  const fallback = `${title}|${artist}|${snapshot.durationMs ?? 0}`
    .toLowerCase()
    .replace(/\s+/g, " ")
    .trim();
  return `${snapshot.player}:fallback:${fallback}`;
}
