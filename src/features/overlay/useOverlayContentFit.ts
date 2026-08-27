import { useLayoutEffect, type MutableRefObject, type RefObject } from "react";
import { api, isTauriRuntime } from "../../shared/api";
import type { OverlayStyle } from "../../shared/types";
import {
  combinedContentSize,
  sameMarqueeMetrics,
  MIN_VERTICAL_HOST_WIDTH,
  type MarqueeMetric,
  type SupportingLine,
} from "./OverlayLayout";

const MIN_LYRIC_FONT_SIZE = 12;
const FIT_RETRY_DELAY_MS = 50;
const SHRINK_DELAY_MS = 700;
const MARQUEE_SPEED_PX_PER_SECOND = 35;
const DEFAULT_MARQUEE_DURATION_SECONDS = 4;

type UseOverlayContentFitOptions = {
  linesRef: RefObject<HTMLDivElement | null>;
  activeRef: RefObject<HTMLDivElement | null>;
  supportingRefs: MutableRefObject<Array<HTMLDivElement | null>>;
  fitFrame: MutableRefObject<number | null>;
  fitRetryTimer: MutableRefObject<ReturnType<typeof setTimeout> | null>;
  shrinkTimer: MutableRefObject<ReturnType<typeof setTimeout> | null>;
  lastRequestedSize: MutableRefObject<{ width: number; height: number } | null>;
  lastMeasuredLayoutKey: MutableRefObject<string | null>;
  style: OverlayStyle;
  settingsVisible: boolean;
  resizing: boolean;
  fitLimits: { width: number; height: number };
  fitScale: number;
  wrapped: boolean;
  marqueeMetrics: MarqueeMetric[];
  primaryLineKey: string;
  supportingKey: string;
  primaryText: string;
  supportingLines: SupportingLine[];
  marqueeTimeLimit: number | null;
  vertical: boolean;
  overlayHorizontalPadding: number;
  overlayVerticalPadding: number;
  horizontalContentLimit: number;
  verticalContentLimit: number;
  marqueeHorizontalLimit: number;
  marqueeVerticalLimit: number;
  horizontalWindowLimit: number;
  verticalWindowLimit: number;
  constrained: boolean;
  setWrapped: (value: boolean) => void;
  setFitScale: (value: number) => void;
  setMarqueeMetrics: (value: MarqueeMetric[]) => void;
};

export function useOverlayContentFit({
  linesRef,
  activeRef,
  supportingRefs,
  fitFrame,
  fitRetryTimer,
  shrinkTimer,
  lastRequestedSize,
  lastMeasuredLayoutKey,
  style,
  settingsVisible,
  resizing,
  fitLimits,
  fitScale,
  wrapped,
  marqueeMetrics,
  primaryLineKey,
  supportingKey,
  primaryText,
  supportingLines,
  marqueeTimeLimit,
  vertical,
  overlayHorizontalPadding,
  overlayVerticalPadding,
  horizontalContentLimit,
  verticalContentLimit,
  marqueeHorizontalLimit,
  marqueeVerticalLimit,
  horizontalWindowLimit,
  verticalWindowLimit,
  constrained,
  setWrapped,
  setFitScale,
  setMarqueeMetrics,
}: UseOverlayContentFitOptions) {
  useLayoutEffect(() => {
    setWrapped(false);
    setFitScale(1);
    setMarqueeMetrics([]);
    lastRequestedSize.current = null;
  }, [fitLimits.height, fitLimits.width, primaryLineKey, supportingKey, style.backgroundPaddingX, style.backgroundPaddingY, style.fontFamily, style.fontSize, style.fontWeight, style.horizontalMaxWidth, style.layout, style.lineGap, style.lineHeight, style.longText, style.orientation, style.romanizationFontScale, style.secondaryFontScale, style.secondaryFontWeight, style.textStrokeWidth, style.translationFontScale, style.verticalMaxHeight]);

  useLayoutEffect(() => {
    if (!settingsVisible || resizing) {
      lastRequestedSize.current = null;
      if (fitRetryTimer.current !== null) clearTimeout(fitRetryTimer.current);
      fitRetryTimer.current = null;
      return;
    }
    const layoutKey = `${style.layout}:${style.orientation}`;
    const layoutChanged = lastMeasuredLayoutKey.current !== layoutKey;
    if (layoutChanged) {
      lastMeasuredLayoutKey.current = layoutKey;
      lastRequestedSize.current = null;
      if (shrinkTimer.current !== null) clearTimeout(shrinkTimer.current);
      shrinkTimer.current = null;
      if (fitScale !== 1) {
        setFitScale(1);
        return;
      }
      if (wrapped) {
        setWrapped(false);
        return;
      }
      if (marqueeMetrics.length > 0) {
        setMarqueeMetrics([]);
        return;
      }
    }
    const lines = linesRef.current;
    const active = activeRef.current;
    if (!lines || !active) return;
    const supportingElements = supportingRefs.current
      .slice(0, supportingLines.length)
      .filter((element): element is HTMLDivElement => Boolean(element));
    const elements = [active, ...supportingElements];
    const baseSizes = [style.fontSize, ...supportingLines.map((line) => line.baseSize)];
    const naturalItems = elements.map((element, index) => {
      const currentSize = Math.max(1, baseSizes[index] * fitScale);
      const ratio = baseSizes[index] / currentSize;
      return { width: element.scrollWidth * ratio, height: element.scrollHeight * ratio };
    });
    const natural = combinedContentSize(naturalItems, style.layout, style.orientation, style.lineGap);
    // 竖排列向左扩展时，父级 scrollWidth 可能漏掉负方向溢出，改用每个歌词元素的实际布局盒汇总。
    const renderedItems = elements.map((element) => {
      const bounds = element.getBoundingClientRect();
      return { width: bounds.width, height: bounds.height };
    });
    const rendered = combinedContentSize(renderedItems, style.layout, style.orientation, style.lineGap);
    const availableScreenWidth = Math.max(1, fitLimits.width - overlayHorizontalPadding);
    const availableScreenHeight = Math.max(1, fitLimits.height - overlayVerticalPadding);

    if (style.longText === "shrink") {
      const targetWidth = vertical ? availableScreenWidth : horizontalContentLimit;
      const targetHeight = vertical ? verticalContentLimit : availableScreenHeight;
      const minimumScale = Math.min(1, MIN_LYRIC_FONT_SIZE / Math.max(1, style.fontSize));
      const nextScale = Math.max(
        minimumScale,
        Math.min(1, targetWidth / Math.max(1, natural.width), targetHeight / Math.max(1, natural.height)),
      );
      if (Math.abs(nextScale - fitScale) > 0.005) {
        setFitScale(nextScale);
        return;
      }
    } else if (fitScale !== 1) {
      setFitScale(1);
      return;
    }

    const longAxisOverflow = vertical
      ? natural.height > verticalContentLimit + 1
      : natural.width > horizontalContentLimit + 1;
    if (style.longText === "wrap" && !wrapped && longAxisOverflow) {
      setWrapped(true);
      return;
    }
    if (style.longText === "marquee") {
      const nextMetrics = elements.map((element, index) => {
        const content = element.firstElementChild;
        const currentSize = Math.max(1, baseSizes[index] * fitScale);
        const ratio = baseSizes[index] / currentSize;
        const contentLength = content instanceof HTMLElement
          ? (vertical ? content.offsetHeight : content.offsetWidth) * ratio
          : 0;
        const naturalLength = contentLength > 0
          ? contentLength
          : vertical ? naturalItems[index].height : naturalItems[index].width;
        const limit = vertical ? marqueeVerticalLimit : marqueeHorizontalLimit;
        const distance = Math.max(0, naturalLength - limit);
        const preferredDuration = Math.max(
          DEFAULT_MARQUEE_DURATION_SECONDS,
          distance / MARQUEE_SPEED_PX_PER_SECOND,
        );
        return {
          overflowing: distance > 1,
          distance,
          duration: marqueeTimeLimit === null
            ? preferredDuration
            : Math.min(preferredDuration, marqueeTimeLimit),
        };
      });
      if (!sameMarqueeMetrics(marqueeMetrics, nextMetrics)) {
        setMarqueeMetrics(nextMetrics);
        return;
      }
    }

    const constrainedHorizontal = !vertical && constrained;
    const constrainedVertical = vertical && constrained;
    const wrappedLayout = style.longText === "wrap" && wrapped;
    const measuredContentWidth = vertical && (style.longText === "shrink" || wrappedLayout)
      ? Math.max(lines.clientWidth, Math.min(rendered.width, availableScreenWidth))
      : constrainedHorizontal
        ? horizontalContentLimit
        : Math.max(lines.clientWidth, Math.min(lines.scrollWidth, availableScreenWidth));
    const measuredContentHeight = !vertical && wrappedLayout
      ? Math.max(lines.clientHeight, Math.min(rendered.height, availableScreenHeight))
      : constrainedVertical
        ? verticalContentLimit
        : Math.max(lines.clientHeight, Math.min(lines.scrollHeight, availableScreenHeight));
    const width = vertical
      ? Math.min(fitLimits.width, Math.max(MIN_VERTICAL_HOST_WIDTH, Math.ceil(measuredContentWidth + overlayHorizontalPadding)))
      : horizontalWindowLimit;
    const height = vertical
      ? verticalWindowLimit
      : Math.min(fitLimits.height, Math.max(76, Math.ceil(measuredContentHeight + overlayVerticalPadding)));
    const previous = lastRequestedSize.current;
    if (previous && Math.abs(previous.width - width) <= 2 && Math.abs(previous.height - height) <= 2) return;
    const applySize = (nextSize: { width: number; height: number }) => {
      if (!isTauriRuntime()) return;
      void api.fitOverlayContent(nextSize.width, nextSize.height).then((applied) => {
        if (applied || lastRequestedSize.current !== nextSize) return;
        fitRetryTimer.current = setTimeout(() => {
          fitRetryTimer.current = null;
          if (lastRequestedSize.current === nextSize) applySize(nextSize);
        }, FIT_RETRY_DELAY_MS);
      });
    };
    const requestSize = (nextSize: { width: number; height: number }) => {
      if (fitFrame.current !== null) cancelAnimationFrame(fitFrame.current);
      if (fitRetryTimer.current !== null) clearTimeout(fitRetryTimer.current);
      fitRetryTimer.current = null;
      fitFrame.current = requestAnimationFrame(() => {
        fitFrame.current = null;
        lastRequestedSize.current = nextSize;
        applySize(nextSize);
      });
    };
    if (!previous) {
      requestSize({ width, height });
    } else {
      const immediate = { width: Math.max(previous.width, width), height: Math.max(previous.height, height) };
      if (immediate.width !== previous.width || immediate.height !== previous.height) requestSize(immediate);
      if (width < immediate.width || height < immediate.height) {
        if (shrinkTimer.current !== null) clearTimeout(shrinkTimer.current);
        shrinkTimer.current = setTimeout(() => {
          shrinkTimer.current = null;
          requestSize({ width, height });
        }, SHRINK_DELAY_MS);
      }
    }
    return () => {
      if (fitFrame.current !== null) cancelAnimationFrame(fitFrame.current);
      fitFrame.current = null;
      if (shrinkTimer.current !== null) clearTimeout(shrinkTimer.current);
      shrinkTimer.current = null;
    };
  }, [constrained, fitLimits.height, fitLimits.width, fitScale, horizontalContentLimit, horizontalWindowLimit, marqueeHorizontalLimit, marqueeMetrics, marqueeTimeLimit, marqueeVerticalLimit, overlayHorizontalPadding, overlayVerticalPadding, primaryText, resizing, settingsVisible, style.fontFamily, style.fontSize, style.fontWeight, style.layout, style.lineGap, style.lineHeight, style.longText, style.orientation, style.romanizationFontScale, style.secondaryFontScale, style.secondaryFontWeight, style.textStrokeWidth, style.translationFontScale, supportingKey, vertical, verticalContentLimit, verticalWindowLimit, wrapped]);
}
