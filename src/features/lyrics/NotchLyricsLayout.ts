import type { NotchLayoutMetrics } from "../../shared/types";

export const NOTCH_MAX_WIDTH = 640;
export const COLLAPSED_HEIGHT_FALLBACK = 30;
export const EXPANDED_HEIGHT_FALLBACK = 180;

export function notchSlotPadding(borderRadius: number) {
  const radius = Number.isFinite(borderRadius)
    ? Math.min(40, Math.max(0, borderRadius))
    : 0;
  return 8 + radius * 0.3;
}

export const emptyLayout: NotchLayoutMetrics = {
  hasNotch: false,
  topInset: 0,
  centerGapWidth: 0,
};

export type NotchWidthPreviewValues = {
  maxWidth: number;
  expandedMaxWidth: number;
};

export type NotchWindowFitRequest = {
  key: string;
  ready: Promise<boolean>;
  cancel: () => void;
};

export type IslandState = "collapsed" | "expanding" | "expanded" | "collapsing";

export type IslandDimensions = {
  collapsedWidth: number;
  collapsedHeight: number;
  expandedWidth: number;
  expandedHeight: number;
};

export function physicalSizeMatches(
  actual: { width: number; height: number },
  expected: { physicalWidth: number; physicalHeight: number },
) {
  return Math.abs(actual.width - expected.physicalWidth) <= 1
    && Math.abs(actual.height - expected.physicalHeight) <= 1;
}

export function waitForWebviewLayout() {
  return new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
}

export function islandRadii(hasNotch: boolean, borderRadius: number, expanded: boolean) {
  const radius = `${borderRadius + (hasNotch && expanded ? 4 : 0)}px`;
  return hasNotch
    ? {
      borderTopLeftRadius: "0px",
      borderTopRightRadius: "0px",
      borderBottomRightRadius: radius,
      borderBottomLeftRadius: radius,
    }
    : { borderRadius: `${borderRadius}px` };
}
