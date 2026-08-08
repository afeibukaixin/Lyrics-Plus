import type { SupportedLanguage } from "./languages";

export type { SupportedLanguage } from "./languages";

export type PlayerKind = "apple_music" | "spotify";
export type PlayerSelection = "auto" | PlayerKind;
export type PlaybackErrorCode =
  | "waiting"
  | "not_installed"
  | "automation_denied"
  | "response_timeout"
  | "invalid_response"
  | "multiple_playing"
  | "no_unique_player"
  | "unavailable";

export type GlobalShortcutSettings = {
  toggleOverlay: string;
  unlockOverlay: string;
  resetOverlay: string;
};

export const defaultGlobalShortcuts: GlobalShortcutSettings = {
  toggleOverlay: "CommandOrControl+Shift+KeyL",
  unlockOverlay: "CommandOrControl+Shift+KeyU",
  resetOverlay: "CommandOrControl+Shift+Digit0",
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
  durationMs: number | null;
  positionMs: number | null;
  canSeek: boolean;
  observedAtMs: number;
  errorCode: PlaybackErrorCode | null;
  error: string | null;
};

export type ArtworkAsset = {
  player: PlayerKind;
  trackId: string;
  filePath: string;
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

export type SettingsSection = "overlay" | "lyrics" | "app";
export type LanguagePreference = "system" | SupportedLanguage;
export type NativeLanguage = "zh-CN" | "en-US";

export type SettingsResetResponse = {
  overlaySettings: OverlaySettings;
  overlayStyle: OverlayStyle;
  providerView: ProviderSettingsView;
  playerSelection: PlayerSelection;
  uiFontScale: number;
};

export type OverlayAppearance = Omit<OverlayStyle, "horizontalMaxWidth" | "verticalMaxHeight">;

export type AppConfig = {
  schemaVersion: number;
  app: {
    uiFontScale: number;
    language: string;
    playerSelection: PlayerSelection;
    hideDockIcon: boolean;
    shortcuts: GlobalShortcutSettings;
  };
  lyrics: {
    providers: ProviderSettings;
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

export type LibraryEntry = {
  path: string;
  fileName: string;
  title: string;
  artist: string;
  source: string;
  format: string;
  durationMs: number | null;
  fileSize: number;
  modifiedAtMs: number | null;
  duplicateCount: number;
  associationCount: number;
  hasTranslation: boolean;
  hasWordTiming: boolean;
  hasRomanization: boolean;
};

export type LibraryPage = {
  libraryDir: string;
  entries: LibraryEntry[];
  totalCount: number;
  offset: number;
  limit: number;
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
  error: string | null;
};

export type LibraryPreview = {
  entry: LibraryEntry;
  raw: string;
  document: LyricsDocument | null;
};

export type OverlayStyle = {
  fontSize: number;
  activeColor: string;
  inactiveColor: string;
  opacity: number;
  backgroundOpacity: number;
  backgroundBlur: number;
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
  horizontalMaxWidth: number | null;
  verticalMaxHeight: number | null;
};

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
  fontSize: 36,
  activeColor: "#c4b5fd",
  inactiveColor: "#c8d2df",
  opacity: 1,
  backgroundOpacity: 0.6,
  backgroundBlur: 18,
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
  secondaryFontScale: 0.8,
  translationFontScale: 0.8,
  romanizationFontScale: 0.8,
  translationColor: "#cbd5e1",
  romanizationColor: "#aab7c8",
  horizontalMaxWidth: null,
  verticalMaxHeight: null,
};
