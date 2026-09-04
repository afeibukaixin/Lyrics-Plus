import { useEffect, useRef, useState } from "react";
import { reportFrontendError } from "../../shared/debugLog";
import type { ListLyricsPreferences } from "../../shared/types";

type UseListLyricsToolbarOptions = {
  options: ListLyricsPreferences;
  appearance: ListLyricsPreferences["appearance"];
  setLyricsDisplayPreferences: (
    mode: "listWindow",
    preferences: ListLyricsPreferences,
  ) => Promise<unknown>;
  setListLyricsLocked: (locked: boolean) => Promise<unknown>;
};

export function useListLyricsToolbar({
  options,
  appearance,
  setLyricsDisplayPreferences,
  setListLyricsLocked,
}: UseListLyricsToolbarOptions) {
  const toolbarHideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [toolbarVisible, setToolbarVisible] = useState(false);

  useEffect(() => () => {
    if (toolbarHideTimer.current !== null) clearTimeout(toolbarHideTimer.current);
  }, []);

  useEffect(() => {
    if (options.locked) setToolbarVisible(false);
  }, [options.locked]);

  const updatePreferences = (next: ListLyricsPreferences) =>
    setLyricsDisplayPreferences("listWindow", next).then(() => true).catch((error) => {
      reportFrontendError("Failed to update lyrics window preferences", error);
      return false;
    });

  const updateAppearance = (patch: Partial<ListLyricsPreferences["appearance"]>) =>
    updatePreferences({ ...options, appearance: { ...appearance, ...patch } });

  const updateLocked = (nextLocked: boolean) =>
    setListLyricsLocked(nextLocked).catch((error) => {
      reportFrontendError("Failed to update lyrics window lock state", error);
    });

  const showToolbar = () => {
    if (options.locked) return;
    if (toolbarHideTimer.current !== null) clearTimeout(toolbarHideTimer.current);
    toolbarHideTimer.current = null;
    setToolbarVisible(true);
  };

  const scheduleToolbarHide = () => {
    if (toolbarHideTimer.current !== null) clearTimeout(toolbarHideTimer.current);
    toolbarHideTimer.current = setTimeout(() => {
      toolbarHideTimer.current = null;
      setToolbarVisible(false);
    }, 500);
  };

  return {
    toolbarVisible,
    showToolbar,
    scheduleToolbarHide,
    updatePreferences,
    updateAppearance,
    updateLocked,
  };
}
