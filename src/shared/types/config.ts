import type { SupportedLanguage } from "./base";
import type { GlobalShortcutSettings, RegisteredApplication, PlayerSelection, SystemMediaFilterMode } from "./player";
import type { LyricsBaseAppearance, LyricsDisplayPreferences, LyricsStyleInheritance } from "./lyrics";
import type { ProviderSettings, ProviderSettingsView } from "./provider";
import type { OverlaySettings, OverlayStyle } from "./overlay";

export type SettingsSection = "style" | "lyrics" | "player" | "application" | "about";
export type LanguagePreference = "system" | SupportedLanguage;
export type ThemePreference = "system" | "light" | "dark";
export type NativeLanguage = "zh-CN" | "en-US";
export type ChineseConversion = "original" | "simplified" | "traditional";

export type SettingsResetResponse = {
  overlaySettings: OverlaySettings;
  overlayStyle: OverlayStyle;
  providerView: ProviderSettingsView;
  playerSelection: PlayerSelection;
};

export type OverlayAppearance = Omit<OverlayStyle, "horizontalMaxWidth" | "verticalMaxHeight">;

export type AppConfig = {
  schemaVersion: number;
  app: {
    theme: ThemePreference;
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
    providers: ProviderSettings;
    displays: LyricsDisplayPreferences;
    baseAppearance: LyricsBaseAppearance;
    styleInheritance: LyricsStyleInheritance;
  };
  overlay: {
    visible: boolean;
    locked: boolean;
    hideWhenNotPlaying: boolean;
    appearance: OverlayAppearance;
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
