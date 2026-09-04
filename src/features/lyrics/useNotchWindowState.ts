import { useRef, useState } from "react";
import { isTauriRuntime } from "../../shared/api";
import type { NotchLyricsPreferences } from "../../shared/types";
import {
  COLLAPSED_HEIGHT_FALLBACK,
  emptyLayout,
  EXPANDED_HEIGHT_FALLBACK,
  NOTCH_MAX_WIDTH,
  type IslandDimensions,
  type IslandState,
  type NotchWindowFitRequest,
  type NotchWidthPreviewValues,
} from "./NotchLyricsLayout";

type UseNotchWindowStateOptions = {
  appearance: NotchLyricsPreferences["appearance"];
};

export function useNotchWindowState({ appearance }: UseNotchWindowStateOptions) {
  const [layout, setLayout] = useState(emptyLayout);
  const [islandState, setIslandState] = useState<IslandState>("collapsed");
  const [expandedWidth, setExpandedWidth] = useState(
    () => Math.min(NOTCH_MAX_WIDTH, Math.max(appearance.maxWidth, appearance.expandedMaxWidth)),
  );
  const [collapsedHeight, setCollapsedHeight] = useState(COLLAPSED_HEIGHT_FALLBACK);
  const [expandedHeight, setExpandedHeight] = useState(EXPANDED_HEIGHT_FALLBACK);
  const [previewValues, setPreviewValues] = useState<NotchWidthPreviewValues | null>(null);
  const [previewActive, setPreviewActive] = useState(false);
  const [widthMotionActive, setWidthMotionActive] = useState(false);
  const [islandVisible, setIslandVisible] = useState(() => !isTauriRuntime());
  const [visibilityMotionActive, setVisibilityMotionActive] = useState(isTauriRuntime);
  const shellRef = useRef<HTMLElement>(null);
  const hoverAreaRef = useRef<HTMLDivElement>(null);
  const islandRef = useRef<HTMLElement>(null);
  const islandVisualRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const toolbarRevealRef = useRef<HTMLDivElement>(null);
  const islandStateRef = useRef<IslandState>("collapsed");
  const hostFitReadyRef = useRef(!isTauriRuntime());
  const pendingHoverApplyRef = useRef(false);
  const pendingVisibilityRef = useRef<boolean | null>(null);
  const flushHostReadyRef = useRef<() => void>(() => undefined);
  const islandVisibleRef = useRef(islandVisible);
  const visibilityMotionActiveRef = useRef(visibilityMotionActive);
  const widthMotionActiveRef = useRef(false);
  const previewActiveRef = useRef(false);
  const previewValuesRef = useRef<NotchWidthPreviewValues | null>(null);
  const dimensionsRef = useRef<IslandDimensions>({
    collapsedWidth: appearance.maxWidth,
    collapsedHeight: COLLAPSED_HEIGHT_FALLBACK,
    expandedWidth: Math.min(NOTCH_MAX_WIDTH, Math.max(appearance.maxWidth, appearance.expandedMaxWidth)),
    expandedHeight: EXPANDED_HEIGHT_FALLBACK,
  });
  const pendingDimensionsRef = useRef<IslandDimensions | null>(null);
  const lastFitRequestRef = useRef<NotchWindowFitRequest | null>(null);
  const lastObservedGeometryRef = useRef({ collapsedHeight: -1, expandedHeight: -1 });
  const reconcileHoverStateRef = useRef<() => void>(() => undefined);

  return {
    layout,
    setLayout,
    islandState,
    setIslandState,
    expandedWidth,
    setExpandedWidth,
    collapsedHeight,
    setCollapsedHeight,
    expandedHeight,
    setExpandedHeight,
    previewValues,
    setPreviewValues,
    previewActive,
    setPreviewActive,
    widthMotionActive,
    setWidthMotionActive,
    islandVisible,
    setIslandVisible,
    visibilityMotionActive,
    setVisibilityMotionActive,
    shellRef,
    hoverAreaRef,
    islandRef,
    islandVisualRef,
    contentRef,
    toolbarRevealRef,
    islandStateRef,
    hostFitReadyRef,
    pendingHoverApplyRef,
    pendingVisibilityRef,
    flushHostReadyRef,
    islandVisibleRef,
    visibilityMotionActiveRef,
    widthMotionActiveRef,
    previewActiveRef,
    previewValuesRef,
    dimensionsRef,
    pendingDimensionsRef,
    lastFitRequestRef,
    lastObservedGeometryRef,
    reconcileHoverStateRef,
  };
}
