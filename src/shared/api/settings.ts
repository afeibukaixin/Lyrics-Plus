import { invoke } from "./core";
import type {
  AppConfig,
  ConfigDraftValidation,
  ConfigEditorData,
  ConfigExport,
  LyricsBaseAppearance,
  LyricsDisplayPreferences,
  LyricsModeStyleInheritance,
  LyricsStyleMode,
  SettingsSection,
  SettingsResetResponse,
 } from "./types";

export const settingsApi = {
  resetSettingsSection: (section: SettingsSection) =>
    invoke<SettingsResetResponse>("reset_settings_section", { section }),
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
