import type { OverlayFontWeight } from "./base";
import type { OverlayStyle } from "./overlay";

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

export type LyricsLoadStatus = "ready" | "missing" | "error";

export type LyricsLoadResponse = {
  status: LyricsLoadStatus;
  document: LyricsDocument | null;
  error: string | null;
};

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

export type StatusBarAlignment = "left" | "center" | "right";

export type ListLyricsLineKind = "original" | "translation" | "romanization";

export type ListLyricsLineOrder = [
  ListLyricsLineKind,
  ListLyricsLineKind,
  ListLyricsLineKind,
];

export type SupportingLyricsPriority = "translation" | "romanization";

export type CompactLyricsPresentation = {
  layout: "single" | "double";
  doubleLineMode: "rolling" | "alternating";
  showTranslation: boolean;
  showRomanization: boolean;
  supportingPriority: SupportingLyricsPriority;
};

export type DesktopLyricsPresentation = CompactLyricsPresentation & {
  orientation: OverlayStyle["orientation"];
  alignment: OverlayStyle["alignment"];
  primaryLinePosition: OverlayStyle["primaryLinePosition"];
  longText: OverlayStyle["longText"];
  autoCenterWithTranslationOrRomanization: boolean;
};

export type DesktopLyricsAppearance = Omit<
  OverlayStyle,
  | "layout"
  | "doubleLineMode"
  | "orientation"
  | "alignment"
  | "primaryLinePosition"
  | "longText"
  | "secondaryDisplay"
  | "autoCenterWithTranslationOrRomanization"
  | "horizontalMaxWidth"
  | "verticalMaxHeight"
>;

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
  verticalOffset: number;
  fontWeight: OverlayFontWeight;
  secondaryFontWeight: OverlayFontWeight;
  textColor: string;
  inactiveColor: string;
  highlightColor: string;
  translationColor: string;
  romanizationColor: string;
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
  secondaryLineGap: number;
  activeColor: string;
  inactiveColor: string;
  activeOpacity: number;
  inactiveOpacity: number;
  translationColor: string;
  romanizationColor: string;
  activeBackgroundColor: string;
  backgroundColor: string;
  backgroundOpacity: number;
  backgroundMode: "solid" | "transparent";
  textShadowOffsetX: number;
  textShadowOffsetY: number;
  textShadowBlur: number;
  textShadowColor: string;
  textStrokeWidth: number;
  textStrokeColor: string;
  alignment: "left" | "center" | "right";
};

export type NotchLyricsAppearance = {
  fontFamily: string;
  fontSize: number;
  fontWeight: OverlayFontWeight;
  secondaryFontWeight: OverlayFontWeight;
  activeColor: string;
  inactiveColor: string;
  translationColor: string;
  romanizationColor: string;
  karaokeStyle: CompactKaraokeStyle;
  lineGap: number;
  borderRadius: number;
  topBorderRadius: number;
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
  desktop: {
    enabled: boolean;
    locked: boolean;
    hideWhenNotPlaying: boolean;
    presentation: DesktopLyricsPresentation;
    appearance: DesktopLyricsAppearance;
  };
  statusBar: {
    enabled: boolean;
    hideWhenNotPlaying: boolean;
    presentation: CompactLyricsPresentation & { alignment: StatusBarAlignment };
    appearance: StatusBarLyricsAppearance;
  };
  listWindow: {
    enabled: boolean;
    alwaysOnTop: boolean;
    locked: boolean;
    showTranslation: boolean;
    showRomanization: boolean;
    lineOrder: ListLyricsLineOrder;
    appearance: ListLyricsAppearance;
  };
  notch: {
    enabled: boolean;
    hideWhenNotPlaying: boolean;
    monitorId: string | null;
    showLyrics: boolean;
    leftSlot: NotchSlotContent;
    rightSlot: NotchSlotContent;
    presentation: CompactLyricsPresentation;
    inlineLyricsOnNonNotch: boolean;
    appearance: NotchLyricsAppearance;
  };
};

export type DesktopLyricsPreferences = LyricsDisplayPreferences["desktop"];
export type StatusBarLyricsPreferences = LyricsDisplayPreferences["statusBar"];
export type ListLyricsPreferences = LyricsDisplayPreferences["listWindow"];
export type NotchLyricsPreferences = LyricsDisplayPreferences["notch"];
