import { getCurrentWindow } from "@tauri-apps/api/window";
import type { PointerEvent as ReactPointerEvent } from "react";
import { api, isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import styles from "./LyricsListWindow.module.scss";

export type ResizeDirection = Parameters<ReturnType<typeof getCurrentWindow>["startResizeDragging"]>[0];

const resizeDirections: Array<{ direction: ResizeDirection; className: string }> = [
  { direction: "North", className: styles.resizeNorth },
  { direction: "South", className: styles.resizeSouth },
  { direction: "East", className: styles.resizeEast },
  { direction: "West", className: styles.resizeWest },
  { direction: "NorthEast", className: styles.resizeNorthEast },
  { direction: "NorthWest", className: styles.resizeNorthWest },
  { direction: "SouthEast", className: styles.resizeSouthEast },
  { direction: "SouthWest", className: styles.resizeSouthWest },
];

type UseListLyricsWindowOptions = {
  locked: boolean;
};

export function useListLyricsWindow({ locked }: UseListLyricsWindowOptions) {
  const startWindowDrag = (event: ReactPointerEvent<HTMLElement>) => {
    if (!isTauriRuntime() || locked || event.button !== 0 || event.detail > 1) return;
    if ((event.target as HTMLElement).closest("button, [role='slider'], [data-no-window-drag]")) return;
    event.preventDefault();
    void getCurrentWindow().startDragging().catch((error) => {
      reportFrontendError("Failed to drag the lyrics window", error);
    });
  };

  const startResize = (direction: ResizeDirection) => (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!isTauriRuntime() || locked || event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    void getCurrentWindow().startResizeDragging(direction).catch((error) => {
      reportFrontendError("Failed to resize the lyrics window", error);
    });
  };

  const resetWindowSize = () => {
    if (!isTauriRuntime()) return;
    void api.resetListLyricsWindowSize().catch((error) => {
      reportFrontendError("Failed to reset the lyrics window size", error);
    });
  };

  const openStyleSettings = () => {
    void api.showLyricsStyleSettings("listWindow");
  };

  const openQuickLyrics = () => {
    void api.showQuickLyricsWindow();
  };

  return {
    resizeDirections,
    startWindowDrag,
    startResize,
    resetWindowSize,
    openStyleSettings,
    openQuickLyrics,
  };
}
