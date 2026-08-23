import type {
  OverlayResizeBounds,
  OverlayResizeEdge,
  OverlayStyle,
} from "../../shared/types";

export const HORIZONTAL_LINE_GAP = 8;
export const VERTICAL_COLUMN_GAP = 14;

export type MarqueeMetric = {
  overflowing: boolean;
  distance: number;
  duration: number;
};

export type ActiveResizeSession = {
  pointerId: number;
  edge: OverlayResizeEdge;
  axis: "horizontal" | "vertical";
  handle: HTMLDivElement;
  startCoordinate: number;
  latestCoordinate: number;
  startMainSize: number | null;
  minimumMainSize: number;
  pendingMainSize: number | null;
  lastBounds: OverlayResizeBounds | null;
  processing: boolean;
  ending: boolean;
  committing: boolean;
};

export function sameMarqueeMetrics(left: MarqueeMetric[], right: MarqueeMetric[]) {
  return left.length === right.length && left.every((item, index) => {
    const other = right[index];
    return item.overflowing === other.overflowing
      && Math.abs(item.distance - other.distance) < 0.5
      && Math.abs(item.duration - other.duration) < 0.05;
  });
}

export function nextValue<T extends string>(current: T, values: readonly T[]) {
  return values[(values.indexOf(current) + 1) % values.length];
}

export function combinedContentSize(
  items: Array<{ width: number; height: number }>,
  layout: OverlayStyle["layout"],
  orientation: OverlayStyle["orientation"],
) {
  const [primary, ...secondary] = items;
  if (!primary) return { width: 0, height: 0 };
  if (secondary.length === 0 || layout === "single") return primary;
  if (orientation === "horizontal") {
    return {
      width: Math.max(...items.map((item) => item.width)),
      height: items.reduce((total, item) => total + item.height, 0)
        + HORIZONTAL_LINE_GAP * (items.length - 1),
    };
  }
  return {
    width: items.reduce((total, item) => total + item.width, 0)
      + VERTICAL_COLUMN_GAP * (items.length - 1),
    height: Math.max(...items.map((item) => item.height)),
  };
}

export type SupportingKind = "next" | "translation" | "romanization";

export type SupportingLine = {
  kind: SupportingKind;
  text: string;
  baseSize: number;
  color: string;
};

export function formatOffset(offsetMs: number) {
  if (offsetMs === 0) return "0s";
  const seconds = (Math.abs(offsetMs) / 1000).toFixed(3).replace(/\.?0+$/, "");
  return `${offsetMs > 0 ? "+" : "−"}${seconds}s`;
}

export function formatOffsetMs(offsetMs: number) {
  if (offsetMs === 0) return "0ms";
  return `${offsetMs > 0 ? "+" : "−"}${Math.abs(offsetMs)}ms`;
}
