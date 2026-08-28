import { invoke } from "./core";
import type { NotchWindowFitResponse } from "./core";
import type {
  OverlayResizeBounds,
  OverlayResizeEdge,
  OverlaySettings,
  OverlayStyle,
  ToolbarPlacement,
 } from "./types";

export const overlayApi = {
  setOverlayVisible: (visible: boolean) => invoke<void>("set_overlay_visible", { visible }),
  getOverlaySettings: () => invoke<OverlaySettings>("get_overlay_settings"),
  setOverlayLocked: (locked: boolean) => invoke<void>("set_overlay_locked", { locked }),
  getOverlayStyle: () => invoke<OverlayStyle>("get_overlay_style"),
  getOverlayToolbarPlacement: () =>
    invoke<ToolbarPlacement>("get_overlay_toolbar_placement"),
  setOverlayStyle: (style: OverlayStyle) =>
    invoke<OverlayStyle>("set_overlay_style", { style }),
  startOverlayDrag: () => invoke<void>("start_overlay_drag"),
  nudgeOverlay: (dx: number, dy: number) => invoke<void>("nudge_overlay", { dx, dy }),
  resetOverlayBounds: () => invoke<OverlayStyle>("reset_overlay_bounds"),
  resizeOverlayEdge: (edge: OverlayResizeEdge, mainSize: number, minimumMainSize: number) =>
    invoke<OverlayResizeBounds>("resize_overlay_edge", { edge, mainSize, minimumMainSize }),
  fitOverlayContent: (width: number, height: number) =>
    invoke<boolean>("fit_overlay_content", { width, height }),
  fitNotchLyricsContent: (width: number, height: number) =>
    invoke<NotchWindowFitResponse>("fit_notch_lyrics_content", { width, height }),
};
