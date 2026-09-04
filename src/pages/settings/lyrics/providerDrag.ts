import type { ProviderDragState } from "../shared/SettingsContext";

const PROVIDER_DRAG_HYSTERESIS_PX = 6;

export function continueProviderDrag(
  current: ProviderDragState,
  pointerId: number,
  currentY: number,
): ProviderDragState {
  if (current.pointerId !== pointerId) return current;
  const movement = currentY - current.currentY;
  if (movement === 0) return current;
  const sourceCenter = current.positions[current.sourceIndex].center;
  const draggedCenter = sourceCenter + currentY - current.startY;
  let targetIndex = current.targetIndex;
  if (movement > 0) {
    while (targetIndex < current.positions.length - 1) {
      const boundaryIndex = targetIndex < current.sourceIndex ? targetIndex : targetIndex + 1;
      if (draggedCenter <= current.positions[boundaryIndex].center + PROVIDER_DRAG_HYSTERESIS_PX) break;
      targetIndex += 1;
    }
  } else {
    while (targetIndex > 0) {
      const boundaryIndex = targetIndex > current.sourceIndex ? targetIndex : targetIndex - 1;
      if (draggedCenter >= current.positions[boundaryIndex].center - PROVIDER_DRAG_HYSTERESIS_PX) break;
      targetIndex -= 1;
    }
  }
  return { ...current, currentY, targetIndex };
}

export function providerDragTransform(state: ProviderDragState | null, index: number) {
  if (!state) return undefined;
  if (index === state.sourceIndex) {
    return `translate3d(0, ${state.currentY - state.startY}px, 0) scale(1.015)`;
  }
  if (state.targetIndex > state.sourceIndex && index > state.sourceIndex && index <= state.targetIndex) {
    return `translate3d(0, ${state.positions[index - 1].top - state.positions[index].top}px, 0)`;
  }
  if (state.targetIndex < state.sourceIndex && index >= state.targetIndex && index < state.sourceIndex) {
    return `translate3d(0, ${state.positions[index + 1].top - state.positions[index].top}px, 0)`;
  }
  return undefined;
}
