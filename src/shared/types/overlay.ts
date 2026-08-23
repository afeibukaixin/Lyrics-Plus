import type { OverlayFontWeight } from "./base";

export type OverlaySettings = {
  visible: boolean;
  locked: boolean;
};

export type OverlayResizeEdge = "left" | "right" | "top" | "bottom";

export type OverlayResizeBounds = {
  width: number;
  height: number;
};
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
