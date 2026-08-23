import { useEffect, useLayoutEffect, type Dispatch, type MutableRefObject, type RefObject, type SetStateAction } from "react";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { listen } from "@tauri-apps/api/event";
import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";
import { api, isTauriRuntime } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import type { OverlaySettings, OverlayStyle, ToolbarPlacement } from "../../shared/types";

const HORIZONTAL_TOOLBAR_WINDOW_INSET = 8;
const VERTICAL_TOOLBAR_WINDOW_INSET = 14;
const MIN_HORIZONTAL_WIDTH = 320;
const MIN_VERTICAL_HEIGHT = 280;

type UseOverlayWindowLayoutOptions = {
  style: OverlayStyle;
  styleRef: MutableRefObject<OverlayStyle>;
  settings: OverlaySettings;
  setStyle: Dispatch<SetStateAction<OverlayStyle>>;
  setSettings: Dispatch<SetStateAction<OverlaySettings>>;
  setToolbarSide: Dispatch<SetStateAction<ToolbarPlacement>>;
  setOverlayHovered: Dispatch<SetStateAction<boolean>>;
  setUnlockFeedback: Dispatch<SetStateAction<boolean>>;
  clearResizeState: () => void;
  finishResizeRef: MutableRefObject<() => void>;
  unlockFeedbackTimer: MutableRefObject<ReturnType<typeof setTimeout> | null>;
  toolbarRef: RefObject<HTMLDivElement | null>;
  offsetLabel: string;
  vertical: boolean;
  fitLimits: { width: number; height: number };
  setFitLimits: Dispatch<SetStateAction<{ width: number; height: number }>>;
  minimumHorizontalWidth: number;
  minimumVerticalHeight: number;
  setToolbarMinimums: Dispatch<SetStateAction<{ horizontal: number; vertical: number }>>;
  lastRequestedSize: MutableRefObject<{ width: number; height: number } | null>;
  fitRetryTimer: MutableRefObject<ReturnType<typeof setTimeout> | null>;
};

export function useOverlayWindowLayout({
  style,
  styleRef,
  settings,
  setStyle,
  setSettings,
  setToolbarSide,
  setOverlayHovered,
  setUnlockFeedback,
  clearResizeState,
  finishResizeRef,
  unlockFeedbackTimer,
  toolbarRef,
  offsetLabel,
  vertical,
  fitLimits,
  setFitLimits,
  minimumHorizontalWidth,
  minimumVerticalHeight,
  setToolbarMinimums,
  lastRequestedSize,
  fitRetryTimer,
}: UseOverlayWindowLayoutOptions) {
  useEffect(() => {
    styleRef.current = style;
  }, [style, styleRef]);

  useEffect(() => () => {
    lastRequestedSize.current = null;
    if (fitRetryTimer.current !== null) clearTimeout(fitRetryTimer.current);
  }, [fitRetryTimer, lastRequestedSize]);

  useEffect(() => {
    document.documentElement.dataset.window = "overlay";
    if (!isTauriRuntime()) return;
    void api.getOverlayStyle().then((saved) => {
      styleRef.current = saved;
      setStyle(saved);
    });
    void api.getOverlaySettings().then(setSettings);
    void api.getOverlayToolbarPlacement().then(setToolbarSide);
    const cleanupStyleListener = createTauriListenerCleanup(listen<OverlayStyle>("overlay://style", ({ payload }) => {
      clearResizeState();
      styleRef.current = payload;
      setStyle(payload);
    }));
    const cleanupSettingsListener = createTauriListenerCleanup(listen<OverlaySettings>("overlay://settings", ({ payload }) => {
      if (payload.locked) clearResizeState();
      if (payload.locked || !payload.visible) setOverlayHovered(false);
      setSettings(payload);
    }));
    const cleanupHoverListener = createTauriListenerCleanup(listen<boolean>("overlay://hover", ({ payload }) => {
      setOverlayHovered(payload);
    }));
    const cleanupToolbarPlacementListener = createTauriListenerCleanup(
      listen<ToolbarPlacement>("overlay://toolbar-placement", ({ payload }) => setToolbarSide(payload)),
    );
    const cleanupUnlockFeedbackListener = createTauriListenerCleanup(listen("overlay://unlock-feedback", () => {
      if (unlockFeedbackTimer.current !== null) clearTimeout(unlockFeedbackTimer.current);
      setUnlockFeedback(true);
      unlockFeedbackTimer.current = setTimeout(() => {
        unlockFeedbackTimer.current = null;
        setUnlockFeedback(false);
      }, 1_500);
    }));
    return () => {
      if (unlockFeedbackTimer.current !== null) clearTimeout(unlockFeedbackTimer.current);
      cleanupStyleListener();
      cleanupSettingsListener();
      cleanupHoverListener();
      cleanupToolbarPlacementListener();
      cleanupUnlockFeedbackListener();
    };
  }, [clearResizeState, setOverlayHovered, setSettings, setStyle, setToolbarSide, setUnlockFeedback, styleRef, unlockFeedbackTimer]);

  useEffect(() => {
    const clearSelection = () => {
      const selection = window.getSelection();
      if (selection && selection.rangeCount > 0) selection.removeAllRanges();
    };
    const preventSelection = (event: Event) => {
      event.preventDefault();
      clearSelection();
    };
    clearSelection();
    document.addEventListener("selectstart", preventSelection);
    document.addEventListener("selectionchange", clearSelection);
    document.addEventListener("dragstart", preventSelection);
    return () => {
      document.removeEventListener("selectstart", preventSelection);
      document.removeEventListener("selectionchange", clearSelection);
      document.removeEventListener("dragstart", preventSelection);
    };
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    const refreshLimits = async () => {
      const monitor = await currentMonitor();
      if (!monitor) return;
      const width = monitor.workArea.size.width / monitor.scaleFactor - 48;
      const height = monitor.workArea.size.height / monitor.scaleFactor - 48;
      setFitLimits({ width: Math.max(190, width), height: Math.max(76, height) });
    };
    void refreshLimits();
  }, [setFitLimits]);

  useEffect(() => {
    const onBlur = () => finishResizeRef.current();
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("blur", onBlur);
      clearResizeState();
    };
  }, [clearResizeState, finishResizeRef]);

  useEffect(() => {
    if (settings.locked) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.key.startsWith("Arrow")) return;
      const step = event.shiftKey ? 10 : 1;
      const movement: Record<string, [number, number]> = {
        ArrowLeft: [-step, 0],
        ArrowRight: [step, 0],
        ArrowUp: [0, -step],
        ArrowDown: [0, step],
      };
      const delta = movement[event.key];
      if (!delta) return;
      event.preventDefault();
      void api.nudgeOverlay(delta[0], delta[1]);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [settings.locked]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    const overlayWindow = getCurrentWindow();
    void overlayWindow.setMinSize(new LogicalSize(
      vertical ? 190 : minimumHorizontalWidth,
      vertical ? minimumVerticalHeight : 76,
    ));
    void overlayWindow.setMaxSize(new LogicalSize(fitLimits.width, fitLimits.height));
  }, [fitLimits.height, fitLimits.width, minimumHorizontalWidth, minimumVerticalHeight, vertical]);

  useLayoutEffect(() => {
    const toolbar = toolbarRef.current;
    if (!toolbar || settings.locked) return;
    const measureToolbar = () => {
      const measured = vertical
        ? Math.ceil(toolbar.scrollHeight + VERTICAL_TOOLBAR_WINDOW_INSET)
        : Math.ceil(toolbar.scrollWidth + HORIZONTAL_TOOLBAR_WINDOW_INSET);
      const minimum = Math.max(vertical ? MIN_VERTICAL_HEIGHT : MIN_HORIZONTAL_WIDTH, measured);
      setToolbarMinimums((current) => {
        const key = vertical ? "vertical" : "horizontal";
        return current[key] === minimum ? current : { ...current, [key]: minimum };
      });
    };
    measureToolbar();
    const observer = new ResizeObserver(measureToolbar);
    observer.observe(toolbar);
    return () => observer.disconnect();
  }, [offsetLabel, setToolbarMinimums, settings.locked, toolbarRef, vertical]);
}
