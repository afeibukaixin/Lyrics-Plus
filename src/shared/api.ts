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
  LyricsDocument,
  LibraryScanStatus,
  LyricsSearchInput,
  LyricsSearchResult,
  OverlaySettings,
  OverlayResizeBounds,
  OverlayResizeEdge,
  OverlayStyle,
  PlaybackSnapshot,
  PlayerFollowerServiceState,
  PlayerSelection,
  ProviderSettings,
  ProviderSettingsView,
  ProviderStatus,
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
  getPlayerSelection: () => invoke<PlayerSelection>("get_player_selection"),
  setPlayerSelection: (selection: PlayerSelection) =>
    invoke<void>("set_player_selection", { selection }),
  getCachedLyrics: (trackKey: string) =>
    invoke<LyricsDocument | null>("get_cached_lyrics", { trackKey }),
  getLibraryScanStatus: () => invoke<LibraryScanStatus>("get_library_scan_status"),
  setLyricsDirectory: (path: string) =>
    invoke<LibraryScanStatus>("set_lyrics_directory", { path }),
  openLyricsDirectory: () => invoke<void>("open_lyrics_directory"),
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
  showMainWindow: () => invoke<void>("show_main_window"),
  showQuickLyricsWindow: () => invoke<void>("show_quick_lyrics_window"),
  resetSettingsSection: (section: SettingsSection) =>
    invoke<SettingsResetResponse>("reset_settings_section", { section }),
  getAppConfig: () => invoke<AppConfig>("get_app_config"),
  setUiFontScale: (scale: number) => invoke<AppConfig>("set_ui_font_scale", { scale }),
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
    if (["set_global_shortcuts", "set_provider_settings", "set_system_media_filter_mode", "set_system_media_applications", "resolve_system_media_applications", "resolve_player_follower_application", "set_player_follower_application", "resolve_application_by_bundle_id"].includes(error.command) && error.message) return error.message;
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
  if (!snapshot.player || !snapshot.title || !snapshot.artist) return null;
  if (snapshot.trackId) return `${snapshot.player}:${snapshot.trackId}`;
  const fallback = `${snapshot.title}|${snapshot.artist}|${snapshot.durationMs ?? 0}`
    .toLocaleLowerCase()
    .replace(/\s+/g, " ")
    .trim();
  return `${snapshot.player}:fallback:${fallback}`;
}
