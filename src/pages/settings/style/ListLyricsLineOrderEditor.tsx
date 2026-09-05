import { GripVertical } from "lucide-react";
import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { ListLyricsLineKind, ListLyricsLineOrder } from "../../../shared/types";
import styles from "../settings.module.scss";

const DRAG_START_THRESHOLD_PX = 6;
const DRAG_HYSTERESIS_PX = 6;

type RowPosition = { left: number; center: number };

type DragState = {
  kind: ListLyricsLineKind;
  pointerId: number;
  sourceIndex: number;
  targetIndex: number;
  startX: number;
  currentX: number;
  started: boolean;
  positions: RowPosition[];
};

type ListLyricsLineOrderEditorProps = {
  order: ListLyricsLineOrder;
  labels: Record<ListLyricsLineKind, string>;
  dragLabel: (kind: ListLyricsLineKind) => string;
  disabled?: boolean;
  onChange: (order: ListLyricsLineOrder) => Promise<void>;
};

function sameOrder(left: ListLyricsLineOrder, right: ListLyricsLineOrder) {
  return left.every((kind, index) => kind === right[index]);
}

function updateDragState(current: DragState, pointerId: number, currentX: number): DragState {
  if (current.pointerId !== pointerId) return current;
  const distanceFromStart = currentX - current.startX;
  if (!current.started && Math.abs(distanceFromStart) < DRAG_START_THRESHOLD_PX) {
    return { ...current, currentX };
  }
  const movement = currentX - (current.started ? current.currentX : current.startX);
  if (movement === 0) return current;
  const sourceCenter = current.positions[current.sourceIndex].center;
  const draggedCenter = sourceCenter + currentX - current.startX;
  let targetIndex = current.targetIndex;

  if (movement > 0) {
    while (targetIndex < current.positions.length - 1) {
      const boundaryIndex = targetIndex < current.sourceIndex ? targetIndex : targetIndex + 1;
      if (draggedCenter <= current.positions[boundaryIndex].center + DRAG_HYSTERESIS_PX) break;
      targetIndex += 1;
    }
  } else {
    while (targetIndex > 0) {
      const boundaryIndex = targetIndex > current.sourceIndex ? targetIndex : targetIndex - 1;
      if (draggedCenter >= current.positions[boundaryIndex].center - DRAG_HYSTERESIS_PX) break;
      targetIndex -= 1;
    }
  }

  return { ...current, currentX, targetIndex, started: true };
}

function dragTransform(state: DragState | null, index: number) {
  if (!state) return undefined;
  if (index === state.sourceIndex) {
    if (!state.started) return undefined;
    return `translate3d(${state.currentX - state.startX}px, 0, 0) scale(1.015)`;
  }
  if (state.targetIndex > state.sourceIndex && index > state.sourceIndex && index <= state.targetIndex) {
    return `translate3d(${state.positions[index - 1].left - state.positions[index].left}px, 0, 0)`;
  }
  if (state.targetIndex < state.sourceIndex && index >= state.targetIndex && index < state.sourceIndex) {
    return `translate3d(${state.positions[index + 1].left - state.positions[index].left}px, 0, 0)`;
  }
  return undefined;
}

export default function ListLyricsLineOrderEditor({
  order,
  labels,
  dragLabel,
  disabled = false,
  onChange,
}: ListLyricsLineOrderEditorProps) {
  const rowRefs = useRef(new Map<ListLyricsLineKind, HTMLDivElement>());
  const dragRef = useRef<DragState | null>(null);
  const pendingPointerRef = useRef<{ pointerId: number; currentX: number } | null>(null);
  const frameRef = useRef<number | null>(null);
  const [drag, setDrag] = useState<DragState | null>(null);
  const [pendingOrder, setPendingOrder] = useState<ListLyricsLineOrder | null>(null);
  const displayedOrder = pendingOrder ?? order;
  const interactionDisabled = disabled || pendingOrder !== null;

  useEffect(() => () => {
    if (frameRef.current !== null) cancelAnimationFrame(frameRef.current);
  }, []);

  useEffect(() => {
    if (pendingOrder && sameOrder(order, pendingOrder)) setPendingOrder(null);
  }, [order, pendingOrder]);

  const cancelScheduledDragUpdate = () => {
    if (frameRef.current !== null) {
      cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    }
    pendingPointerRef.current = null;
  };

  const applyScheduledDragUpdate = () => {
    frameRef.current = null;
    const pending = pendingPointerRef.current;
    const current = dragRef.current;
    if (!pending || !current || pending.pointerId !== current.pointerId) return;
    const next = updateDragState(current, pending.pointerId, pending.currentX);
    pendingPointerRef.current = null;
    if (next === current) return;
    dragRef.current = next;
    setDrag(next);
  };

  const scheduleDragUpdate = (pointerId: number, currentX: number) => {
    pendingPointerRef.current = { pointerId, currentX };
    if (frameRef.current === null) frameRef.current = requestAnimationFrame(applyScheduledDragUpdate);
  };

  const cancelDrag = () => {
    cancelScheduledDragUpdate();
    dragRef.current = null;
    setDrag(null);
  };

  const beginDrag = (kind: ListLyricsLineKind, sourceIndex: number, event: ReactPointerEvent<HTMLButtonElement>) => {
    if (interactionDisabled || dragRef.current || !event.isPrimary) return;
    if (event.pointerType === "mouse" && event.button !== 0) return;
    const positions = displayedOrder.map((item) => {
      const bounds = rowRefs.current.get(item)?.getBoundingClientRect();
      return bounds ? { left: bounds.left, center: bounds.left + bounds.width / 2 } : null;
    });
    if (positions.some((position) => position === null)) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    const nextDrag: DragState = {
      kind,
      pointerId: event.pointerId,
      sourceIndex,
      targetIndex: sourceIndex,
      startX: event.clientX,
      currentX: event.clientX,
      started: false,
      positions: positions as RowPosition[],
    };
    dragRef.current = nextDrag;
    setDrag(nextDrag);
  };

  const continueDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (!dragRef.current || dragRef.current.pointerId !== event.pointerId) return;
    event.preventDefault();
    scheduleDragUpdate(event.pointerId, event.clientX);
  };

  const finishDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    const current = dragRef.current;
    if (!current || current.pointerId !== event.pointerId) return;
    event.preventDefault();
    cancelScheduledDragUpdate();
    const settled = updateDragState(current, event.pointerId, event.clientX);
    dragRef.current = settled;
    const { sourceIndex, targetIndex } = settled;
    dragRef.current = null;
    setDrag(null);
    if (sourceIndex === targetIndex) return;

    const next = [...displayedOrder];
    const [moved] = next.splice(sourceIndex, 1);
    if (!moved) return;
    next.splice(targetIndex, 0, moved);
    const nextOrder = next as ListLyricsLineOrder;
    setPendingOrder(nextOrder);
    void onChange(nextOrder).catch(() => setPendingOrder(null));
  };

  return (
    <div className={styles.lineOrderEditor} data-dragging={Boolean(drag)} role="list" aria-label={labels.original}>
      {displayedOrder.map((kind, index) => (
        <div
          className={styles.lineOrderItem}
          data-dragging={drag?.kind === kind}
          key={kind}
          ref={(element) => {
            if (element) rowRefs.current.set(kind, element);
            else rowRefs.current.delete(kind);
          }}
          role="listitem"
          style={{ transform: dragTransform(drag, index) }}
        >
          <Badge className={styles.lineOrderTag} variant="secondary">
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className={styles.lineOrderHandle}
              aria-label={dragLabel(kind)}
              disabled={interactionDisabled}
              onPointerDown={(event) => beginDrag(kind, index, event)}
              onPointerMove={continueDrag}
              onPointerUp={finishDrag}
              onPointerCancel={cancelDrag}
              onLostPointerCapture={cancelDrag}
            >
              <GripVertical />
            </Button>
            <span>{labels[kind]}</span>
          </Badge>
        </div>
      ))}
    </div>
  );
}
