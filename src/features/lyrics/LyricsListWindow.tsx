import { useEffect, useMemo, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowDownToLine,
  ClockArrowLeft,
  ClockArrowRight,
  EyeOff,
  Minus,
  Music2,
  Pin,
  Plus,
  RotateCcw,
  Search,
  Settings,
  Square,
  SquareDashed,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { api, isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import type { LyricsLine, ListLyricsPreferences } from "../../shared/types";
import { useAppConfig } from "../config/AppConfigProvider";
import { usePlayback } from "../player/usePlayback";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { IconButton } from "@/components/ui/icon-button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { findAlignedAuxiliaryLine } from "./useLyrics";
import { useLyricsPresentation } from "./useLyricsPresentation";
import styles from "./LyricsListWindow.module.scss";

type ResizeDirection = Parameters<ReturnType<typeof getCurrentWindow>["startResizeDragging"]>[0];

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

function formatOffset(value: number) {
  if (value === 0) return "0";
  if (Math.abs(value) >= 1_000 && value % 1_000 === 0) return `${value > 0 ? "+" : ""}${value / 1_000}s`;
  return `${value > 0 ? "+" : ""}${value}ms`;
}

export default function LyricsListWindow() {
  const { t } = useTranslation();
  const { config, setLyricsDisplayPreferences } = useAppConfig();
  const playback = usePlayback();
  const lyrics = useLyricsPresentation(playback.snapshot, playback.positionMs);
  const activeRef = useRef<HTMLDivElement>(null);
  const toolbarHideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingOffsetRef = useRef(0);
  const offsetWriteQueue = useRef<Promise<unknown>>(Promise.resolve());
  const [following, setFollowing] = useState(true);
  const [toolbarVisible, setToolbarVisible] = useState(false);
  const options = config.lyrics.displays.listWindow;
  const appearance = options.appearance;
  const lines = lyrics.document?.tracks.original.lines ?? [];
  const offsetAvailable = Boolean(lyrics.document && lyrics.trackKey);
  const offsetMs = lyrics.document?.offsetMs ?? 0;
  const transparentBackground = appearance.backgroundMode === "transparent";
  const translationAvailable = Boolean(lyrics.document?.tracks.translation);
  const romanizationAvailable = Boolean(lyrics.document?.tracks.romanization);

  useEffect(() => setFollowing(true), [lyrics.trackKey]);

  useEffect(() => {
    pendingOffsetRef.current = offsetMs;
  }, [offsetMs]);

  useEffect(() => () => {
    if (toolbarHideTimer.current !== null) clearTimeout(toolbarHideTimer.current);
  }, []);

  useEffect(() => {
    if (!following || !activeRef.current) return;
    activeRef.current.scrollIntoView({
      block: "center",
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
    });
  }, [following, lyrics.activeIndex]);

  const auxiliary = useMemo(() => lines.map((line) => ({
    translation: options.showTranslation && lyrics.document?.tracks.translation
      ? findAlignedAuxiliaryLine(lyrics.document.tracks.translation.lines, line)
      : null,
    romanization: options.showRomanization && lyrics.document?.tracks.romanization
      ? findAlignedAuxiliaryLine(lyrics.document.tracks.romanization.lines, line)
      : null,
  })), [lines, lyrics.document, options.showRomanization, options.showTranslation]);

  const updatePreferences = (next: ListLyricsPreferences) =>
    setLyricsDisplayPreferences("listWindow", next).then(() => true).catch((error) => {
      reportFrontendError("Failed to update list lyrics preferences", error);
      return false;
    });

  const updateAppearance = (patch: Partial<ListLyricsPreferences["appearance"]>) =>
    updatePreferences({ ...options, appearance: { ...appearance, ...patch } });

  const pauseFollowing = () => {
    if (lines.length > 0) setFollowing(false);
  };

  const resumeFollowing = () => {
    setFollowing(true);
    requestAnimationFrame(() => activeRef.current?.scrollIntoView({
      block: "center",
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
    }));
  };

  const showToolbar = () => {
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

  const setLyricsOffset = (nextOffsetMs: number) => {
    if (!lyrics.trackKey) return;
    pendingOffsetRef.current = nextOffsetMs;
    const trackKey = lyrics.trackKey;
    offsetWriteQueue.current = offsetWriteQueue.current
      .then(() => api.setLyricsOffset(trackKey, nextOffsetMs))
      .catch((error) => reportFrontendError("Failed to update list lyrics offset", error));
  };

  const changeLyricsOffset = (deltaMs: number) => {
    setLyricsOffset(pendingOffsetRef.current + deltaMs);
  };

  const startWindowDrag = (event: ReactPointerEvent<HTMLElement>) => {
    if (!isTauriRuntime() || event.button !== 0 || event.detail > 1) return;
    if ((event.target as HTMLElement).closest("button, [role='slider'], [data-no-window-drag]")) return;
    event.preventDefault();
    void getCurrentWindow().startDragging().catch((error) => {
      reportFrontendError("Failed to drag the list lyrics window", error);
    });
  };

  const startResize = (direction: ResizeDirection) => (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!isTauriRuntime() || event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    void getCurrentWindow().startResizeDragging(direction).catch((error) => {
      reportFrontendError("Failed to resize the list lyrics window", error);
    });
  };

  const resetWindowSize = () => {
    if (!isTauriRuntime()) return;
    void api.resetListLyricsWindowSize().catch((error) => {
      reportFrontendError("Failed to reset the list lyrics window size", error);
    });
  };

  const supportingToggleTitle = (track: string, enabled: boolean, available: boolean) => {
    const action = enabled
      ? t("overlay.toolbar.hideTrack", { track })
      : t("overlay.toolbar.showTrack", { track });
    return available ? action : t("lyricsList.toolbar.unavailableTrack", { action, track });
  };

  const backgroundLabel = transparentBackground
    ? t("overlay.toolbar.backgroundTransparent")
    : t("overlay.toolbar.backgroundVisible");

  const title = playback.snapshot.title ?? t("lyricsList.noTrack");
  const artist = playback.snapshot.artist ?? t("lyricsList.waiting");

  return (
    <main
      className={styles.shell}
      data-background-mode={appearance.backgroundMode}
      data-following={following}
      data-toolbar-visible={toolbarVisible}
      onMouseEnter={showToolbar}
      onMouseLeave={() => scheduleToolbarHide()}
      style={{
        "--list-font-family": appearance.fontFamily,
        "--list-font-size": `${appearance.fontSize}px`,
        "--list-font-weight": appearance.fontWeight,
        "--list-secondary-scale": appearance.secondaryFontScale,
        "--list-line-height": appearance.lineHeight,
        "--list-line-gap": `${appearance.lineGap}px`,
        "--list-active-color": appearance.activeColor,
        "--list-inactive-color": appearance.inactiveColor,
        "--list-translation-color": appearance.translationColor,
        "--list-romanization-color": appearance.romanizationColor,
        "--list-active-background": appearance.activeBackgroundColor,
        "--list-background": appearance.backgroundColor,
        "--list-background-opacity": transparentBackground ? 0 : appearance.backgroundOpacity,
        "--list-alignment": appearance.alignment,
      } as CSSProperties}
    >
      <div className={styles.background} aria-hidden="true" />

      {transparentBackground && (
        <div className={styles.dragRegion} aria-hidden="true" onPointerDown={startWindowDrag} />
      )}

      <header className={styles.header} onPointerDown={startWindowDrag}>
        <div className={styles.track}>
          <h1>{title}</h1>
          <p>{artist}</p>
        </div>
        {lyrics.document && <span className={styles.source}>{lyrics.document.metadata.source}</span>}
      </header>

      <div
        className={styles.toolbar}
        data-no-window-drag
        role="toolbar"
        aria-label={t("lyricsList.toolbar.label")}
        onFocusCapture={showToolbar}
        onBlurCapture={() => scheduleToolbarHide()}
      >
        <IconButton label={t("lyricsList.toolbar.decreaseFont")} variant="ghost" size="icon-sm" disabled={appearance.fontSize <= 12} onClick={() => void updateAppearance({ fontSize: Math.max(12, appearance.fontSize - 2) })}><Minus /></IconButton>
        <IconButton label={t("lyricsList.toolbar.increaseFont")} variant="ghost" size="icon-sm" disabled={appearance.fontSize >= 56} onClick={() => void updateAppearance({ fontSize: Math.min(56, appearance.fontSize + 2) })}><Plus /></IconButton>
        <div className={styles.offsetControl} role="group" aria-label={t("lyricsList.toolbar.offsetGroup", { value: formatOffset(offsetMs) })}>
          <IconButton label={t("lyricsList.toolbar.delay")} variant="ghost" size="icon-sm" disabled={!offsetAvailable} onClick={(event) => changeLyricsOffset(event.shiftKey ? -500 : -100)}><ClockArrowLeft /></IconButton>
          <IconButton className={styles.offsetValue} label={t("lyricsList.toolbar.resetOffset", { value: formatOffset(offsetMs) })} variant="ghost" size="icon-sm" disabled={!offsetAvailable || offsetMs === 0} onClick={() => setLyricsOffset(0)}>{offsetAvailable ? formatOffset(offsetMs) : "—"}</IconButton>
          <IconButton label={t("lyricsList.toolbar.advance")} variant="ghost" size="icon-sm" disabled={!offsetAvailable} onClick={(event) => changeLyricsOffset(event.shiftKey ? 500 : 100)}><ClockArrowRight /></IconButton>
        </div>
        <IconButton
          label={t("overlay.toolbar.toggleBackground", { value: backgroundLabel })}
          tooltip={t("overlay.toolbar.toggleBackgroundTitle", { value: backgroundLabel })}
          variant="ghost"
          size="icon-sm"
          aria-pressed={!transparentBackground}
          data-on={!transparentBackground}
          onClick={() => void updateAppearance({ backgroundMode: transparentBackground ? "solid" : "transparent" })}
        >{transparentBackground ? <SquareDashed /> : <Square />}</IconButton>
        <IconButton
          className={styles.trackToggle}
          label={supportingToggleTitle(t("common.feature.translation"), options.showTranslation, translationAvailable)}
          tooltip={supportingToggleTitle(t("common.feature.translation"), options.showTranslation, translationAvailable)}
          variant="ghost"
          size="icon-sm"
          aria-pressed={options.showTranslation}
          data-available={translationAvailable}
          data-on={options.showTranslation}
          onClick={() => void updatePreferences({ ...options, showTranslation: !options.showTranslation })}
        >{t("overlay.toolbar.translationGlyph")}</IconButton>
        <IconButton
          className={styles.trackToggle}
          label={supportingToggleTitle(t("common.feature.romanization"), options.showRomanization, romanizationAvailable)}
          tooltip={supportingToggleTitle(t("common.feature.romanization"), options.showRomanization, romanizationAvailable)}
          variant="ghost"
          size="icon-sm"
          aria-pressed={options.showRomanization}
          data-available={romanizationAvailable}
          data-on={options.showRomanization}
          onClick={() => void updatePreferences({ ...options, showRomanization: !options.showRomanization })}
        >{t("overlay.toolbar.romanizationGlyph")}</IconButton>
        <IconButton label={t("lyricsList.toolbar.openSettings")} variant="ghost" size="icon-sm" onClick={() => void api.showLyricsStyleSettings("listWindow")}><Settings /></IconButton>
        <IconButton
          label={options.alwaysOnTop ? t("lyricsList.toolbar.unpin") : t("lyricsList.toolbar.pin")}
          variant="ghost"
          size="icon-sm"
          aria-pressed={options.alwaysOnTop}
          onClick={() => void updatePreferences({ ...options, alwaysOnTop: !options.alwaysOnTop })}
        ><Pin /></IconButton>
        <IconButton label={t("lyricsList.toolbar.resetSize")} variant="ghost" size="icon-sm" onClick={resetWindowSize}><RotateCcw /></IconButton>
        <IconButton label={t("lyricsList.toolbar.hide")} variant="ghost" size="icon-sm" onClick={() => void updatePreferences({ ...options, enabled: false })}><EyeOff /></IconButton>
      </div>

      {lines.length > 0 ? (
        <div className={styles.workspace}>
          <ScrollArea className={styles.scroller} onWheel={pauseFollowing} onPointerDown={pauseFollowing}>
            <div className={styles.lines} role="list" aria-label={t("lyricsList.lyrics")}>
              {lines.map((line: LyricsLine, index) => {
                const active = index === lyrics.activeIndex;
                const supporting = auxiliary[index];
                return (
                  <div
                    className={cn(styles.line, active && styles.activeLine)}
                    data-active={active || undefined}
                    key={`${line.startMs}:${index}`}
                    ref={active ? activeRef : undefined}
                    role="listitem"
                    aria-current={active ? "true" : undefined}
                  >
                    <p>{line.text || "\u00a0"}</p>
                    {supporting?.translation && <small data-kind="translation">{supporting.translation.text}</small>}
                    {supporting?.romanization && <small data-kind="romanization">{supporting.romanization.text}</small>}
                  </div>
                );
              })}
            </div>
          </ScrollArea>
          {!following && (
            <Button className={styles.followButton} variant="secondary" size="sm" onClick={resumeFollowing}>
              <ArrowDownToLine data-icon="inline-start" />{t("lyricsList.returnCurrent")}
            </Button>
          )}
        </div>
      ) : (
        <Empty className={styles.empty}>
          <EmptyHeader>
            <EmptyMedia variant="icon"><Music2 /></EmptyMedia>
            <EmptyTitle>{lyrics.status === "loading" ? t("lyricsList.loading") : t("lyricsList.empty")}</EmptyTitle>
            <EmptyDescription>{lyrics.error ?? t("lyricsList.emptyHint")}</EmptyDescription>
          </EmptyHeader>
          {playback.snapshot.title && playback.snapshot.artist && (
            <Button variant="outline" onClick={() => void api.showQuickLyricsWindow()}>
              <Search data-icon="inline-start" />{t("lyricsList.chooseLyrics")}
            </Button>
          )}
        </Empty>
      )}

      {resizeDirections.map(({ direction, className }) => (
        <div key={direction} className={cn(styles.resizeHandle, className)} aria-hidden="true" onPointerDown={startResize(direction)} />
      ))}
    </main>
  );
}
