import type { SupportedLanguage } from "./base";
import type { GlobalShortcutSettings, RegisteredApplication, PlayerKind, PlayerSelection, SystemMediaFilterMode } from "./player";
import type { LyricsBaseAppearance, LyricsDisplayPreferences, LyricsStyleInheritance } from "./lyrics";
import type { ProviderSettings, ProviderSettingsView } from "./provider";
import type { OverlaySettings, OverlayStyle } from "./overlay";

export type SettingsSection = "style" | "lyrics" | "player" | "application" | "about";
export type LanguagePreference = "system" | SupportedLanguage;
export type ThemePreference = "system" | "light" | "dark";
export type NativeLanguage = "zh-CN" | "en-US" | "ja-JP" | "ko-KR";
export type ChineseConversion = "original" | "simplified" | "traditional";

export type SettingsResetResponse = {
  overlaySettings: OverlaySettings;
  overlayStyle: OverlayStyle;
  providerView: ProviderSettingsView;
  playerSelection: PlayerSelection;
};

export type TelemetrySettings = {
  enabled: boolean;
};

export type AppConfig = {
  schemaVersion: number;
  app: {
    theme: ThemePreference;
    uiFontFamily: string | null;
    language: string;
    playerSelection: PlayerSelection;
    systemMediaFilterMode: SystemMediaFilterMode;
    systemMediaApplications: RegisteredApplication[];
    playerFollowerApplication: RegisteredApplication | null;
    hideDockIcon: boolean;
    hideMenuBarIcon: boolean;
    silentStartup: boolean;
    autoCheckUpdates: boolean;
    lyricsWindowsShowOnAllSpaces: boolean;
    shortcuts: GlobalShortcutSettings;
  };
  lyrics: {
    chineseConversion: ChineseConversion;
    repairSimplifiedJapanese: boolean;
    providers: ProviderSettings;
    displays: LyricsDisplayPreferences;
    baseAppearance: LyricsBaseAppearance;
    styleInheritance: LyricsStyleInheritance;
  };
};

export type ConfigExport = {
  fileName: string;
  raw: string;
};

export type ConfigDraftError = {
  message: string;
  line: number;
  column: number;
};

export type ConfigDraftValidation = {
  valid: boolean;
  error: ConfigDraftError | null;
  normalizedJson: string | null;
  effectiveConfig: AppConfig;
};

export type ConfigEditorData = {
  defaultJsonc: string;
  userJson: string;
  revision: number;
  validation: ConfigDraftValidation;
};

export type LyricsSearchInput = {
  title: string;
  artist: string;
  album: string | null;
  durationMs: number | null;
  platform?: PlayerKind | null;
  platformItemId?: string | null;
};

export type LyricsSearchIntent = "automatic" | "refresh" | "manual";

export type LibraryScanPhase =
  | "idle"
  | "discovering"
  | "indexing"
  | "completed"
  | "failed";

export type LibraryScanStatus = {
  scanId: number;
  libraryDir: string;
  phase: LibraryScanPhase;
  discovered: number;
  processed: number;
  total: number | null;
  skipped: number;
  added: number;
  updated: number;
  unchanged: number;
  removed: number;
  failed: number;
  firstFailure: string | null;
  error: string | null;
};
