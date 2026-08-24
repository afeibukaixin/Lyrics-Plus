import type { NotchLayoutMetrics } from "../../shared/types";

export const NOTCH_MAX_WIDTH = 640;
export const COLLAPSED_HEIGHT_FALLBACK = 30;
export const EXPANDED_HEIGHT_FALLBACK = 180;
export const NON_NOTCH_TOP_INSET_FALLBACK = 30;

export function resolvedNotchTopInset(layout: NotchLayoutMetrics) {
  return Number.isFinite(layout.topInset) && layout.topInset > 0
    ? layout.topInset
    : NON_NOTCH_TOP_INSET_FALLBACK;
}

export function notchCollapsedHeightFloor(layout: NotchLayoutMetrics) {
  const topInset = resolvedNotchTopInset(layout);
  return layout.hasNotch ? Math.max(COLLAPSED_HEIGHT_FALLBACK, topInset) : topInset;
}

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
  const topRadius = "0px";
  // GSAP 全程使用分角属性，避免屏幕类型切换或动画往返时残留简写圆角。
  return {
    borderTopLeftRadius: topRadius,
    borderTopRightRadius: topRadius,
    borderBottomRightRadius: radius,
    borderBottomLeftRadius: radius,
  };
}
