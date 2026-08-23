import { invoke } from "./core";
import type {
  AppConfig,
  GlobalShortcutSettings,
  GlobalShortcutStatus,
  LanguagePreference,
  LyricsStyleMode,
  NativeLanguage,
  PlayerFollowerServiceState,
  RegisteredApplication,
  SystemMediaFilterMode,
  ThemePreference,
} from "./types";

export const applicationApi = {
  showMainWindow: () => invoke<void>("show_main_window"),
  showLyricsStyleSettings: (mode: LyricsStyleMode) =>
    invoke<void>("show_lyrics_style_settings", { mode }),
  showQuickLyricsWindow: () => invoke<void>("show_quick_lyrics_window"),
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
};
