import type {
  ListLyricsAppearance,
  LyricsBaseAppearance,
  LyricsStyleInheritance,
  NotchLyricsAppearance,
  StatusBarLyricsAppearance,
 } from "./lyrics";
import type { OverlayStyle } from "./overlay";

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
  doubleLineMode: "rolling",
  orientation: "horizontal",
  alignment: "center",
  primaryLinePosition: "first",
  lineGap: 8,
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
  textStrokeWidth: 0,
  textStrokeColor: "#000000",
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
  verticalOffset: 0,
  fontWeight: 600,
  secondaryFontWeight: 500,
  textColor: "#a3e635",
  inactiveColor: "#ecfccb",
  highlightColor: "#a3e635",
  translationColor: "#d9f99d",
  romanizationColor: "#bef264",
  karaokeStyle: "sweep",
  alignment: "left",
  width: 220,
};

export const defaultListLyricsAppearance: ListLyricsAppearance = {
  fontFamily: defaultOverlayStyle.fontFamily,
  fontSize: 24,
  fontWeight: 600,
  secondaryFontScale: 0.58,
  lineHeight: 1.45,
  lineGap: 8,
  secondaryLineGap: 5,
  activeColor: "#a3e635",
  inactiveColor: "#ecfccb",
  activeOpacity: 1,
  inactiveOpacity: 0.58,
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
  secondaryFontWeight: 500,
  activeColor: defaultOverlayStyle.activeColor,
  inactiveColor: defaultOverlayStyle.inactiveColor,
  translationColor: defaultOverlayStyle.translationColor,
  romanizationColor: defaultOverlayStyle.romanizationColor,
  karaokeStyle: "sweep",
  lineGap: 8,
  borderRadius: 12,
  topBorderRadius: 12,
  maxWidth: 320,
  expandedMaxWidth: 440,
};
