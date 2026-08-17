import type { SupportedLanguage } from "./languages";

export type { SupportedLanguage } from "./languages";

export type PlayerKind = "apple_music" | "spotify" | "system";
export type PlayerSelection = "auto" | PlayerKind;
export type SystemMediaFilterMode = "allowlist" | "blocklist";
export type PlaybackErrorCode =
  | "waiting"
  | "not_installed"
  | "automation_denied"
  | "response_timeout"
  | "invalid_response"
  | "multiple_playing"
  | "no_unique_player"
  | "source_not_allowed"
  | "unavailable";

export type RegisteredApplication = {
  name: string;
  bundleId: string;
};

export type PlayerFollowerServiceState =
  | "development"
  | "unsupported"
  | "not_registered"
  | "enabled"
  | "requires_approval"
  | "not_found";

export type GlobalShortcutSettings = {
  toggleOverlay: string;
  unlockOverlay: string;
  resetOverlay: string;
  toggleStatusBarLyrics: string;
  toggleListLyrics: string;
  toggleNotchLyrics: string;
};

export type GlobalShortcutStatus = Record<keyof GlobalShortcutSettings, boolean>;

export type LegalNoticeStatus = {
  currentVersion: number;
  accepted: boolean;
};

export const defaultGlobalShortcuts: GlobalShortcutSettings = {
  toggleOverlay: "CommandOrControl+Shift+KeyL",
  unlockOverlay: "CommandOrControl+Shift+KeyU",
  resetOverlay: "CommandOrControl+Shift+Digit0",
  toggleStatusBarLyrics: "",
  toggleListLyrics: "",
  toggleNotchLyrics: "",
};

export type OverlaySettings = {
  visible: boolean;
  locked: boolean;
};

export type OverlayResizeEdge = "left" | "right" | "top" | "bottom";

export type OverlayResizeBounds = {
  width: number;
  height: number;
};

export type PlaybackSnapshot = {
  player: PlayerKind | null;
  isRunning: boolean;
  isPlaying: boolean;
  trackId: string | null;
  title: string | null;
  artist: string | null;
  album: string | null;
  sourceAppName: string | null;
  sourceAppBundleId: string | null;
  durationMs: number | null;
  positionMs: number | null;
  observedAtMs: number;
  errorCode: PlaybackErrorCode | null;
  error: string | null;
};

export type LyricsLine = {
  startMs: number;
  endMs: number | null;
  text: string;
  words: LyricsWord[] | null;
};

export type LyricsWord = {
  startMs: number;
  endMs: number;
  text: string;
};

export type LyricsTrack = {
  lines: LyricsLine[];
};

export type LyricsDocument = {
  metadata: {
    title: string | null;
    artist: string | null;
    album: string | null;
    source: string;
    originalFormat: string;
    manualSelected: boolean;
  };
  tracks: {
    original: LyricsTrack;
    translation: LyricsTrack | null;
    romanization: LyricsTrack | null;
  };
  offsetMs: number;
  raw: string;
};

export type LyricsRuntimeStatus = "idle" | "loading" | "ready" | "not_found" | "error";

export type LyricsRuntimeSnapshot = {
  trackKey: string | null;
  document: LyricsDocument | null;
  status: LyricsRuntimeStatus;
  error: string | null;
};

export type NotchLayoutMetrics = {
  hasNotch: boolean;
  topInset: number;
  centerGapWidth: number;
};

export type LyricsStyleMode = "desktop" | "statusBar" | "listWindow" | "notch";

export type LyricsBaseAppearance = {
  fontFamily: string;
  activeColor: string;
  inactiveColor: string;
  translationColor: string;
  romanizationColor: string;
  supportingColor: string;
  backgroundColor: string;
};

export type LyricsModeStyleInheritance = {
  inheritFontFamily: boolean;
  inheritColors: boolean;
};

export type LyricsStyleInheritance = Record<LyricsStyleMode, LyricsModeStyleInheritance>;

export type StatusBarLyricsAppearance = {
  fontFamily: string;
  fontSize: number;
  fontWeight: OverlayFontWeight;
  textColor: string;
  inactiveColor: string;
  highlightColor: string;
  width: number;
};

export type ListLyricsAppearance = {
  fontFamily: string;
  fontSize: number;
  fontWeight: OverlayFontWeight;
  secondaryFontScale: number;
  lineHeight: number;
  lineGap: number;
  activeColor: string;
  inactiveColor: string;
  translationColor: string;
  romanizationColor: string;
  activeBackgroundColor: string;
  backgroundColor: string;
  backgroundOpacity: number;
  backgroundMode: "solid" | "transparent";
  alignment: "left" | "center" | "right";
};

export type NotchLyricsAppearance = {
  fontFamily: string;
  fontSize: number;
  fontWeight: OverlayFontWeight;
  activeColor: string;
  inactiveColor: string;
  translationColor: string;
  romanizationColor: string;
  borderRadius: number;
  maxWidth: number;
};

export type LyricsMonitor = {
  id: string;
  name: string;
  width: number;
  height: number;
  isPrimary: boolean;
};

export type LyricsDisplayPreferences = {
  statusBar: {
    enabled: boolean;
    hideWhenNotPlaying: boolean;
    appearance: StatusBarLyricsAppearance;
  };
  listWindow: {
    enabled: boolean;
    alwaysOnTop: boolean;
    showTranslation: boolean;
    showRomanization: boolean;
    appearance: ListLyricsAppearance;
  };
  notch: {
    enabled: boolean;
    hideWhenNotPlaying: boolean;
    monitorId: string | null;
    showTwoLines: boolean;
    showTranslation: boolean;
    showRomanization: boolean;
    appearance: NotchLyricsAppearance;
  };
};

export type StatusBarLyricsPreferences = LyricsDisplayPreferences["statusBar"];
export type ListLyricsPreferences = LyricsDisplayPreferences["listWindow"];
export type NotchLyricsPreferences = LyricsDisplayPreferences["notch"];

export type LyricsSearchResult = {
  id: string;
  providerId: string;
  title: string;
  artist: string;
  album: string | null;
  durationMs: number | null;
  source: string;
  synced: boolean;
  hasTranslation: boolean;
  hasWordTiming: boolean;
  hasRomanization: boolean;
  score: number;
  lyrics: string;
};

export type SearchResponse = {
  autoApply: boolean;
  results: LyricsSearchResult[];
  providerStatuses: ProviderStatus[];
};

export type ProviderOrderMode = "smart" | "strict";

export type ProviderPreference = {
  id: string;
  enabled: boolean;
};

export type ProviderSettings = {
  mode: ProviderOrderMode;
  providers: ProviderPreference[];
  autoApplyThreshold: number;
  titleFilterKeywords: string[];
  amllBaseUrl: string;
};

export type ProviderCredentialView = {
  musixmatchConfigured: boolean;
  musixmatchTokenType: MusixmatchTokenType | null;
};

export type MusixmatchTokenType = "desktopUserToken" | "developerApiKey";

export type ProviderCredentialUpdate = {
  credentials: ProviderCredentialView;
  providerView: ProviderSettingsView;
};

export type ProviderHealth = "unknown" | "available" | "degraded" | "unavailable";

export type ProviderStatus = {
  providerId: string;
  name: string;
  health: ProviderHealth;
  message: string | null;
  checkedAtMs: number | null;
};

export type ProviderSettingsView = {
  settings: ProviderSettings;
  statuses: ProviderStatus[];
};

export type SettingsSection = "style" | "lyrics" | "player" | "application" | "about";
export type LanguagePreference = "system" | SupportedLanguage;
export type ThemePreference = "system" | "light" | "dark";
export type NativeLanguage = "zh-CN" | "en-US";

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
    silentStartup: boolean;
    autoCheckUpdates: boolean;
    shortcuts: GlobalShortcutSettings;
  };
  lyrics: {
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

export type OverlayFontWeight = 400 | 500 | 600 | 700 | 800;

export type OverlayStyle = {
  fontFamily: string;
  fontSize: number;
  fontWeight: OverlayFontWeight;
  secondaryFontWeight: OverlayFontWeight;
  lineHeight: number;
  activeColor: string;
  inactiveColor: string;
  opacity: number;
  backgroundOpacity: number;
  backgroundBlur: number;
  backgroundRadius: number;
  backgroundPaddingX: number;
  backgroundPaddingY: number;
  backgroundMode: "solid" | "transparent";
  background: "glass" | "transparent" | "solid";
  solidColor: string;
  layout: "single" | "double";
  orientation: "horizontal" | "vertical";
  alignment: "center" | "distributed";
  longText: "shrink" | "wrap" | "marquee";
  secondaryDisplay: "next" | "translation" | "romanization" | "translation_romanization";
  autoCenterWithTranslationOrRomanization: boolean;
  karaokeStyle: "sweep" | "bounce" | "highlight";
  secondaryFontScale: number;
  translationFontScale: number;
  romanizationFontScale: number;
  translationColor: string;
  romanizationColor: string;
  textShadowOffsetX: number;
  textShadowOffsetY: number;
  textShadowBlur: number;
  textShadowColor: string;
  horizontalMaxWidth: number | null;
  verticalMaxHeight: number | null;
};

export type ToolbarPlacement = "top" | "bottom" | "left" | "right";

export function secondaryDisplayFlags(mode: OverlayStyle["secondaryDisplay"]) {
  return {
    translation: mode === "translation" || mode === "translation_romanization",
    romanization: mode === "romanization" || mode === "translation_romanization",
  };
}

export function secondaryDisplayFromFlags(translation: boolean, romanization: boolean): OverlayStyle["secondaryDisplay"] {
  if (translation && romanization) return "translation_romanization";
  if (translation) return "translation";
  if (romanization) return "romanization";
  return "next";
}

export const defaultOverlayStyle: OverlayStyle = {
  fontFamily: 'Inter, "SF Pro Text", "SF Pro Display", -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Noto Sans CJK SC", "Noto Sans SC", Arial, sans-serif',
  fontSize: 36,
  fontWeight: 800,
  secondaryFontWeight: 500,
  lineHeight: 1.2,
  activeColor: "#a3e635",
  inactiveColor: "#ecfccb",
  opacity: 1,
  backgroundOpacity: 0.6,
  backgroundBlur: 18,
  backgroundRadius: 18,
  backgroundPaddingX: 26,
  backgroundPaddingY: 22,
  backgroundMode: "solid",
  background: "glass",
  solidColor: "#171821",
  layout: "single",
  orientation: "horizontal",
  alignment: "center",
  longText: "marquee",
  secondaryDisplay: "translation_romanization",
  autoCenterWithTranslationOrRomanization: false,
  karaokeStyle: "sweep",
  secondaryFontScale: 1,
  translationFontScale: 0.8,
  romanizationFontScale: 0.8,
  translationColor: "#d9f99d",
  romanizationColor: "#bef264",
  textShadowOffsetX: 0,
  textShadowOffsetY: 1,
  textShadowBlur: 4,
  textShadowColor: "rgba(0, 0, 0, 0.55)",
  horizontalMaxWidth: null,
  verticalMaxHeight: null,
};

export const defaultLyricsBaseAppearance: LyricsBaseAppearance = {
  fontFamily: defaultOverlayStyle.fontFamily,
  activeColor: "#a3e635",
  inactiveColor: "#ecfccb",
  translationColor: "#d9f99d",
  romanizationColor: "#bef264",
  supportingColor: "#94a3b8",
  backgroundColor: "#171821",
};

export const defaultLyricsStyleInheritance: LyricsStyleInheritance = {
  desktop: { inheritFontFamily: true, inheritColors: true },
  statusBar: { inheritFontFamily: true, inheritColors: true },
  listWindow: { inheritFontFamily: true, inheritColors: true },
  notch: { inheritFontFamily: true, inheritColors: true },
};

export const defaultStatusBarLyricsAppearance: StatusBarLyricsAppearance = {
  fontFamily: defaultOverlayStyle.fontFamily,
  fontSize: 14,
  fontWeight: 600,
  textColor: "#a3e635",
  inactiveColor: "#ecfccb",
  highlightColor: "#a3e635",
  width: 220,
};

export const defaultListLyricsAppearance: ListLyricsAppearance = {
  fontFamily: defaultOverlayStyle.fontFamily,
  fontSize: 24,
  fontWeight: 600,
  secondaryFontScale: 0.58,
  lineHeight: 1.45,
  lineGap: 8,
  activeColor: "#a3e635",
  inactiveColor: "#ecfccb",
  translationColor: "#d9f99d",
  romanizationColor: "#bef264",
  activeBackgroundColor: "rgba(148, 163, 184, 0.14)",
  backgroundColor: "#171821",
  backgroundOpacity: 1,
  backgroundMode: "solid",
  alignment: "center",
};

export const defaultNotchLyricsAppearance: NotchLyricsAppearance = {
  fontFamily: defaultOverlayStyle.fontFamily,
  fontSize: 18,
  fontWeight: 700,
  activeColor: defaultOverlayStyle.activeColor,
  inactiveColor: defaultOverlayStyle.inactiveColor,
  translationColor: defaultOverlayStyle.translationColor,
  romanizationColor: defaultOverlayStyle.romanizationColor,
  borderRadius: 22,
  maxWidth: 640,
};
