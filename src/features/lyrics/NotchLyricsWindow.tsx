import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ClockArrowLeft,
  ClockArrowRight,
  EyeOff,
  Minus,
  PanelsTopBottom,
  PanelTop,
  Plus,
  Settings,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { IconButton } from "@/components/ui/icon-button";
import { api, isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import { useAppConfig } from "../config/AppConfigProvider";
import { usePlayback } from "../player/usePlayback";
import {
  createTauriListenerCleanup,
  NOTCH_WIDTH_PREVIEW_EVENT,
  type NotchWidthPreviewPayload,
} from "../../shared/tauriEvent";
import { useLyricsPresentation } from "./useLyricsPresentation";
import type { LyricsLine, NotchLayoutMetrics } from "../../shared/types";
import styles from "./NotchLyricsWindow.module.scss";

const LOOP_MARQUEE_SPEED_PX_PER_SECOND = 28;
const LOOP_MARQUEE_START_PAUSE_MS = 1_000;
const LOOP_MARQUEE_END_PAUSE_MS = 900;
const LOOP_MARQUEE_HOME_PAUSE_MS = 900;
const LYRIC_MARQUEE_SPEED_PX_PER_SECOND = 35;
const DEFAULT_LYRIC_MARQUEE_DURATION_MS = 4_000;
const MIN_LYRIC_MARQUEE_DURATION_MS = 100;
const HOVER_COLLAPSE_DELAY_MS = 300;
const WIDTH_ANIMATION_MS = 160;
const WIDTH_ANIMATION_FALLBACK_MS = WIDTH_ANIMATION_MS + 60;
const NOTCH_MAX_WIDTH = 640;
const WINDOW_HORIZONTAL_PADDING = 16;
const TOOLBAR_HORIZONTAL_PADDING = 24;
const TOOLBAR_VERTICAL_SPACE = 46;

const emptyLayout: NotchLayoutMetrics = {
  hasNotch: false,
  topInset: 0,
  centerGapWidth: 0,
};

function formatOffset(offsetMs: number) {
  if (offsetMs === 0) return "0s";
  const seconds = (Math.abs(offsetMs) / 1000).toFixed(3).replace(/\.?0+$/, "");
  return `${offsetMs > 0 ? "+" : "−"}${seconds}s`;
}

function formatOffsetMs(offsetMs: number) {
  if (offsetMs === 0) return "0ms";
  return `${offsetMs > 0 ? "+" : "−"}${Math.abs(offsetMs)}ms`;
}

function OverflowText({ children, contentKey, paused, align = "left", behavior = "loop", maxDurationMs = null }: {
  children: ReactNode;
  contentKey: string;
  paused: boolean;
  align?: "left" | "center" | "right";
  behavior?: "loop" | "once";
  maxDurationMs?: number | null;
}) {
  const viewportRef = useRef<HTMLSpanElement>(null);
  const contentRef = useRef<HTMLSpanElement>(null);
  const animationRef = useRef<Animation | null>(null);

  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    const content = contentRef.current;
    if (!viewport || !content) return;

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    let frame = 0;
    let resetForPause = false;

    const stopAtHome = () => {
      if (resetForPause) return;
      cancelAnimationFrame(frame);
      animationRef.current?.cancel();
      animationRef.current = null;
      content.style.transform = "translateX(0)";
      resetForPause = true;
    };

    const measure = () => {
      if (paused) {
        stopAtHome();
        return;
      }
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        animationRef.current?.cancel();
        animationRef.current = null;
        content.style.transform = "translateX(0)";

        const distance = Math.max(0, content.scrollWidth - viewport.clientWidth);
        if (distance <= 1 || reducedMotion.matches) return;

        if (behavior === "once") {
          const preferredDurationMs = Math.max(
            DEFAULT_LYRIC_MARQUEE_DURATION_MS,
            distance / LYRIC_MARQUEE_SPEED_PX_PER_SECOND * 1_000,
          );
          const duration = maxDurationMs === null
            ? preferredDurationMs
            : Math.min(preferredDurationMs, maxDurationMs);
          animationRef.current = content.animate([
            { transform: "translateX(0)", offset: 0 },
            { transform: "translateX(0)", offset: 0.12 },
            { transform: `translateX(-${distance}px)`, offset: 0.88 },
            { transform: `translateX(-${distance}px)`, offset: 1 },
          ], {
            duration,
            easing: "ease-in-out",
            fill: "forwards",
            iterations: 1,
          });
          return;
        }

        const travelMs = distance / LOOP_MARQUEE_SPEED_PX_PER_SECOND * 1_000;
        const totalMs = LOOP_MARQUEE_START_PAUSE_MS
          + travelMs
          + LOOP_MARQUEE_END_PAUSE_MS
          + travelMs
          + LOOP_MARQUEE_HOME_PAUSE_MS;
        const atStartEnd = LOOP_MARQUEE_START_PAUSE_MS / totalMs;
        const atFarEdge = (LOOP_MARQUEE_START_PAUSE_MS + travelMs) / totalMs;
        const leaveFarEdge = (LOOP_MARQUEE_START_PAUSE_MS + travelMs + LOOP_MARQUEE_END_PAUSE_MS) / totalMs;
        const arriveHome = (LOOP_MARQUEE_START_PAUSE_MS + travelMs + LOOP_MARQUEE_END_PAUSE_MS + travelMs) / totalMs;

        animationRef.current = content.animate([
          { transform: "translateX(0)", offset: 0 },
          { transform: "translateX(0)", offset: atStartEnd },
          { transform: `translateX(-${distance}px)`, offset: atFarEdge },
          { transform: `translateX(-${distance}px)`, offset: leaveFarEdge },
          { transform: "translateX(0)", offset: arriveHome },
          { transform: "translateX(0)", offset: 1 },
        ], {
          duration: totalMs,
          easing: "linear",
          iterations: Infinity,
        });
      });
    };

    const observer = new ResizeObserver(measure);
    observer.observe(viewport);
    observer.observe(content);
    reducedMotion.addEventListener("change", measure);
    measure();

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      reducedMotion.removeEventListener("change", measure);
      animationRef.current?.cancel();
      animationRef.current = null;
    };
  }, [behavior, contentKey, maxDurationMs, paused]);

  return (
    <span className={styles.overflowViewport} data-align={align} ref={viewportRef}>
      <span className={styles.overflowContent} ref={contentRef}>{children}</span>
    </span>
  );
}

function KaraokeLine({ line, positionMs }: { line: LyricsLine; positionMs: number }) {
  const words = line.words?.filter((word) => word.text.length > 0) ?? [];
  if (words.length === 0) return <span>{line.text}</span>;

  return (
    <span className={styles.karaokeText}>
      {words.map((word, index) => {
        const duration = Math.max(0, word.endMs - word.startMs);
        const progress = positionMs <= word.startMs
          ? 0
          : duration === 0 || positionMs >= word.endMs
            ? 100
            : ((positionMs - word.startMs) / duration) * 100;
        return (
          <span
            className={styles.karaokeWord}
            key={`${word.startMs}-${index}`}
            style={{ "--word-progress": `${progress}%` } as CSSProperties}
          >
            <span className={styles.karaokeWordBase}>{word.text}</span>
            <span aria-hidden="true" className={styles.karaokeWordFill}>{word.text}</span>
          </span>
        );
      })}
    </span>
  );
}

export default function NotchLyricsWindow() {
  const { t } = useTranslation();
  const { config, setLyricsDisplayPreferences } = useAppConfig();
  const playback = usePlayback();
  const lyrics = useLyricsPresentation(playback.snapshot, playback.positionMs);
  const [layout, setLayout] = useState(emptyLayout);
  const [expanded, setExpanded] = useState(false);
  const [expandedWidth, setExpandedWidth] = useState(0);
  const [previewWidth, setPreviewWidth] = useState<number | null>(null);
  const [previewActive, setPreviewActive] = useState(false);
  const [widthMotionActive, setWidthMotionActive] = useState(false);
  const [offsetPreview, setOffsetPreview] = useState<{
    trackKey: string;
    offsetMs: number;
  } | null>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);
  const collapseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const widthMotionTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingWindowShrinkRef = useRef(false);
  const expandedRef = useRef(false);
  const previewActiveRef = useRef(false);
  const previewWidthRef = useRef<number | null>(null);
  const latestHoverRef = useRef(false);
  const notchRef = useRef(config.lyrics.displays.notch);
  const lastFitRequestRef = useRef<string | null>(null);
  const lastObservedGeometryRef = useRef({ contentHeight: -1, toolbarWidth: -1 });
  const pendingOffsetRef = useRef(0);
  const offsetWriteQueue = useRef<Promise<void>>(Promise.resolve());
  const offsetWriteVersionRef = useRef(0);
  const notch = config.lyrics.displays.notch;
  const appearance = notch.appearance;
  const effectiveWidth = previewWidth ?? appearance.maxWidth;
  const marqueePaused = previewActive || widthMotionActive;
  const originalLines = lyrics.document?.tracks.original.lines ?? [];
  const beforeFirstLine = lyrics.activeIndex < 0;
  const primaryLine = lyrics.currentLine ?? (beforeFirstLine ? originalLines[0] : null);
  const secondaryLine = beforeFirstLine ? originalLines[1] ?? null : lyrics.nextLine;
  const selectedSupportingLines = [
    ...(notch.showTranslation && lyrics.currentTranslation?.text.trim()
      ? [{ kind: "translation" as const, line: lyrics.currentTranslation }]
      : []),
    ...(notch.showRomanization && lyrics.currentRomanization?.text.trim()
      ? [{ kind: "romanization" as const, line: lyrics.currentRomanization }]
      : []),
  ];
  const selectedSupportingLine = selectedSupportingLines[0];
  const supportingLines = !notch.showTwoLines
    ? []
    : selectedSupportingLine
      ? [selectedSupportingLine]
      : secondaryLine?.text.trim()
        ? [{ kind: "next" as const, line: secondaryLine }]
        : [];
  const currentLineDisplayEndMs = lyrics.nextLine?.startMs ?? lyrics.currentLine?.endMs;
  const lyricMarqueeTimeLimitMs = lyrics.currentLine && currentLineDisplayEndMs != null
    ? Math.max(
      MIN_LYRIC_MARQUEE_DURATION_MS,
      currentLineDisplayEndMs - lyrics.currentLine.startMs,
    )
    : null;
  const translationAvailable = Boolean(lyrics.document?.tracks.translation);
  const romanizationAvailable = Boolean(lyrics.document?.tracks.romanization);
  const offsetAvailable = Boolean(lyrics.document && lyrics.trackKey);
  const runtimeOffsetMs = lyrics.document?.offsetMs ?? 0;
  const runtimeOffsetRef = useRef({ trackKey: lyrics.trackKey, offsetMs: runtimeOffsetMs });
  runtimeOffsetRef.current = { trackKey: lyrics.trackKey, offsetMs: runtimeOffsetMs };
  const offsetMs = offsetPreview?.trackKey === lyrics.trackKey
    ? offsetPreview.offsetMs
    : runtimeOffsetMs;
  const fontAtMinimum = appearance.fontSize <= 12;
  const fontAtMaximum = appearance.fontSize >= 32;
  const offsetResetUnavailable = !offsetAvailable || offsetMs === 0;
  const offsetLabel = offsetAvailable ? formatOffset(offsetMs) : "—";
  const offsetValueTitle = offsetAvailable
    ? offsetMs === 0
      ? t("overlay.toolbar.offsetZeroTitle")
      : t("overlay.toolbar.offsetTitle", { value: formatOffsetMs(offsetMs) })
    : t("overlay.toolbar.noOffset");
  const layoutValue = t(`overlay.layout.${notch.showTwoLines ? "double" : "single"}`);

  useLayoutEffect(() => {
    notchRef.current = notch;
  }, [notch]);

  useEffect(() => {
    if (offsetPreview?.trackKey === lyrics.trackKey) {
      if (offsetPreview.offsetMs === runtimeOffsetMs) setOffsetPreview(null);
      return;
    }
    pendingOffsetRef.current = runtimeOffsetMs;
  }, [lyrics.trackKey, offsetPreview, runtimeOffsetMs]);

  useEffect(() => {
    offsetWriteVersionRef.current += 1;
    setOffsetPreview(null);
    pendingOffsetRef.current = runtimeOffsetMs;
  }, [lyrics.trackKey]);

  const saveNotch = useCallback((next: typeof notch) => {
    notchRef.current = next;
    void setLyricsDisplayPreferences("notch", next).catch((error) => {
      reportFrontendError("Failed to update Dynamic Island lyrics preferences", error);
    });
  }, [setLyricsDisplayPreferences]);

  const patchNotch = useCallback((patch: Partial<typeof notch>) => {
    saveNotch({ ...notchRef.current, ...patch });
  }, [saveNotch]);

  const patchAppearance = useCallback((patch: Partial<typeof appearance>) => {
    const current = notchRef.current;
    saveNotch({ ...current, appearance: { ...current.appearance, ...patch } });
  }, [saveNotch]);

  const setLyricsOffset = (nextOffsetMs: number) => {
    if (!lyrics.trackKey) return;
    const version = offsetWriteVersionRef.current + 1;
    offsetWriteVersionRef.current = version;
    pendingOffsetRef.current = nextOffsetMs;
    const trackKey = lyrics.trackKey;
    setOffsetPreview({ trackKey, offsetMs: nextOffsetMs });
    const operation = offsetWriteQueue.current
      .then(() => api.setLyricsOffset(trackKey, nextOffsetMs));
    offsetWriteQueue.current = operation.then(
      () => undefined,
      (error) => {
        if (offsetWriteVersionRef.current === version) {
          setOffsetPreview(null);
          if (runtimeOffsetRef.current.trackKey === trackKey) {
            pendingOffsetRef.current = runtimeOffsetRef.current.offsetMs;
          }
        }
        reportFrontendError("Failed to update the Dynamic Island lyrics offset", error);
      },
    );
  };

  const changeLyricsOffset = (deltaMs: number) => {
    setLyricsOffset(pendingOffsetRef.current + deltaMs);
  };

  const fitWindow = useCallback((
    nextExpanded: boolean,
    collapsedWidthOverride?: number,
    reservedWidth?: number,
  ) => {
    const content = contentRef.current;
    const toolbar = toolbarRef.current;
    if (!content || !toolbar) return;
    const collapsedWidth = collapsedWidthOverride ?? notchRef.current.appearance.maxWidth;
    const toolbarWidth = Math.ceil(toolbar.scrollWidth + TOOLBAR_HORIZONTAL_PADDING);
    const nextExpandedWidth = Math.max(collapsedWidth, toolbarWidth);
    setExpandedWidth(nextExpandedWidth);
    if (!isTauriRuntime()) return;
    const contentWidth = reservedWidth ?? (nextExpanded ? nextExpandedWidth : collapsedWidth);
    const width = contentWidth + WINDOW_HORIZONTAL_PADDING;
    const height = Math.ceil(content.scrollHeight + (nextExpanded ? TOOLBAR_VERTICAL_SPACE : 0));
    const requestKey = `${width}:${height}`;
    if (lastFitRequestRef.current === requestKey) return;
    lastFitRequestRef.current = requestKey;
    void api.fitNotchLyricsContent(width, height).catch((error) => {
      if (lastFitRequestRef.current === requestKey) lastFitRequestRef.current = null;
      reportFrontendError("Failed to fit the Dynamic Island lyrics window", error);
    });
  }, []);

  useLayoutEffect(() => {
    if (!previewActiveRef.current) fitWindow(expandedRef.current);
  }, [appearance.maxWidth, fitWindow]);

  useEffect(() => {
    if (previewActive || previewWidth === null || previewWidth !== appearance.maxWidth) return;
    previewWidthRef.current = null;
    setPreviewWidth(null);
  }, [appearance.maxWidth, previewActive, previewWidth]);

  const setExpandedState = useCallback((next: boolean) => {
    expandedRef.current = next;
    setExpanded(next);
  }, []);

  const clearCollapseTimers = useCallback(() => {
    if (collapseTimerRef.current !== null) clearTimeout(collapseTimerRef.current);
    collapseTimerRef.current = null;
  }, []);

  const finishWidthMotion = useCallback(() => {
    if (widthMotionTimerRef.current !== null) clearTimeout(widthMotionTimerRef.current);
    widthMotionTimerRef.current = null;
    setWidthMotionActive(false);
    if (!pendingWindowShrinkRef.current) return;
    pendingWindowShrinkRef.current = false;
    if (!previewActiveRef.current) fitWindow(false);
  }, [fitWindow]);

  const startWidthMotion = useCallback((shrinkWindowAfterward: boolean) => {
    if (widthMotionTimerRef.current !== null) clearTimeout(widthMotionTimerRef.current);
    pendingWindowShrinkRef.current = shrinkWindowAfterward;
    setWidthMotionActive(true);
    widthMotionTimerRef.current = setTimeout(finishWidthMotion, WIDTH_ANIMATION_FALLBACK_MS);
  }, [finishWidthMotion]);

  const cancelWidthMotion = useCallback(() => {
    if (widthMotionTimerRef.current !== null) clearTimeout(widthMotionTimerRef.current);
    widthMotionTimerRef.current = null;
    pendingWindowShrinkRef.current = false;
    setWidthMotionActive(false);
  }, []);

  const expandIsland = useCallback(() => {
    clearCollapseTimers();
    fitWindow(true);
    if (expandedRef.current) return;
    startWidthMotion(false);
    requestAnimationFrame(() => setExpandedState(true));
  }, [clearCollapseTimers, fitWindow, setExpandedState, startWidthMotion]);

  const scheduleCollapse = useCallback(() => {
    clearCollapseTimers();
    collapseTimerRef.current = setTimeout(() => {
      collapseTimerRef.current = null;
      if (!expandedRef.current) return;
      startWidthMotion(true);
      setExpandedState(false);
    }, HOVER_COLLAPSE_DELAY_MS);
  }, [clearCollapseTimers, setExpandedState, startWidthMotion]);

  const applyHoverState = useCallback((hovered: boolean) => {
    if (hovered) expandIsland();
    else scheduleCollapse();
  }, [expandIsland, scheduleCollapse]);

  const handlePointerHover = useCallback((hovered: boolean) => {
    latestHoverRef.current = hovered;
    if (!previewActiveRef.current) applyHoverState(hovered);
  }, [applyHoverState]);

  const supportingToggleTitle = (track: string, enabled: boolean, available: boolean) => {
    const action = enabled ? t("overlay.toolbar.hideTrack", { track }) : t("overlay.toolbar.showTrack", { track });
    return available ? action : t("notchLyrics.toolbar.unavailableTrack", { action, track });
  };

  useEffect(() => {
    void api.getNotchLayoutMetrics().then(setLayout).catch(() => undefined);
    return createTauriListenerCleanup(
      listen<NotchLayoutMetrics>("notch://layout", ({ payload }) => setLayout(payload)),
    );
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    return createTauriListenerCleanup(
      listen<boolean>("notch://hover", ({ payload }) => {
        handlePointerHover(payload);
      }),
    );
  }, [handlePointerHover]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    return createTauriListenerCleanup(
      listen<NotchWidthPreviewPayload>(NOTCH_WIDTH_PREVIEW_EVENT, ({ payload }) => {
        if (payload.phase === "update") {
          const startingPreview = !previewActiveRef.current;
          previewActiveRef.current = true;
          previewWidthRef.current = payload.width;
          setPreviewActive(true);
          setPreviewWidth(payload.width);
          if (startingPreview) {
            clearCollapseTimers();
            cancelWidthMotion();
            const toolbarWidth = Math.ceil(
              (toolbarRef.current?.scrollWidth ?? 0) + TOOLBAR_HORIZONTAL_PADDING,
            );
            fitWindow(
              expandedRef.current,
              payload.width,
              Math.max(NOTCH_MAX_WIDTH, toolbarWidth),
            );
          }
          return;
        }

        previewActiveRef.current = false;
        setPreviewActive(false);
        if (payload.phase === "commit") {
          previewWidthRef.current = payload.width;
          setPreviewWidth(payload.width);
          fitWindow(expandedRef.current, payload.width);
        } else {
          previewWidthRef.current = null;
          setPreviewWidth(null);
          fitWindow(expandedRef.current);
        }
        applyHoverState(latestHoverRef.current);
      }),
    );
  }, [applyHoverState, cancelWidthMotion, clearCollapseTimers, fitWindow]);

  useLayoutEffect(() => {
    const content = contentRef.current;
    const toolbar = toolbarRef.current;
    if (!content || !toolbar) return;
    let frame = 0;
    const measure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const contentHeight = Math.ceil(content.scrollHeight);
        const toolbarWidth = Math.ceil(toolbar.scrollWidth);
        const previous = lastObservedGeometryRef.current;
        if (previous.contentHeight === contentHeight && previous.toolbarWidth === toolbarWidth) return;
        lastObservedGeometryRef.current = { contentHeight, toolbarWidth };
        if (previewActiveRef.current) {
          setExpandedWidth(Math.max(
            previewWidthRef.current ?? notchRef.current.appearance.maxWidth,
            toolbarWidth + TOOLBAR_HORIZONTAL_PADDING,
          ));
          return;
        }
        fitWindow(expandedRef.current);
      });
    };
    const observer = new ResizeObserver(measure);
    observer.observe(content);
    observer.observe(toolbar);
    measure();
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [fitWindow]);

  useEffect(() => () => {
    clearCollapseTimers();
    if (widthMotionTimerRef.current !== null) clearTimeout(widthMotionTimerRef.current);
  }, [clearCollapseTimers]);

  return (
    <main
      className={styles.shell}
      data-expanded={expanded || undefined}
      data-has-notch={layout.hasNotch || undefined}
      data-width-preview={previewActive || undefined}
      style={{
        "--notch-font-family": appearance.fontFamily,
        "--notch-font-size": `${appearance.fontSize}px`,
        "--notch-font-weight": appearance.fontWeight,
        "--notch-active-color": appearance.activeColor,
        "--notch-inactive-color": appearance.inactiveColor,
        "--notch-translation-color": appearance.translationColor,
        "--notch-romanization-color": appearance.romanizationColor,
        "--notch-radius": `${appearance.borderRadius}px`,
        "--notch-max-width": `${effectiveWidth}px`,
        "--notch-expanded-width": `${Math.max(effectiveWidth, expandedWidth)}px`,
        "--notch-top-inset": `${layout.topInset}px`,
        "--notch-center-gap": `${layout.centerGapWidth}px`,
      } as CSSProperties}
    >
      <section
        aria-expanded={expanded}
        aria-live="polite"
        className={styles.island}
        onPointerEnter={() => handlePointerHover(true)}
        onPointerLeave={() => handlePointerHover(false)}
        onTransitionEnd={(event) => {
          if (event.target === event.currentTarget && event.propertyName === "width") {
            finishWidthMotion();
          }
        }}
      >
        <div className={styles.content} ref={contentRef}>
          <header className={styles.metadata}>
            <strong title={playback.snapshot.title ?? undefined}>
              <OverflowText
                contentKey={`${playback.snapshot.trackId ?? "fallback"}:title:${playback.snapshot.title ?? ""}`}
                paused={marqueePaused}
              >
                {playback.snapshot.title ?? "Lyrics Plus"}
              </OverflowText>
            </strong>
            <span className={styles.notchGap} aria-hidden="true" />
            <span className={styles.artist} title={playback.snapshot.artist ?? undefined}>
              <OverflowText
                align="right"
                contentKey={`${playback.snapshot.trackId ?? "fallback"}:artist:${playback.snapshot.artist ?? ""}`}
                paused={marqueePaused}
              >
                {playback.snapshot.artist ?? ""}
              </OverflowText>
            </span>
          </header>
          {primaryLine && (
            <div className={styles.currentLine} key={`${primaryLine.startMs}:${primaryLine.text}`}>
              <OverflowText
                align="center"
                behavior="once"
                contentKey={`${primaryLine.startMs}:${primaryLine.text}`}
                maxDurationMs={lyricMarqueeTimeLimitMs}
                paused={marqueePaused}
              >
                <KaraokeLine line={primaryLine} positionMs={lyrics.adjustedPositionMs} />
              </OverflowText>
            </div>
          )}
          {supportingLines.map(({ kind, line }) => (
            <div className={styles.supportingLine} data-kind={kind} key={`${kind}:${line.startMs}:${line.text}`}>
              <OverflowText
                align="center"
                behavior="once"
                contentKey={`${kind}:${line.startMs}:${line.text}`}
                maxDurationMs={lyricMarqueeTimeLimitMs}
                paused={marqueePaused}
              >
                {line.text}
              </OverflowText>
            </div>
          ))}
        </div>
        <div className={styles.toolbarReveal}>
          <div className={styles.toolbarRevealInner}>
            <div className={styles.toolbar} role="toolbar" aria-label={t("notchLyrics.toolbar.label")} ref={toolbarRef}>
              <IconButton label={t("overlay.toolbar.decreaseFont")} tooltip={fontAtMinimum ? t("notchLyrics.toolbar.minimumFontSize") : undefined} variant="ghost" size="icon-sm" aria-disabled={fontAtMinimum} onClick={() => {
                if (fontAtMinimum) return;
                patchAppearance({ fontSize: Math.max(12, notchRef.current.appearance.fontSize - 2) });
              }}><Minus /></IconButton>
              <IconButton label={t("overlay.toolbar.increaseFont")} tooltip={fontAtMaximum ? t("notchLyrics.toolbar.maximumFontSize") : undefined} variant="ghost" size="icon-sm" aria-disabled={fontAtMaximum} onClick={() => {
                if (fontAtMaximum) return;
                patchAppearance({ fontSize: Math.min(32, notchRef.current.appearance.fontSize + 2) });
              }}><Plus /></IconButton>
              <div className={styles.offsetControl} role="group" aria-label={t("overlay.toolbar.offsetGroup", { value: offsetAvailable ? formatOffsetMs(offsetMs) : t("overlay.toolbar.unavailable") })}>
                <IconButton label={t("overlay.toolbar.delay")} tooltip={offsetAvailable ? t("overlay.toolbar.delayTitle") : t("notchLyrics.toolbar.unavailableOffset")} variant="ghost" size="icon-sm" aria-disabled={!offsetAvailable} onClick={(event) => {
                  if (!offsetAvailable) return;
                  changeLyricsOffset(event.shiftKey ? -500 : -100);
                }}><ClockArrowLeft /></IconButton>
                <IconButton className={styles.offsetValue} label={!offsetAvailable ? t("overlay.toolbar.noOffset") : offsetMs === 0 ? t("overlay.toolbar.zeroOffset") : t("overlay.toolbar.offsetReset", { value: formatOffsetMs(offsetMs) })} tooltip={offsetValueTitle} variant="ghost" size="icon-sm" aria-disabled={offsetResetUnavailable} onClick={() => {
                  if (offsetResetUnavailable) return;
                  setLyricsOffset(0);
                }}>{offsetLabel}</IconButton>
                <IconButton label={t("overlay.toolbar.advance")} tooltip={offsetAvailable ? t("overlay.toolbar.advanceTitle") : t("notchLyrics.toolbar.unavailableOffset")} variant="ghost" size="icon-sm" aria-disabled={!offsetAvailable} onClick={(event) => {
                  if (!offsetAvailable) return;
                  changeLyricsOffset(event.shiftKey ? 500 : 100);
                }}><ClockArrowRight /></IconButton>
              </div>
              <IconButton
                label={t("overlay.toolbar.toggleLayout", { value: layoutValue })}
                tooltip={t("overlay.toolbar.toggleLayoutTitle", { value: layoutValue })}
                variant="ghost"
                size="icon-sm"
                onClick={() => patchNotch({ showTwoLines: !notchRef.current.showTwoLines })}
              >{notch.showTwoLines ? <PanelsTopBottom /> : <PanelTop />}</IconButton>
              <IconButton label={supportingToggleTitle(t("common.feature.translation"), notch.showTranslation, translationAvailable)} tooltip={supportingToggleTitle(t("common.feature.translation"), notch.showTranslation, translationAvailable)} variant="ghost" size="icon-sm" className={styles.trackToggle} data-available={translationAvailable} data-on={notch.showTranslation} aria-pressed={notch.showTranslation} onClick={() => patchNotch({ showTranslation: !notchRef.current.showTranslation })}>{t("overlay.toolbar.translationGlyph")}</IconButton>
              <IconButton label={supportingToggleTitle(t("common.feature.romanization"), notch.showRomanization, romanizationAvailable)} tooltip={supportingToggleTitle(t("common.feature.romanization"), notch.showRomanization, romanizationAvailable)} variant="ghost" size="icon-sm" className={styles.trackToggle} data-available={romanizationAvailable} data-on={notch.showRomanization} aria-pressed={notch.showRomanization} onClick={() => patchNotch({ showRomanization: !notchRef.current.showRomanization })}>{t("overlay.toolbar.romanizationGlyph")}</IconButton>
              <IconButton label={t("notchLyrics.toolbar.hide")} variant="ghost" size="icon-sm" onClick={() => void api.setNotchLyricsVisible(false)}><EyeOff /></IconButton>
              <IconButton label={t("notchLyrics.toolbar.openSettings")} variant="ghost" size="icon-sm" onClick={() => void api.showMainWindow()}><Settings /></IconButton>
            </div>
          </div>
        </div>
      </section>
    </main>
  );
}
