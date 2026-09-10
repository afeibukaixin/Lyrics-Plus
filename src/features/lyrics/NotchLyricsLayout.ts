import type { NotchLayoutMetrics } from "../../shared/types";

export const NOTCH_MAX_WIDTH = 640;
export const NOTCH_TOP_CORNER_MAX_RADIUS = 15;
export const NOTCH_DEFAULT_BORDER_RADIUS_MAX = 20;
export const NOTCH_NON_NOTCH_MIN_WIDTH = 270;
export const NOTCH_NATIVE_MIN_WIDTH = 120;
export const COLLAPSED_HEIGHT_FALLBACK = 30;
export const EXPANDED_HEIGHT_FALLBACK = 180;
export const NON_NOTCH_TOP_INSET_FALLBACK = 30;

const NOTCH_SLOT_MIN_HORIZONTAL_PADDING = 4;
const NOTCH_SLOT_MAX_WIDTH = 30;
const NOTCH_SLOT_RADIUS_ANCHOR = 12;
const NOTCH_SLOT_RADIUS_MAX = NOTCH_DEFAULT_BORDER_RADIUS_MAX;
const NOTCH_CORNER_DIAGONAL_INSET = 1 - Math.SQRT1_2;
export const NOTCH_SLOT_VERTICAL_PADDING = 6;

export function resolvedNotchTopInset(layout: NotchLayoutMetrics) {
  return Number.isFinite(layout.topInset) && layout.topInset > 0
    ? layout.topInset
    : NON_NOTCH_TOP_INSET_FALLBACK;
}

export function notchCollapsedHeightFloor(layout: NotchLayoutMetrics) {
  const topInset = resolvedNotchTopInset(layout);
  return layout.hasNotch ? Math.max(COLLAPSED_HEIGHT_FALLBACK, topInset) : topInset;
}

/**
 * 计算紧凑态封面和频谱实际占用的槽位宽度，让它们在最低宽度时贴住刘海内侧。
 */
export function notchCompactSlotSize(layout: NotchLayoutMetrics) {
  const topInset = resolvedNotchTopInset(layout);
  return Math.max(
    0,
    Math.min(NOTCH_SLOT_MAX_WIDTH, topInset - NOTCH_SLOT_VERTICAL_PADDING * 2),
  );
}

export function notchSlotPadding(borderRadius: number) {
  const radius = Number.isFinite(borderRadius)
    ? Math.min(NOTCH_SLOT_RADIUS_MAX, Math.max(0, borderRadius))
    : 0;
  // 按圆角在 45° 方向的内收投影补偿横向位置，默认 12px 仍与纵向保持 6px。
  const curvedPadding = NOTCH_SLOT_VERTICAL_PADDING
    + (radius - NOTCH_SLOT_RADIUS_ANCHOR) * NOTCH_CORNER_DIAGONAL_INSET;
  return Math.max(NOTCH_SLOT_MIN_HORIZONTAL_PADDING, curvedPadding);
}

/**
 * 计算常驻态不遮挡刘海和左右槽位的最小宽度。
 *
 * 即使当前默认圆角较小，也按最大 20px 圆角预留活动空间，避免用户
 * 调大圆角后槽位被挤入刘海区域。无刘海屏没有硬件缺口，沿用统一的
 * 270px 下限以保持左右槽位和同行歌词的可用空间。
 */
export function notchMinimumWidth(layout: NotchLayoutMetrics) {
  if (!layout.hasNotch) return NOTCH_NON_NOTCH_MIN_WIDTH;
  const centerGapWidth = Number.isFinite(layout.centerGapWidth)
    ? Math.max(0, layout.centerGapWidth)
    : 0;
  const width = centerGapWidth
    + 2 * (
      notchCompactSlotSize(layout)
      + notchSlotPadding(NOTCH_DEFAULT_BORDER_RADIUS_MAX)
    );
  return Math.min(NOTCH_MAX_WIDTH, Math.max(NOTCH_NATIVE_MIN_WIDTH, Math.ceil(width)));
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

export function islandRadii(borderRadius: number) {
  const radius = `${borderRadius}px`;
  // GSAP 全程使用分角属性，避免屏幕类型切换或动画往返时残留简写圆角。
  return {
    borderTopLeftRadius: "0px",
    borderTopRightRadius: "0px",
    borderBottomRightRadius: radius,
    borderBottomLeftRadius: radius,
  };
}
