import { useCallback, useRef, useState, type Dispatch, type MutableRefObject, type PointerEvent as ReactPointerEvent, type SetStateAction } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import type { OverlayResizeEdge, OverlayStyle } from "../../shared/types";
import type { ActiveResizeSession } from "./OverlayLayout";

type ResizeAxis = "horizontal" | "vertical";
type ResizeEvent = ReactPointerEvent<HTMLDivElement>;

type UseOverlayResizeOptions = {
  locked: boolean;
  minimumHorizontalWidth: number;
  minimumVerticalHeight: number;
  styleRef: MutableRefObject<OverlayStyle>;
  setStyle: Dispatch<SetStateAction<OverlayStyle>>;
};

export function useOverlayResize({
  locked,
  minimumHorizontalWidth,
  minimumVerticalHeight,
  styleRef,
  setStyle,
}: UseOverlayResizeOptions) {
  const resizeSession = useRef<ActiveResizeSession | null>(null);
  const finishResizeRef = useRef<() => void>(() => undefined);
  const [activeResizeEdge, setActiveResizeEdge] = useState<OverlayResizeEdge | null>(null);
  const resizing = activeResizeEdge !== null;

  const clearResizeState = useCallback(() => {
    const session = resizeSession.current;
    resizeSession.current = null;
    if (session?.handle.hasPointerCapture(session.pointerId)) {
      try { session.handle.releasePointerCapture(session.pointerId); } catch { /* Already released by the system. */ }
    }
    setActiveResizeEdge(null);
  }, []);

  const resizeCoordinate = (event: Pick<ResizeEvent, "screenX" | "screenY">, axis: ResizeAxis) =>
    axis === "horizontal" ? event.screenX : event.screenY;

  const requestedMainSize = (session: ActiveResizeSession) => {
    if (session.startMainSize === null) return null;
    const delta = session.latestCoordinate - session.startCoordinate;
    const direction = session.edge === "left" || session.edge === "top" ? -1 : 1;
    return session.startMainSize + delta * direction;
  };

  const commitResizeSession = (session: ActiveResizeSession) => {
    if (resizeSession.current !== session || session.committing || session.processing || session.pendingMainSize !== null || !session.ending) return;
    session.committing = true;
    const bounds = session.lastBounds;
    if (!bounds) {
      if (session.startMainSize === null) {
        session.committing = false;
        return;
      }
      clearResizeState();
      return;
    }
    const next = {
      ...styleRef.current,
      ...(session.axis === "horizontal"
        ? { horizontalMaxWidth: Math.max(session.minimumMainSize, bounds.width) }
        : { verticalMaxHeight: Math.max(session.minimumMainSize, bounds.height) }),
    };
    styleRef.current = next;
    setStyle(next);
    void api.setOverlayStyle(next).then((saved) => {
      styleRef.current = saved;
      setStyle(saved);
      if (resizeSession.current === session) clearResizeState();
    }).catch((error) => {
      reportFrontendError("Failed to persist the overlay bounds", error);
      if (resizeSession.current === session) clearResizeState();
    });
  };

  const processResizeQueue = async (session: ActiveResizeSession): Promise<void> => {
    if (session.processing || resizeSession.current !== session) return;
    session.processing = true;
    try {
      while (resizeSession.current === session && session.pendingMainSize !== null) {
        const mainSize = session.pendingMainSize;
        session.pendingMainSize = null;
        const bounds = await api.resizeOverlayEdge(session.edge, mainSize, session.minimumMainSize);
        if (resizeSession.current !== session) return;
        session.lastBounds = bounds;
      }
    } catch (error) {
      if (resizeSession.current === session) {
        reportFrontendError("Failed to resize the overlay window", error);
        clearResizeState();
      }
    } finally {
      session.processing = false;
      if (resizeSession.current === session) {
        if (session.pendingMainSize !== null) void processResizeQueue(session);
        else commitResizeSession(session);
      }
    }
  };

  const queueResize = (session: ActiveResizeSession) => {
    const mainSize = requestedMainSize(session);
    if (mainSize === null || resizeSession.current !== session) return;
    session.pendingMainSize = mainSize;
    void processResizeQueue(session);
  };

  const finishResizeSession = (session: ActiveResizeSession, coordinate?: number) => {
    if (resizeSession.current !== session || session.ending) return;
    if (coordinate !== undefined) session.latestCoordinate = coordinate;
    session.ending = true;
    queueResize(session);
    if (session.handle.hasPointerCapture(session.pointerId)) {
      try { session.handle.releasePointerCapture(session.pointerId); } catch { /* Already released by the system. */ }
    }
    commitResizeSession(session);
  };

  finishResizeRef.current = () => {
    const session = resizeSession.current;
    if (session) finishResizeSession(session);
  };

  const beginResize = (edge: OverlayResizeEdge, axis: ResizeAxis) => (event: ResizeEvent) => {
    if (locked || event.button !== 0 || !isTauriRuntime()) return;
    event.preventDefault();
    event.stopPropagation();
    clearResizeState();
    const handle = event.currentTarget;
    const coordinate = resizeCoordinate(event, axis);
    const session: ActiveResizeSession = {
      pointerId: event.pointerId,
      edge,
      axis,
      handle,
      startCoordinate: coordinate,
      latestCoordinate: coordinate,
      startMainSize: null,
      minimumMainSize: axis === "horizontal" ? minimumHorizontalWidth : minimumVerticalHeight,
      pendingMainSize: null,
      lastBounds: null,
      processing: false,
      ending: false,
      committing: false,
    };
    resizeSession.current = session;
    setActiveResizeEdge(edge);
    handle.setPointerCapture(event.pointerId);
    const overlayWindow = getCurrentWindow();
    void Promise.all([overlayWindow.outerSize(), overlayWindow.scaleFactor()]).then(([size, scale]) => {
      if (resizeSession.current !== session) return;
      session.startMainSize = (axis === "horizontal" ? size.width : size.height) / scale;
      queueResize(session);
      commitResizeSession(session);
    }).catch((error) => {
      reportFrontendError("Failed to read the overlay bounds", error);
      if (resizeSession.current === session) clearResizeState();
    });
  };

  const continueResize = (event: ResizeEvent) => {
    const session = resizeSession.current;
    if (!session || session.pointerId !== event.pointerId || session.ending) return;
    event.preventDefault();
    event.stopPropagation();
    session.latestCoordinate = resizeCoordinate(event, session.axis);
    queueResize(session);
  };

  const endResize = (event: ResizeEvent) => {
    const session = resizeSession.current;
    if (!session || session.pointerId !== event.pointerId) return;
    event.preventDefault();
    event.stopPropagation();
    finishResizeSession(session, resizeCoordinate(event, session.axis));
  };

  const cancelResize = (event: ResizeEvent) => {
    const session = resizeSession.current;
    if (!session || session.pointerId !== event.pointerId) return;
    event.preventDefault();
    event.stopPropagation();
    finishResizeSession(session);
  };

  const lostResizeCapture = (event: ResizeEvent) => {
    const session = resizeSession.current;
    if (!session || session.pointerId !== event.pointerId || session.ending) return;
    finishResizeSession(session);
  };

  return {
    activeResizeEdge,
    beginResize,
    cancelResize,
    clearResizeState,
    continueResize,
    endResize,
    finishResizeRef,
    lostResizeCapture,
    resizing,
  };
}
