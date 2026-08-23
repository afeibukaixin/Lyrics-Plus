import { useCallback, useLayoutEffect, useEffect, type MutableRefObject, type RefObject } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { gsap } from "gsap";
import { api, isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import type { NotchLayoutMetrics, NotchLyricsAppearance } from "../../shared/types";
import {
  COLLAPSED_HEIGHT_FALLBACK,
  EXPANDED_HEIGHT_FALLBACK,
  islandRadii,
  NOTCH_MAX_WIDTH,
  physicalSizeMatches,
  waitForWebviewLayout,
  type IslandDimensions,
  type IslandState,
  type NotchWindowFitRequest,
  type NotchWidthPreviewValues,
} from "./NotchLyricsLayout";

const WINDOW_HORIZONTAL_PADDING = 16;
const NO_NOTCH_TOP_GAP = 6;

type UseNotchWindowGeometryOptions = {
  layout: NotchLayoutMetrics;
  appearance: NotchLyricsAppearance;
  contentRef: RefObject<HTMLDivElement | null>;
  toolbarRevealRef: RefObject<HTMLDivElement | null>;
  islandRef: RefObject<HTMLElement | null>;
  hostFitReadyRef: MutableRefObject<boolean>;
  pendingHoverApplyRef: MutableRefObject<boolean>;
  pendingDimensionsRef: MutableRefObject<IslandDimensions | null>;
  previewActiveRef: MutableRefObject<boolean>;
  previewValuesRef: MutableRefObject<NotchWidthPreviewValues | null>;
  dimensionsRef: MutableRefObject<IslandDimensions>;
  lastFitRequestRef: MutableRefObject<NotchWindowFitRequest | null>;
  lastObservedGeometryRef: MutableRefObject<{ collapsedHeight: number; expandedHeight: number }>;
  widthMotionActiveRef: MutableRefObject<boolean>;
  islandVisibleRef: MutableRefObject<boolean>;
  visibilityMotionActiveRef: MutableRefObject<boolean>;
  islandStateRef: MutableRefObject<IslandState>;
  flushHostReadyRef: MutableRefObject<() => void>;
  reconcileHoverStateRef: MutableRefObject<() => void>;
  expandedWidth: number;
  collapsedHeight: number;
  expandedHeight: number;
  islandState: IslandState;
  effectiveWidth: number;
  setExpandedWidth: (value: number) => void;
  setCollapsedHeight: (value: number) => void;
  setExpandedHeight: (value: number) => void;
};

export function useNotchWindowGeometry({
  layout,
  appearance,
  contentRef,
  toolbarRevealRef,
  islandRef,
  hostFitReadyRef,
  pendingHoverApplyRef,
  pendingDimensionsRef,
  previewActiveRef,
  previewValuesRef,
  dimensionsRef,
  lastFitRequestRef,
  lastObservedGeometryRef,
  widthMotionActiveRef,
  islandVisibleRef,
  visibilityMotionActiveRef,
  islandStateRef,
  flushHostReadyRef,
  reconcileHoverStateRef,
  expandedWidth,
  collapsedHeight,
  expandedHeight,
  islandState,
  effectiveWidth,
  setExpandedWidth,
  setCollapsedHeight,
  setExpandedHeight,
}: UseNotchWindowGeometryOptions) {
  const requestNativeFit = useCallback((dimensions: IslandDimensions) => {
    if (!isTauriRuntime()) {
      hostFitReadyRef.current = true;
      return;
    }
    const width = dimensions.expandedWidth + WINDOW_HORIZONTAL_PADDING;
    const height = dimensions.expandedHeight + (layout.hasNotch ? 0 : NO_NOTCH_TOP_GAP);
    const requestKey = `${width}:${height}`;
    if (lastFitRequestRef.current?.key === requestKey) return;
    lastFitRequestRef.current?.cancel();
    hostFitReadyRef.current = false;
    let cancelRequest: () => void = () => undefined;
    const ready = (async () => {
      const currentWindow = getCurrentWindow();
      let expectedSize: { physicalWidth: number; physicalHeight: number } | null = null;
      let latestResize: { width: number; height: number } | null = null;
      let cancelled = false;
      let resolveMatchedResize: () => void = () => undefined;
      const matchedResize = new Promise<void>((resolve) => {
        resolveMatchedResize = resolve;
      });
      let unlistenResize: (() => void) | null = null;
      cancelRequest = () => {
        cancelled = true;
        resolveMatchedResize();
      };

      try {
        // 先监听再请求原生窗口调整，避免漏掉 AppKit 很快发出的 resize 回执。
        unlistenResize = await currentWindow.onResized(({ payload }) => {
          latestResize = payload;
          if (expectedSize && physicalSizeMatches(payload, expectedSize)) {
            resolveMatchedResize();
          }
        });
        if (cancelled) return false;

        const result = await api.fitNotchLyricsContent(width, height);
        if (cancelled) return false;
        expectedSize = result;
        if (result.sizeChanged) {
          const currentSize = await currentWindow.outerSize();
          const resizeAlreadyMatched = latestResize
            ? physicalSizeMatches(latestResize, result)
            : false;
          if (!resizeAlreadyMatched && !physicalSizeMatches(currentSize, result)) {
            await matchedResize;
          }
        }
        if (cancelled) return false;
        await waitForWebviewLayout();
        return !cancelled;
      } catch (error) {
        reportFrontendError("Failed to fit the Dynamic Island lyrics window", error);
        return false;
      } finally {
        unlistenResize?.();
      }
    })();
    const request = { key: requestKey, ready, cancel: () => cancelRequest() };
    lastFitRequestRef.current = request;
    void ready.then(() => {
      if (lastFitRequestRef.current !== request) return;
      lastFitRequestRef.current = null;
      hostFitReadyRef.current = true;
      flushHostReadyRef.current();
      if (
        pendingHoverApplyRef.current
        && islandVisibleRef.current
        && !visibilityMotionActiveRef.current
        && !previewActiveRef.current
      ) {
        pendingHoverApplyRef.current = false;
        requestAnimationFrame(() => reconcileHoverStateRef.current());
      }
    });
  }, [flushHostReadyRef, hostFitReadyRef, islandVisibleRef, lastFitRequestRef, layout.hasNotch, pendingHoverApplyRef, reconcileHoverStateRef, visibilityMotionActiveRef]);

  const applyMeasuredDimensions = useCallback((next: IslandDimensions) => {
    dimensionsRef.current = next;
    setExpandedWidth(next.expandedWidth);
    setCollapsedHeight(next.collapsedHeight);
    setExpandedHeight(next.expandedHeight);
    requestNativeFit(next);
  }, [dimensionsRef, requestNativeFit, setCollapsedHeight, setExpandedHeight, setExpandedWidth]);

  const fitWindow = useCallback((collapsedWidthOverride?: number) => {
    const compactContent = contentRef.current;
    const expandedContent = toolbarRevealRef.current;
    if (!compactContent || !expandedContent) return;
    const preview = previewValuesRef.current;
    const collapsedWidth = collapsedWidthOverride ?? preview?.maxWidth ?? appearance.maxWidth;
    const expandedMaxWidth = Math.min(
      NOTCH_MAX_WIDTH,
      Math.max(
        collapsedWidth,
        preview?.expandedMaxWidth ?? appearance.expandedMaxWidth,
      ),
    );
    const nextDimensions: IslandDimensions = {
      collapsedWidth,
      collapsedHeight: Math.max(COLLAPSED_HEIGHT_FALLBACK, Math.ceil(compactContent.scrollHeight)),
      expandedWidth: expandedMaxWidth,
      expandedHeight: Math.max(EXPANDED_HEIGHT_FALLBACK, Math.ceil(expandedContent.scrollHeight)),
    };
    if (widthMotionActiveRef.current) {
      pendingDimensionsRef.current = nextDimensions;
      return;
    }
    applyMeasuredDimensions(nextDimensions);
  }, [appearance.expandedMaxWidth, appearance.maxWidth, applyMeasuredDimensions, contentRef, pendingDimensionsRef, previewValuesRef, toolbarRevealRef, widthMotionActiveRef]);

  useLayoutEffect(() => {
    if (!previewActiveRef.current) fitWindow();
  }, [appearance.expandedMaxWidth, appearance.maxWidth, fitWindow, previewActiveRef]);

  useLayoutEffect(() => {
    if (widthMotionActiveRef.current) return;
    const island = islandRef.current;
    if (!island) return;
    const dimensions = dimensionsRef.current;
    const expanded = islandStateRef.current === "expanded";
    // 稳定态也由 GSAP 写入数值，SCSS 只提供首帧回退尺寸，不参与状态切换。
    gsap.set(island, {
      width: expanded ? dimensions.expandedWidth : dimensions.collapsedWidth,
      height: expanded ? dimensions.expandedHeight : dimensions.collapsedHeight,
      ...islandRadii(layout.hasNotch, appearance.borderRadius, expanded),
    });
  }, [appearance.borderRadius, collapsedHeight, dimensionsRef, effectiveWidth, expandedHeight, expandedWidth, islandRef, islandState, islandStateRef, layout.hasNotch, widthMotionActiveRef]);

  const applyPendingDimensions = useCallback(() => {
    const pending = pendingDimensionsRef.current;
    if (!pending) return;
    pendingDimensionsRef.current = null;
    applyMeasuredDimensions(pending);
  }, [applyMeasuredDimensions, pendingDimensionsRef]);

  useLayoutEffect(() => {
    const content = contentRef.current;
    const player = toolbarRevealRef.current;
    if (!content || !player) return;
    let frame = 0;
    const measure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const nextCollapsedHeight = Math.ceil(content.scrollHeight);
        const nextExpandedHeight = Math.ceil(player.scrollHeight);
        const previous = lastObservedGeometryRef.current;
        if (previous.collapsedHeight === nextCollapsedHeight && previous.expandedHeight === nextExpandedHeight) return;
        lastObservedGeometryRef.current = {
          collapsedHeight: nextCollapsedHeight,
          expandedHeight: nextExpandedHeight,
        };
        fitWindow();
      });
    };
    const observer = new ResizeObserver(measure);
    observer.observe(content);
    observer.observe(player);
    measure();
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [contentRef, fitWindow, lastObservedGeometryRef, toolbarRevealRef]);

  useEffect(() => () => {
    lastFitRequestRef.current?.cancel();
    lastFitRequestRef.current = null;
  }, [lastFitRequestRef]);

  return { applyPendingDimensions, fitWindow };
}
