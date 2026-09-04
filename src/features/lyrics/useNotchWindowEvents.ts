import {
  useEffect,
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauriRuntime } from "../../shared/api";
import {
  createTauriListenerCleanup,
  NOTCH_POINTER_SAMPLE_EVENT,
  NOTCH_VISIBILITY_TRANSITION_EVENT,
  NOTCH_WIDTH_PREVIEW_EVENT,
  type NotchPointerSamplePayload,
  type NotchVisibilityTransitionPayload,
  type NotchWidthPreviewPayload,
} from "../../shared/tauriEvent";
import type { NotchLayoutMetrics } from "../../shared/types";
import type { NotchWidthPreviewValues } from "./NotchLyricsLayout";

type HoverSource = "pointerenter" | "pointermove" | "pointerleave" | "native";

type UseNotchWindowEventsOptions = {
  setLayout: Dispatch<SetStateAction<NotchLayoutMetrics>>;
  applyIslandVisibility: (visible: boolean) => void;
  updateHoverFromPoint: (x: number, y: number, source: HoverSource) => void;
  previewValuesRef: MutableRefObject<NotchWidthPreviewValues | null>;
  setPreviewValues: Dispatch<SetStateAction<NotchWidthPreviewValues | null>>;
  previewActiveRef: MutableRefObject<boolean>;
  setPreviewActive: Dispatch<SetStateAction<boolean>>;
  cancelWidthMotion: () => void;
  fitWindow: (collapsedWidthOverride?: number) => void;
  hostFitReadyRef: MutableRefObject<boolean>;
  pendingHoverApplyRef: MutableRefObject<boolean>;
  reconcileHoverStateRef: MutableRefObject<() => void>;
};

export function useNotchWindowEvents({
  setLayout,
  applyIslandVisibility,
  updateHoverFromPoint,
  previewValuesRef,
  setPreviewValues,
  previewActiveRef,
  setPreviewActive,
  cancelWidthMotion,
  fitWindow,
  hostFitReadyRef,
  pendingHoverApplyRef,
  reconcileHoverStateRef,
}: UseNotchWindowEventsOptions) {
  useEffect(() => {
    void api.getNotchLayoutMetrics().then(setLayout).catch(() => undefined);
    return createTauriListenerCleanup(
      listen<NotchLayoutMetrics>("notch://layout", ({ payload }) => setLayout(payload)),
    );
  }, [setLayout]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let outerFrame = 0;
    let innerFrame = 0;
    const cancelScheduledEntrance = () => {
      if (outerFrame) cancelAnimationFrame(outerFrame);
      if (innerFrame) cancelAnimationFrame(innerFrame);
      outerFrame = 0;
      innerFrame = 0;
    };
    outerFrame = requestAnimationFrame(() => {
      outerFrame = 0;
      innerFrame = requestAnimationFrame(() => {
        innerFrame = 0;
        applyIslandVisibility(true);
      });
    });
    const cleanup = createTauriListenerCleanup(
      listen<NotchVisibilityTransitionPayload>(
        NOTCH_VISIBILITY_TRANSITION_EVENT,
        ({ payload }) => {
          cancelScheduledEntrance();
          applyIslandVisibility(payload.visible);
        },
      ),
    );
    return () => {
      cancelScheduledEntrance();
      cleanup();
    };
  }, [applyIslandVisibility]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    return createTauriListenerCleanup(
      listen<NotchPointerSamplePayload>(NOTCH_POINTER_SAMPLE_EVENT, ({ payload }) => {
        updateHoverFromPoint(payload.clientX, payload.clientY, "native");
      }),
    );
  }, [updateHoverFromPoint]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    return createTauriListenerCleanup(
      listen<NotchWidthPreviewPayload>(NOTCH_WIDTH_PREVIEW_EVENT, ({ payload }) => {
        if (payload.phase === "update" || payload.phase === "commit") {
          const nextPreviewValues = {
            maxWidth: payload.maxWidth,
            expandedMaxWidth: Math.max(payload.maxWidth, payload.expandedMaxWidth),
          };
          previewValuesRef.current = nextPreviewValues;
          setPreviewValues(nextPreviewValues);
          if (payload.phase === "update") {
            const startingPreview = !previewActiveRef.current;
            previewActiveRef.current = true;
            setPreviewActive(true);
            if (startingPreview) {
              cancelWidthMotion();
            }
            fitWindow(nextPreviewValues.maxWidth);
            return;
          }

          previewActiveRef.current = false;
          setPreviewActive(false);
          fitWindow(nextPreviewValues.maxWidth);
          if (hostFitReadyRef.current) {
            pendingHoverApplyRef.current = false;
            reconcileHoverStateRef.current();
          } else pendingHoverApplyRef.current = true;
          return;
        }

        previewActiveRef.current = false;
        setPreviewActive(false);
        previewValuesRef.current = null;
        setPreviewValues(null);
        fitWindow();
        if (hostFitReadyRef.current) {
          pendingHoverApplyRef.current = false;
          reconcileHoverStateRef.current();
        } else pendingHoverApplyRef.current = true;
      }),
    );
  }, [cancelWidthMotion, fitWindow, hostFitReadyRef, pendingHoverApplyRef, previewActiveRef, previewValuesRef, reconcileHoverStateRef, setPreviewActive, setPreviewValues]);
}
