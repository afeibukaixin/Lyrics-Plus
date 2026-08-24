import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  type Dispatch,
  type MutableRefObject,
  type RefObject,
  type SetStateAction,
} from "react";
import { flushSync } from "react-dom";
import { useGSAP } from "@gsap/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { gsap } from "gsap";
import { CustomEase } from "gsap/CustomEase";
import { isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import type { NotchLayoutMetrics, NotchLyricsPreferences } from "../../shared/types";
import {
  islandRadii,
  type IslandDimensions,
  type IslandState,
} from "./NotchLyricsLayout";

gsap.registerPlugin(useGSAP, CustomEase);

const MORPH_DURATION_SECONDS = 0.26;
// 仅给真实 Visual Island 边缘留出极小容差，不把透明窗口算进 hover。
const HOVER_PADDING = 4;

const EXPAND_MORPH_EASE = CustomEase.create("notch-expand-morph", "0.16,1,0.3,1");
const COLLAPSE_MORPH_EASE = CustomEase.create("notch-collapse-morph", "0.4,0,0.2,1");

type UseNotchIslandMotionOptions = {
  appearance: NotchLyricsPreferences["appearance"];
  layout: NotchLayoutMetrics;
  shellRef: RefObject<HTMLElement | null>;
  hoverAreaRef: RefObject<HTMLElement | null>;
  islandRef: RefObject<HTMLElement | null>;
  islandSurfaceRef: RefObject<HTMLDivElement | null>;
  contentRef: RefObject<HTMLDivElement | null>;
  toolbarRevealRef: RefObject<HTMLDivElement | null>;
  islandStateRef: MutableRefObject<IslandState>;
  setIslandState: Dispatch<SetStateAction<IslandState>>;
  setIslandVisible: Dispatch<SetStateAction<boolean>>;
  setVisibilityMotionActive: Dispatch<SetStateAction<boolean>>;
  setWidthMotionActive: Dispatch<SetStateAction<boolean>>;
  islandVisibleRef: MutableRefObject<boolean>;
  visibilityMotionActiveRef: MutableRefObject<boolean>;
  widthMotionActiveRef: MutableRefObject<boolean>;
  hostFitReadyRef: MutableRefObject<boolean>;
  pendingHoverApplyRef: MutableRefObject<boolean>;
  pendingVisibilityRef: MutableRefObject<boolean | null>;
  flushHostReadyRef: MutableRefObject<() => void>;
  previewActiveRef: MutableRefObject<boolean>;
  dimensionsRef: MutableRefObject<IslandDimensions>;
  pendingDimensionsRef: MutableRefObject<{ collapsedWidth: number; collapsedHeight: number; expandedWidth: number; expandedHeight: number } | null>;
  reconcileHoverStateRef: MutableRefObject<() => void>;
  applyPendingDimensions: () => void;
};

export function useNotchIslandMotion({
  appearance,
  layout,
  shellRef,
  hoverAreaRef,
  islandRef,
  islandSurfaceRef,
  contentRef,
  toolbarRevealRef,
  islandStateRef,
  setIslandState,
  setIslandVisible,
  setVisibilityMotionActive,
  setWidthMotionActive,
  islandVisibleRef,
  visibilityMotionActiveRef,
  widthMotionActiveRef,
  hostFitReadyRef,
  pendingHoverApplyRef,
  pendingVisibilityRef,
  flushHostReadyRef,
  previewActiveRef,
  dimensionsRef,
  pendingDimensionsRef,
  reconcileHoverStateRef,
  applyPendingDimensions,
}: UseNotchIslandMotionOptions) {
  const presenceTimelineRef = useRef<gsap.core.Timeline | null>(null);
  const islandMorphRef = useRef<gsap.core.Timeline | null>(null);
  const reducedMotionRef = useRef(false);
  const pointerInsideRef = useRef(false);
  const pendingExpandRef = useRef(false);
  const requestedPointerInteractiveRef = useRef<boolean | null>(null);
  const appliedPointerInteractiveRef = useRef<boolean | null>(null);
  const pointerInteractionQueueRef = useRef<Promise<void>>(Promise.resolve());

  const setNotchPointerInteractive = useCallback((interactive: boolean) => {
    if (!isTauriRuntime()) return;
    requestedPointerInteractiveRef.current = interactive;
    // 串行处理并在执行前读取最新目标，避免快速进出岛体时过期请求反复切换穿透状态。
    pointerInteractionQueueRef.current = pointerInteractionQueueRef.current
      .catch(() => undefined)
      .then(async () => {
        const desired = requestedPointerInteractiveRef.current;
        if (desired === null || appliedPointerInteractiveRef.current === desired) return;
        try {
          await getCurrentWindow().setIgnoreCursorEvents(!desired);
          appliedPointerInteractiveRef.current = desired;
        } catch (error) {
          if (requestedPointerInteractiveRef.current === desired) {
            requestedPointerInteractiveRef.current = null;
          }
          reportFrontendError("Failed to update Dynamic Island pointer passthrough", error);
        }
      });
  }, []);

  useEffect(() => {
    setNotchPointerInteractive(false);
    return () => setNotchPointerInteractive(false);
  }, [setNotchPointerInteractive]);

  const setIslandStateValue = useCallback((next: IslandState) => {
    if (import.meta.env.DEV && islandStateRef.current !== next) {
      console.debug("notch island state", { from: islandStateRef.current, to: next });
    }
    islandStateRef.current = next;
    setIslandState(next);
  }, [setIslandState, islandStateRef]);

  const finishWidthMotion = useCallback((finalExpanded: boolean) => {
    islandMorphRef.current = null;
    flushSync(() => setIslandStateValue(finalExpanded ? "expanded" : "collapsed"));
    widthMotionActiveRef.current = false;
    setWidthMotionActive(false);
    const island = islandRef.current;
    const surface = islandSurfaceRef.current;
    const content = contentRef.current;
    const toolbarReveal = toolbarRevealRef.current;
    if (island) gsap.set(island, { clearProps: "willChange" });
    if (surface) gsap.set(surface, { clearProps: "height,willChange" });
    if (content) {
      gsap.set(content, {
        autoAlpha: finalExpanded ? 0 : 1,
        y: finalExpanded ? -4 : 0,
        scale: 1,
        clearProps: "willChange",
      });
    }
    if (toolbarReveal) {
      gsap.set(toolbarReveal, {
        autoAlpha: finalExpanded ? 1 : 0,
        y: finalExpanded ? 0 : -4,
        scale: 1,
        clearProps: "willChange",
      });
    }
    applyPendingDimensions();
    requestAnimationFrame(() => reconcileHoverStateRef.current());
  }, [applyPendingDimensions, contentRef, islandRef, islandSurfaceRef, reconcileHoverStateRef, setIslandStateValue, setWidthMotionActive, toolbarRevealRef, widthMotionActiveRef]);

  const cancelWidthMotion = useCallback(() => {
    islandMorphRef.current?.kill();
    islandMorphRef.current = null;
    pendingDimensionsRef.current = null;
    pendingExpandRef.current = false;
    widthMotionActiveRef.current = false;
    setWidthMotionActive(false);
    flushSync(() => setIslandStateValue("collapsed"));
    const island = islandRef.current;
    const surface = islandSurfaceRef.current;
    const content = contentRef.current;
    const toolbarReveal = toolbarRevealRef.current;
    if (island) gsap.set(island, { clearProps: "width,height,borderRadius,borderTopLeftRadius,borderTopRightRadius,borderBottomRightRadius,borderBottomLeftRadius,willChange" });
    if (surface) gsap.set(surface, { clearProps: "height,willChange" });
    if (content) gsap.set(content, { autoAlpha: 1, y: 0, scale: 1, clearProps: "willChange" });
    if (toolbarReveal) gsap.set(toolbarReveal, { autoAlpha: 0, y: -4, scale: 1, clearProps: "willChange" });
  }, [contentRef, islandRef, islandSurfaceRef, pendingDimensionsRef, setIslandStateValue, setWidthMotionActive, toolbarRevealRef, widthMotionActiveRef]);

  const finishVisibilityMotion = useCallback(() => {
    visibilityMotionActiveRef.current = false;
    setVisibilityMotionActive(false);
    const surface = islandSurfaceRef.current;
    const content = contentRef.current;
    if (surface) gsap.set(surface, { clearProps: "willChange" });
    if (content) gsap.set(content, { clearProps: "willChange" });
    if (islandVisibleRef.current && hostFitReadyRef.current && !previewActiveRef.current) {
      pendingHoverApplyRef.current = false;
      requestAnimationFrame(() => reconcileHoverStateRef.current());
    }
  }, [contentRef, hostFitReadyRef, islandSurfaceRef, islandVisibleRef, pendingHoverApplyRef, previewActiveRef, reconcileHoverStateRef, setVisibilityMotionActive, visibilityMotionActiveRef]);

  const { contextSafe } = useGSAP(() => {
    const surface = islandSurfaceRef.current;
    const content = contentRef.current;
    if (!surface || !content) return;
    const media = gsap.matchMedia();

    media.add({
      reduceMotion: "(prefers-reduced-motion: reduce)",
      allowMotion: "(prefers-reduced-motion: no-preference)",
    }, (context) => {
      const reduceMotion = Boolean(context.conditions?.reduceMotion);
      reducedMotionRef.current = reduceMotion;
      gsap.set(surface, {
        clipPath: "inset(0 46% round 999px)",
        scaleY: 0.04,
        transformOrigin: "50% 0%",
      });
      gsap.set(content, { autoAlpha: 0, y: -4 });

      if (reduceMotion) {
        presenceTimelineRef.current = null;
        if (islandVisibleRef.current) {
          gsap.set(surface, { clipPath: "inset(0 0% round 0px)", scaleY: 1 });
          gsap.set(content, { autoAlpha: 1, y: 0 });
        }
        finishVisibilityMotion();
        return;
      }

      const timeline = gsap.timeline({
        paused: true,
        onComplete: finishVisibilityMotion,
        onReverseComplete: finishVisibilityMotion,
      });
      timeline
        .to(surface, { scaleY: 0.18, duration: 0.08, ease: "power2.out" }, 0)
        .to(surface, {
          clipPath: "inset(0 0% round 0px)",
          duration: 0.21,
          ease: "power3.out",
        }, 0.04)
        .to(surface, {
          keyframes: [
            { scaleY: 1.06, duration: 0.18, ease: "power3.out" },
            { scaleY: 0.985, duration: 0.06, ease: "power2.inOut" },
            { scaleY: 1, duration: 0.04, ease: "power2.out" },
          ],
        }, 0.07)
        .to(content, {
          autoAlpha: 1,
          y: 0,
          duration: 0.22,
          ease: "power2.out",
        }, 0.13);
      presenceTimelineRef.current = timeline;
      timeline.progress(islandVisibleRef.current ? 1 : 0).pause();

      return () => {
        timeline.kill();
        if (presenceTimelineRef.current === timeline) presenceTimelineRef.current = null;
      };
    });

    return () => media.revert();
  }, { scope: shellRef });

  const animateIslandVisibility = useMemo(() => contextSafe((visible: boolean) => {
    const surface = islandSurfaceRef.current;
    const content = contentRef.current;
    if (!surface || !content) return;
    const timeline = presenceTimelineRef.current;
    if (reducedMotionRef.current || !timeline) {
      gsap.set(surface, visible
        ? { clipPath: "inset(0 0% round 0px)", scaleY: 1 }
        : { clipPath: "inset(0 46% round 999px)", scaleY: 0.04 });
      gsap.set(content, visible ? { autoAlpha: 1, y: 0 } : { autoAlpha: 0, y: -4 });
      finishVisibilityMotion();
      return;
    }

    const atTarget = visible ? timeline.progress() >= 1 : timeline.progress() <= 0;
    if (atTarget) {
      finishVisibilityMotion();
      return;
    }
    visibilityMotionActiveRef.current = true;
    setVisibilityMotionActive(true);
    gsap.set(surface, { willChange: "transform,clip-path" });
    gsap.set(content, { willChange: "transform,opacity" });
    if (visible) timeline.play();
    else timeline.reverse();
  }), [contextSafe, contentRef, finishVisibilityMotion, islandSurfaceRef, setVisibilityMotionActive, visibilityMotionActiveRef]);

  const flushHostReady = useCallback(() => {
    const pendingVisibility = pendingVisibilityRef.current;
    if (pendingVisibility === null) return;
    pendingVisibilityRef.current = null;
    animateIslandVisibility(pendingVisibility);
  }, [animateIslandVisibility, pendingVisibilityRef]);
  flushHostReadyRef.current = flushHostReady;

  const performLiquidMorph = useMemo(() => contextSafe((nextExpanded: boolean) => {
    const island = islandRef.current;
    const surface = islandSurfaceRef.current;
    const compactContent = contentRef.current;
    const expandedContent = toolbarRevealRef.current;
    if (!island || !surface || !compactContent || !expandedContent) {
      finishWidthMotion(nextExpanded);
      return;
    }
    const dimensions = dimensionsRef.current;
    const start = nextExpanded
      ? { width: dimensions.collapsedWidth, height: dimensions.collapsedHeight }
      : { width: dimensions.expandedWidth, height: dimensions.expandedHeight };
    const target = nextExpanded
      ? { width: dimensions.expandedWidth, height: dimensions.expandedHeight }
      : { width: dimensions.collapsedWidth, height: dimensions.collapsedHeight };
    const collapsedRadii = islandRadii(layout.hasNotch, appearance.borderRadius, false);
    const expandedRadii = islandRadii(layout.hasNotch, appearance.borderRadius, true);
    const startRadii = nextExpanded ? collapsedRadii : expandedRadii;
    const targetRadii = nextExpanded ? expandedRadii : collapsedRadii;
    const complete = () => finishWidthMotion(nextExpanded);

    gsap.set(island, { ...start, ...startRadii });
    flushSync(() => setIslandStateValue(nextExpanded ? "expanding" : "collapsing"));
    gsap.set(island, { ...start, ...startRadii });

    if (reducedMotionRef.current) {
      gsap.set(island, { ...target, ...targetRadii });
      gsap.set(compactContent, nextExpanded ? { autoAlpha: 0, y: -4, scale: 1 } : { autoAlpha: 1, y: 0, scale: 1 });
      gsap.set(expandedContent, nextExpanded ? { autoAlpha: 1, y: 0, scale: 1 } : { autoAlpha: 0, y: -4, scale: 1 });
      finishWidthMotion(nextExpanded);
      return;
    }

    gsap.set(surface, {
      height: "100%",
      scaleX: 1,
      scaleY: 1,
      transformOrigin: "50% 0%",
    });
    // width、height 和圆角共用同一个 tween，确保二维几何同轨开始、同轨结束。
    gsap.set(island, { willChange: "width,height,border-radius" });

    if (nextExpanded) {
      gsap.set(compactContent, {
        autoAlpha: 1,
        y: 0,
        scale: 1,
        willChange: "transform,opacity",
      });
      gsap.set(expandedContent, {
        autoAlpha: 0,
        y: -8,
        scale: 0.96,
        transformOrigin: "50% 0%",
        willChange: "transform,opacity",
      });
    } else {
      gsap.set(expandedContent, { autoAlpha: 0, y: -4, scale: 0.98 });
      gsap.set(compactContent, {
        autoAlpha: 0,
        y: -3,
        scale: 0.97,
        willChange: "transform,opacity",
      });
    }

    const timeline = gsap.timeline({ onComplete: complete });
    timeline.to(island, {
      width: target.width,
      height: target.height,
      ...targetRadii,
      duration: MORPH_DURATION_SECONDS,
      ease: nextExpanded ? EXPAND_MORPH_EASE : COLLAPSE_MORPH_EASE,
      overwrite: "auto",
    }, 0);

    if (nextExpanded) {
      timeline
        .to(compactContent, {
          autoAlpha: 0,
          y: -3,
          scale: 0.96,
          duration: 0.07,
          ease: "power2.in",
        }, 0)
        .to(expandedContent, {
          autoAlpha: 1,
          y: 0,
          scale: 1,
          duration: 0.16,
          ease: "power3.out",
        }, 0.08);
    } else {
      timeline.to(compactContent, {
        autoAlpha: 1,
        y: 0,
        scale: 1,
        duration: 0.12,
        ease: "power2.out",
      }, 0.12);
    }

    islandMorphRef.current = timeline;
  }), [appearance.borderRadius, contentRef, contextSafe, dimensionsRef, finishWidthMotion, islandRef, islandStateRef, islandSurfaceRef, layout.hasNotch, setIslandStateValue, toolbarRevealRef]);

  const startExpansion = useMemo(() => contextSafe(() => {
    if (
      islandStateRef.current !== "collapsed"
      || !islandVisibleRef.current
      || visibilityMotionActiveRef.current
      || previewActiveRef.current
    ) return;
    pendingExpandRef.current = false;
    widthMotionActiveRef.current = true;
    setWidthMotionActive(true);
    performLiquidMorph(true);
  }), [contextSafe, islandStateRef, islandVisibleRef, performLiquidMorph, previewActiveRef, setWidthMotionActive, visibilityMotionActiveRef, widthMotionActiveRef]);

  const startCollapse = useMemo(() => contextSafe(() => {
    if (
      islandStateRef.current !== "expanded"
      || !islandVisibleRef.current
      || visibilityMotionActiveRef.current
      || previewActiveRef.current
    ) return;
    widthMotionActiveRef.current = true;
    setWidthMotionActive(true);
    performLiquidMorph(false);
  }), [contextSafe, islandStateRef, islandVisibleRef, performLiquidMorph, previewActiveRef, setWidthMotionActive, visibilityMotionActiveRef, widthMotionActiveRef]);

  const reconcileHoverState = useCallback(() => {
    if (
      !hostFitReadyRef.current
      || !islandVisibleRef.current
      || visibilityMotionActiveRef.current
      || previewActiveRef.current
    ) {
      pendingHoverApplyRef.current = true;
      return;
    }
    const currentState = islandStateRef.current;
    if (currentState === "collapsed") {
      if (pointerInsideRef.current || pendingExpandRef.current) startExpansion();
      return;
    }
    if (currentState === "expanded" && !pointerInsideRef.current) {
      startCollapse();
      return;
    }
    if (currentState === "collapsing") {
      pendingExpandRef.current = pointerInsideRef.current;
    }
  }, [hostFitReadyRef, islandStateRef, islandVisibleRef, pendingHoverApplyRef, previewActiveRef, startCollapse, startExpansion, visibilityMotionActiveRef]);
  reconcileHoverStateRef.current = reconcileHoverState;

  const processPointerState = useCallback(() => {
    if (!hostFitReadyRef.current || !islandVisibleRef.current || visibilityMotionActiveRef.current || previewActiveRef.current) {
      pendingHoverApplyRef.current = true;
      return;
    }
    const currentState = islandStateRef.current;
    if (currentState === "collapsed") {
      if (pointerInsideRef.current || pendingExpandRef.current) startExpansion();
      return;
    }
    if (currentState === "expanding") return;
    if (currentState === "expanded") {
      if (!pointerInsideRef.current) startCollapse();
      return;
    }
    pendingExpandRef.current = pointerInsideRef.current;
  }, [hostFitReadyRef, islandStateRef, islandVisibleRef, pendingHoverApplyRef, previewActiveRef, startCollapse, startExpansion, visibilityMotionActiveRef]);

  const updateHoverFromPoint = useCallback((x: number, y: number, source: "pointerenter" | "pointermove" | "pointerleave" | "native") => {
    // Tauri 中原生采样是唯一 hover 来源；WebView 事件会随着鼠标穿透切换产生反馈事件。
    if (isTauriRuntime() && source !== "native") return;
    const rect = islandRef.current?.getBoundingClientRect();
    const hoverRect = hoverAreaRef.current?.getBoundingClientRect();
    const currentState = islandStateRef.current;
    const isInsideInteractiveIsland = Boolean(
      islandVisibleRef.current
      && rect
      && x >= rect.left
      && x <= rect.right
      && y >= rect.top
      && y <= rect.bottom,
    );
    const hoverBounds = currentState === "collapsed" ? rect : hoverRect ?? rect;
    const isInsideHoverArea = source !== "pointerleave"
      && islandVisibleRef.current
      && Boolean(
        hoverBounds
        && x >= hoverBounds.left - HOVER_PADDING
        && x <= hoverBounds.right + HOVER_PADDING
        && y >= hoverBounds.top - HOVER_PADDING
        && y <= hoverBounds.bottom + HOVER_PADDING,
      );
    setNotchPointerInteractive(isInsideInteractiveIsland);
    const changed = pointerInsideRef.current !== isInsideHoverArea;
    pointerInsideRef.current = isInsideHoverArea;
    if (import.meta.env.DEV && (changed || source === "pointerenter" || source === "pointerleave")) {
      console.debug("notch pointer sample", {
        source,
        x,
        y,
        rect,
        hoverRect,
        isInsideHoverArea,
        isInsideInteractiveIsland,
        state: currentState,
      });
    }
    processPointerState();
  }, [hoverAreaRef, islandRef, islandStateRef, islandVisibleRef, processPointerState, setNotchPointerInteractive]);

  const handleIslandPointerEnter = useCallback((event: React.PointerEvent<HTMLElement>) => {
    updateHoverFromPoint(event.clientX, event.clientY, "pointerenter");
  }, [updateHoverFromPoint]);

  const handleIslandPointerMove = useCallback((event: React.PointerEvent<HTMLElement>) => {
    updateHoverFromPoint(event.clientX, event.clientY, "pointermove");
  }, [updateHoverFromPoint]);

  const handleIslandPointerLeave = useCallback((event: React.PointerEvent<HTMLElement>) => {
    updateHoverFromPoint(event.clientX, event.clientY, "pointerleave");
  }, [updateHoverFromPoint]);

  const applyIslandVisibility = useCallback((visible: boolean) => {
    visibilityMotionActiveRef.current = !reducedMotionRef.current;
    setVisibilityMotionActive(!reducedMotionRef.current);
    islandVisibleRef.current = visible;
    setIslandVisible(visible);
    if (!visible) {
      setNotchPointerInteractive(false);
      pendingVisibilityRef.current = null;
      pointerInsideRef.current = false;
      pendingExpandRef.current = false;
      if (islandStateRef.current !== "collapsed" || widthMotionActiveRef.current) cancelWidthMotion();
    } else if (!hostFitReadyRef.current) {
      // 原生窗口完成最大展开尺寸适配前保持播放器隐藏，避免首帧几何不完整。
      pendingVisibilityRef.current = true;
      return;
    }
    animateIslandVisibility(visible);
  }, [animateIslandVisibility, cancelWidthMotion, hostFitReadyRef, islandStateRef, islandVisibleRef, pendingVisibilityRef, setIslandVisible, setNotchPointerInteractive, setVisibilityMotionActive, visibilityMotionActiveRef, widthMotionActiveRef]);

  return {
    applyIslandVisibility,
    cancelWidthMotion,
    handleIslandPointerEnter,
    handleIslandPointerLeave,
    handleIslandPointerMove,
    updateHoverFromPoint,
  };
}
