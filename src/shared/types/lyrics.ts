import type { OverlayFontWeight } from "./base";

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

export type CompactKaraokeStyle = "sweep" | "highlight";

export type NotchSlotContent = "empty" | "title" | "artist" | "artwork" | "spectrum";

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
  karaokeStyle: CompactKaraokeStyle;
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
  karaokeStyle: CompactKaraokeStyle;
  borderRadius: number;
  maxWidth: number;
  expandedMaxWidth: number;
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
    showLyrics: boolean;
    leftSlot: NotchSlotContent;
    rightSlot: NotchSlotContent;
    layout: "single" | "double";
    showTranslation: boolean;
    showRomanization: boolean;
    appearance: NotchLyricsAppearance;
  };
};

export type StatusBarLyricsPreferences = LyricsDisplayPreferences["statusBar"];
export type ListLyricsPreferences = LyricsDisplayPreferences["listWindow"];
export type NotchLyricsPreferences = LyricsDisplayPreferences["notch"];
