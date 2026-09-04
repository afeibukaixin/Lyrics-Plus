import { useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { useTranslation } from "react-i18next";

import { api, isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import {
  defaultOverlayStyle,
  secondaryDisplayFlags,
  secondaryDisplayFromFlags,
  type OverlaySettings,
  type OverlayStyle,
} from "../../shared/types";

import { useLyricsPresentation } from "../lyrics/useLyricsPresentation";
import { usePlayback } from "../player/usePlayback";
import { useOverlayLyricsOffset } from "./useOverlayLyricsOffset";

export function useOverlayController() {
  const { t } = useTranslation();
  const playback = usePlayback();
  const lyrics = useLyricsPresentation(playback.snapshot, playback.positionMs, playback.active);
  const [style, setStyle] = useState<OverlayStyle>(defaultOverlayStyle);
  const [settings, setSettings] = useState<OverlaySettings>({ visible: true, locked: false });
  const styleRef = useRef(style);
  const offsetMs = lyrics.document?.offsetMs ?? 0;
  const { setLyricsOffset, changeLyricsOffset } = useOverlayLyricsOffset(lyrics.trackKey, offsetMs);

  const updateStyle = async (patch: Partial<OverlayStyle>) => {
    const next = { ...styleRef.current, ...patch };
    styleRef.current = next;
    setStyle(next);
    const saved = await api.setOverlayStyle(next);
    styleRef.current = saved;
    setStyle(saved);
  };

  const toggleSupportingTrack = (kind: "translation" | "romanization") => {
    const flags = secondaryDisplayFlags(style.secondaryDisplay);
    const translation = kind === "translation" ? !flags.translation : flags.translation;
    const romanization = kind === "romanization" ? !flags.romanization : flags.romanization;
    void updateStyle({ secondaryDisplay: secondaryDisplayFromFlags(translation, romanization) });
  };

  const lockOverlay = () => {
    void api.setOverlayLocked(true);
  };

  const hideOverlay = () => {
    void api.setOverlayVisible(false);
  };

  const openSettings = () => {
    void api.showLyricsStyleSettings("desktop");
  };

  const startWindowDrag = (event: ReactPointerEvent<HTMLElement>) => {
    if (!isTauriRuntime() || settings.locked || event.button !== 0 || event.detail > 1) return;
    const target = event.target as HTMLElement;
    if (target.closest(
      "button, input, select, textarea, [role='slider'], [data-no-window-drag], [data-tauri-drag-region='false']",
    )) return;
    event.preventDefault();
    void api.startOverlayDrag().catch((error) => {
      reportFrontendError("Failed to drag the desktop lyrics window", error);
    });
  };

  return {
    changeLyricsOffset,
    hideOverlay,
    lockOverlay,
    lyrics,
    openSettings,
    playback,
    setLyricsOffset,
    setSettings,
    setStyle,
    settings,
    startWindowDrag,
    style,
    styleRef,
    t,
    toggleSupportingTrack,
    updateStyle,
  };
}
