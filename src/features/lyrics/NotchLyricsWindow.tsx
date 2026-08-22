import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { flushSync } from "react-dom";
import type { TFunction } from "i18next";
import { listen } from "@tauri-apps/api/event";
import { useGSAP } from "@gsap/react";
import { gsap } from "gsap";
import { Flip } from "gsap/Flip";
import {
  Pause,
  Play,
  SkipBack,
  SkipForward,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import appIconUrl from "../../../src-tauri/icons/32x32.png";
import { IconButton } from "@/components/ui/icon-button";
import { Slider } from "@/components/ui/slider";
import { api, isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import { useAppConfig } from "../config/AppConfigProvider";
import { usePlayback } from "../player/usePlayback";
import { usePlaybackSpectrum } from "../player/usePlaybackSpectrum";
import {
  createTauriListenerCleanup,
  NOTCH_VISIBILITY_TRANSITION_EVENT,
  NOTCH_WIDTH_PREVIEW_EVENT,
  type NotchWidthPreviewTarget,
  type NotchVisibilityTransitionPayload,
  type NotchWidthPreviewPayload,
} from "../../shared/tauriEvent";
import { useLyricsPresentation } from "./useLyricsPresentation";
import type { CompactKaraokeStyle, LyricsLine, NotchLayoutMetrics, NotchSlotContent, PlaybackSpectrumBands } from "../../shared/types";
import styles from "./NotchLyricsWindow.module.scss";

gsap.registerPlugin(useGSAP, Flip);

const LOOP_MARQUEE_SPEED_PX_PER_SECOND = 28;
const LOOP_MARQUEE_START_PAUSE_MS = 1_000;
const LOOP_MARQUEE_END_PAUSE_MS = 900;
const LOOP_MARQUEE_HOME_PAUSE_MS = 900;
const LYRIC_MARQUEE_SPEED_PX_PER_SECOND = 35;
const DEFAULT_LYRIC_MARQUEE_DURATION_MS = 4_000;
const MIN_LYRIC_MARQUEE_DURATION_MS = 100;
const HOVER_COLLAPSE_DELAY_MS = 140;
const NOTCH_MAX_WIDTH = 640;
const WINDOW_HORIZONTAL_PADDING = 16;
const TOOLBAR_HORIZONTAL_PADDING = 24;
const NO_NOTCH_TOP_GAP = 6;

const emptyLayout: NotchLayoutMetrics = {
  hasNotch: false,
  topInset: 0,
  centerGapWidth: 0,
};

function formatPlaybackTime(valueMs: number | null) {
  const totalSeconds = Math.max(0, Math.floor((valueMs ?? 0) / 1_000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function SpectrumBars({ bands }: { bands: PlaybackSpectrumBands }) {
  return (
    <span className={styles.spectrum} aria-hidden="true">
      {bands.slice(0, 8).map((band, index) => (
        <span className={styles.spectrumBar} key={index} style={{ "--spectrum-level": `${Math.max(12, Math.round(band * 100))}%` } as CSSProperties} />
      ))}
    </span>
  );
}

type PlaybackController = ReturnType<typeof usePlayback>;

function ExpandedPlayer({ playback, t }: { playback: PlaybackController; t: TFunction }) {
  const trackKey = playback.snapshot.trackId ?? "fallback";
  const durationMs = playback.snapshot.durationMs ?? 0;
  const canSeek = durationMs > 0 && Boolean(playback.snapshot.player);
  const [draftPositionMs, setDraftPositionMs] = useState<number | null>(null);
  const positionMs = durationMs > 0
    ? Math.min(durationMs, Math.max(0, draftPositionMs ?? playback.positionMs))
    : 0;

  useEffect(() => {
    setDraftPositionMs(null);
  }, [trackKey]);

  const commitPosition = (value: number) => {
    const nextPosition = Math.min(durationMs, Math.max(0, Math.round(value)));
    setDraftPositionMs(nextPosition);
    void playback.seekTo(nextPosition)
      .catch(() => undefined)
      .finally(() => setDraftPositionMs(null));
  };

  return (
    <div className={styles.player}>
      <div className={styles.playerMetadata}>
        <div className={styles.playerArtwork}>
          <img alt="" draggable={false} src={playback.artworkUrl ?? appIconUrl} />
        </div>
        <div className={styles.playerTrack}>
          <strong title={playback.snapshot.title ?? undefined}>{playback.snapshot.title ?? "Lyrics Plus"}</strong>
          <span title={playback.snapshot.artist ?? undefined}>{playback.snapshot.artist ?? ""}</span>
        </div>
      </div>
      <div className={styles.playerProgress}>
        <div className={styles.playerTimes}>
          <span>{formatPlaybackTime(positionMs)}</span>
          <span>−{formatPlaybackTime(Math.max(0, durationMs - positionMs))}</span>
        </div>
        <Slider
          aria-label={t("notchLyrics.player.seek")}
          className={styles.playerSlider}
          disabled={!canSeek || playback.isControlling}
          max={Math.max(1, durationMs)}
          min={0}
          onValueChange={(value) => setDraftPositionMs(Number(value))}
          onValueCommitted={(value) => commitPosition(Number(value))}
          step={1_000}
          value={positionMs}
        />
      </div>
      <div className={styles.playerControls} role="group" aria-label={t("notchLyrics.player.label")}>
        <IconButton label={t("notchLyrics.player.previous")} variant="ghost" size="icon-sm" onClick={() => void playback.previousTrack().catch(() => undefined)}><SkipBack /></IconButton>
        <IconButton className={styles.playerPrimaryControl} label={playback.snapshot.isPlaying ? t("notchLyrics.player.pause") : t("notchLyrics.player.play")} variant="ghost" size="icon" onClick={() => void playback.togglePlayPause().catch(() => undefined)}>{playback.snapshot.isPlaying ? <Pause /> : <Play />}</IconButton>
        <IconButton label={t("notchLyrics.player.next")} variant="ghost" size="icon-sm" onClick={() => void playback.nextTrack().catch(() => undefined)}><SkipForward /></IconButton>
      </div>
    </div>
  );
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

function KaraokeLine({ line, positionMs, karaokeStyle }: { line: LyricsLine; positionMs: number; karaokeStyle: CompactKaraokeStyle }) {
  const words = line.words?.filter((word) => word.text.length > 0) ?? [];
  if (words.length === 0) return <span>{line.text}</span>;

  return (
    <span className={styles.karaokeText} data-karaoke={karaokeStyle}>
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
  const { config } = useAppConfig();
  const playback = usePlayback();
  const lyrics = useLyricsPresentation(playback.snapshot, playback.positionMs);
  const [layout, setLayout] = useState(emptyLayout);
  const [expanded, setExpanded] = useState(false);
  const [expandedWidth, setExpandedWidth] = useState(0);
  const [previewWidth, setPreviewWidth] = useState<number | null>(null);
  const [previewTarget, setPreviewTarget] = useState<NotchWidthPreviewTarget | null>(null);
  const [previewActive, setPreviewActive] = useState(false);
  const [widthMotionActive, setWidthMotionActive] = useState(false);
  const [islandVisible, setIslandVisible] = useState(() => !isTauriRuntime());
  const [visibilityMotionActive, setVisibilityMotionActive] = useState(isTauriRuntime);
  const shellRef = useRef<HTMLElement>(null);
  const islandRef = useRef<HTMLElement>(null);
  const islandSurfaceRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const toolbarRevealRef = useRef<HTMLDivElement>(null);
  const collapseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const presenceTimelineRef = useRef<gsap.core.Timeline | null>(null);
  const widthFlipRef = useRef<gsap.core.Animation | null>(null);
  const toolbarMotionRef = useRef<gsap.core.Animation | null>(null);
  const pendingWindowShrinkRef = useRef(false);
  const expandedRef = useRef(false);
  const islandVisibleRef = useRef(islandVisible);
  const visibilityMotionActiveRef = useRef(visibilityMotionActive);
  const widthMotionActiveRef = useRef(false);
  const reducedMotionRef = useRef(false);
  const previewActiveRef = useRef(false);
  const previewWidthRef = useRef<number | null>(null);
  const previewTargetRef = useRef<NotchWidthPreviewTarget | null>(null);
  const latestHoverRef = useRef(false);
  const notchRef = useRef(config.lyrics.displays.notch);
  const lastFitRequestRef = useRef<string | null>(null);
  const lastObservedGeometryRef = useRef({ contentHeight: -1, playerWidth: -1 });
  const applyHoverStateRef = useRef<(hovered: boolean) => void>(() => undefined);
  const notch = config.lyrics.displays.notch;
  const appearance = notch.appearance;
  const usesSpectrum = notch.leftSlot === "spectrum" || notch.rightSlot === "spectrum";
  const spectrum = usePlaybackSpectrum(usesSpectrum);
  const effectiveWidth = previewTarget === "collapsed" && previewWidth !== null
    ? previewWidth
    : appearance.maxWidth;
  const effectiveExpandedMaxWidth = Math.min(
    NOTCH_MAX_WIDTH,
    Math.max(
      effectiveWidth,
      previewTarget === "expanded" && previewWidth !== null
        ? previewWidth
        : appearance.expandedMaxWidth,
    ),
  );
  const marqueePaused = previewActive || widthMotionActive || visibilityMotionActive;
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
  const supportingLines = notch.layout !== "double"
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
  useLayoutEffect(() => {
    notchRef.current = notch;
  }, [notch]);

  const fitWindow = useCallback((
    nextExpanded: boolean,
    collapsedWidthOverride?: number,
  ) => {
    const content = nextExpanded ? toolbarRevealRef.current : contentRef.current;
    if (!content) return;
    const collapsedWidth = collapsedWidthOverride ?? notchRef.current.appearance.maxWidth;
    const expandedMaxWidth = Math.min(
      NOTCH_MAX_WIDTH,
      Math.max(
        collapsedWidth,
        previewTargetRef.current === "expanded" && previewWidthRef.current !== null
          ? previewWidthRef.current
          : notchRef.current.appearance.expandedMaxWidth,
      ),
    );
    const contentWidth = Math.ceil(content.scrollWidth + (nextExpanded ? TOOLBAR_HORIZONTAL_PADDING : 0));
    const nextExpandedWidth = Math.min(expandedMaxWidth, Math.max(collapsedWidth, contentWidth));
    setExpandedWidth(nextExpandedWidth);
    if (!isTauriRuntime()) return;
    const requestedWidth = nextExpanded ? nextExpandedWidth : collapsedWidth;
    const width = requestedWidth + WINDOW_HORIZONTAL_PADDING;
    const height = Math.ceil(content.scrollHeight + (layout.hasNotch ? 0 : NO_NOTCH_TOP_GAP));
    const requestKey = `${width}:${height}`;
    if (lastFitRequestRef.current === requestKey) return;
    lastFitRequestRef.current = requestKey;
    void api.fitNotchLyricsContent(width, height).catch((error) => {
      if (lastFitRequestRef.current === requestKey) lastFitRequestRef.current = null;
      reportFrontendError("Failed to fit the Dynamic Island lyrics window", error);
    });
  }, [layout.hasNotch]);

  useLayoutEffect(() => {
    if (!previewActiveRef.current) fitWindow(expandedRef.current);
  }, [appearance.expandedMaxWidth, appearance.maxWidth, fitWindow]);

  useEffect(() => {
    const committedWidth = previewTarget === "collapsed"
      ? appearance.maxWidth
      : previewTarget === "expanded"
        ? appearance.expandedMaxWidth
        : null;
    if (previewActive || previewTarget === null || previewWidth === null || previewWidth !== committedWidth) return;
    previewWidthRef.current = null;
    previewTargetRef.current = null;
    setPreviewWidth(null);
    setPreviewTarget(null);
  }, [appearance.expandedMaxWidth, appearance.maxWidth, previewActive, previewTarget, previewWidth]);

  const setExpandedState = useCallback((next: boolean) => {
    expandedRef.current = next;
    setExpanded(next);
  }, []);

  const clearCollapseTimers = useCallback(() => {
    if (collapseTimerRef.current !== null) clearTimeout(collapseTimerRef.current);
    collapseTimerRef.current = null;
  }, []);

  const finishWidthMotion = useCallback(() => {
    widthFlipRef.current = null;
    toolbarMotionRef.current = null;
    widthMotionActiveRef.current = false;
    setWidthMotionActive(false);
    const island = islandRef.current;
    const toolbarReveal = toolbarRevealRef.current;
    if (island) {
      gsap.set(island, {
        clearProps: "width,height,transform,position,top,left,willChange",
      });
    }
    if (toolbarReveal) {
      gsap.set(toolbarReveal, {
        clearProps: "opacity,visibility,transform,height,gridTemplateRows,overflow",
      });
    }
    if (!pendingWindowShrinkRef.current) return;
    pendingWindowShrinkRef.current = false;
    if (!previewActiveRef.current) fitWindow(false);
  }, [fitWindow]);

  const cancelWidthMotion = useCallback((preserveVisualState = false) => {
    widthFlipRef.current?.kill();
    toolbarMotionRef.current?.kill();
    widthFlipRef.current = null;
    toolbarMotionRef.current = null;
    const island = islandRef.current;
    const toolbarReveal = toolbarRevealRef.current;
    if (island && !preserveVisualState) {
      Flip.killFlipsOf(island);
      gsap.set(island, { clearProps: "width,height,transform,position,top,left,willChange" });
    }
    if (toolbarReveal && !preserveVisualState) {
      gsap.set(toolbarReveal, {
        clearProps: "opacity,visibility,transform,height,gridTemplateRows,overflow",
      });
    }
    pendingWindowShrinkRef.current = false;
    widthMotionActiveRef.current = false;
    setWidthMotionActive(false);
  }, []);

  const finishVisibilityMotion = useCallback(() => {
    visibilityMotionActiveRef.current = false;
    setVisibilityMotionActive(false);
    const surface = islandSurfaceRef.current;
    const content = contentRef.current;
    if (surface) gsap.set(surface, { clearProps: "willChange" });
    if (content) gsap.set(content, { clearProps: "willChange" });
    if (
      islandVisibleRef.current
      && latestHoverRef.current
      && !previewActiveRef.current
    ) {
      requestAnimationFrame(() => applyHoverStateRef.current(true));
    }
  }, []);

  const { contextSafe } = useGSAP(() => {
    const surface = islandSurfaceRef.current;
    const content = contentRef.current;
    if (!surface || !content) return;
    const media = gsap.matchMedia();

    media.add({
      reduceMotion: "(prefers-reduced-motion: reduce)",
      allowMotion: "(prefers-reduced-motion: no-preference)",
    }, (context) => {
      const reduceMotion = Boolean(context.conditions?.reduceMotion);
      reducedMotionRef.current = reduceMotion;
      gsap.set(surface, {
        clipPath: "inset(0 46% round 999px)",
        scaleY: 0.04,
        transformOrigin: "50% 0%",
      });
      gsap.set(content, { autoAlpha: 0, y: -4 });

      if (reduceMotion) {
        presenceTimelineRef.current = null;
        if (islandVisibleRef.current) {
          gsap.set(surface, { clipPath: "inset(0 0% round 0px)", scaleY: 1 });
          gsap.set(content, { autoAlpha: 1, y: 0 });
        }
        finishVisibilityMotion();
        return;
      }

      const timeline = gsap.timeline({
        paused: true,
        onComplete: finishVisibilityMotion,
        onReverseComplete: finishVisibilityMotion,
      });
      timeline
        .to(surface, { scaleY: 0.18, duration: 0.08, ease: "power2.out" }, 0)
        .to(surface, {
          clipPath: "inset(0 0% round 0px)",
          duration: 0.21,
          ease: "power3.out",
        }, 0.04)
        .to(surface, {
          keyframes: [
            { scaleY: 1.06, duration: 0.18, ease: "power3.out" },
            { scaleY: 0.985, duration: 0.06, ease: "power2.inOut" },
            { scaleY: 1, duration: 0.04, ease: "power2.out" },
          ],
        }, 0.07)
        .to(content, {
          autoAlpha: 1,
          y: 0,
          duration: 0.22,
          ease: "power2.out",
        }, 0.13);
      presenceTimelineRef.current = timeline;
      timeline.progress(islandVisibleRef.current ? 1 : 0).pause();

      return () => {
        timeline.kill();
        if (presenceTimelineRef.current === timeline) presenceTimelineRef.current = null;
      };
    });

    return () => media.revert();
  }, { scope: shellRef });

  const animateIslandVisibility = useMemo(() => contextSafe((visible: boolean) => {
    const surface = islandSurfaceRef.current;
    const content = contentRef.current;
    if (!surface || !content) return;
    const timeline = presenceTimelineRef.current;
    if (reducedMotionRef.current || !timeline) {
      gsap.set(surface, visible
        ? { clipPath: "inset(0 0% round 0px)", scaleY: 1 }
        : { clipPath: "inset(0 46% round 999px)", scaleY: 0.04 });
      gsap.set(content, visible ? { autoAlpha: 1, y: 0 } : { autoAlpha: 0, y: -4 });
      finishVisibilityMotion();
      return;
    }

    const atTarget = visible ? timeline.progress() >= 1 : timeline.progress() <= 0;
    if (atTarget) {
      finishVisibilityMotion();
      return;
    }
    visibilityMotionActiveRef.current = true;
    setVisibilityMotionActive(true);
    gsap.set(surface, { willChange: "transform,clip-path" });
    gsap.set(content, { willChange: "transform,opacity" });
    if (visible) timeline.play();
    else timeline.reverse();
  }), [contextSafe, finishVisibilityMotion]);

  const performWidthFlip = useMemo(() => contextSafe((nextExpanded: boolean) => {
    const island = islandRef.current;
    if (!island) {
      finishWidthMotion();
      return;
    }

    const toolbarReveal = toolbarRevealRef.current;
    let collapseHeight = 0;
    let collapseStartWidth = 0;
    if (toolbarReveal && !reducedMotionRef.current) {
      if (nextExpanded) {
        gsap.set(toolbarReveal, {
          autoAlpha: Number(gsap.getProperty(toolbarReveal, "opacity")),
          y: Number(gsap.getProperty(toolbarReveal, "y")),
        });
      } else {
        const currentIslandHeight = island.getBoundingClientRect().height;
        const fullToolbarHeight = toolbarReveal.getBoundingClientRect().height;
        const contentHeight = contentRef.current?.getBoundingClientRect().height
          ?? Math.max(0, currentIslandHeight - fullToolbarHeight);
        collapseHeight = Math.min(
          fullToolbarHeight,
          Math.max(0, currentIslandHeight - contentHeight),
        );
        if (collapseHeight > 0) {
          gsap.set(toolbarReveal, {
            height: collapseHeight,
            gridTemplateRows: "1fr",
            overflow: "hidden",
          });
        }
      }
    }

    if (nextExpanded) {
      const state = Flip.getState(island);
      gsap.set(island, {
        clearProps: "width,height,transform,position,top,left",
      });
      if (toolbarReveal) {
        gsap.set(toolbarReveal, { clearProps: "height,gridTemplateRows,overflow" });
      }
      flushSync(() => {
        fitWindow(true);
        setExpandedState(true);
      });
      if (reducedMotionRef.current) {
        finishWidthMotion();
        return;
      }

      gsap.set(island, { willChange: "transform" });
      widthFlipRef.current = Flip.from(state, {
        duration: 0.22,
        ease: "power3.out",
        scale: false,
        simple: true,
        onComplete: finishWidthMotion,
      });
      if (toolbarReveal) {
        toolbarMotionRef.current = gsap.to(toolbarReveal, {
          autoAlpha: 1,
          y: 0,
          duration: 0.16,
          ease: "power2.out",
        });
      }
      return;
    }

    collapseStartWidth = island.getBoundingClientRect().width;
    gsap.set(island, {
      clearProps: "width,height,transform,position,top,left",
    });
    flushSync(() => setExpandedState(false));
    if (reducedMotionRef.current) {
      finishWidthMotion();
      return;
    }

    const collapseTargetWidth = island.getBoundingClientRect().width;
    gsap.set(island, { width: collapseStartWidth, willChange: "width" });
    widthFlipRef.current = gsap.to(island, {
      width: collapseTargetWidth,
      duration: 0.22,
      ease: "power2.inOut",
      onComplete: collapseHeight <= 0 ? finishWidthMotion : undefined,
    });
    if (toolbarReveal && collapseHeight > 0) {
      toolbarMotionRef.current = gsap.to(toolbarReveal, {
        height: 0,
        duration: 0.22,
        ease: "power2.inOut",
        onComplete: finishWidthMotion,
      });
    }
  }), [contextSafe, finishWidthMotion, fitWindow, setExpandedState]);

  const animateExpanded = useMemo(() => contextSafe((nextExpanded: boolean) => {
    cancelWidthMotion(true);
    pendingWindowShrinkRef.current = !nextExpanded;
    widthMotionActiveRef.current = true;
    setWidthMotionActive(true);

    const toolbarReveal = toolbarRevealRef.current;
    if (!nextExpanded && !reducedMotionRef.current && toolbarReveal) {
      toolbarMotionRef.current = gsap.timeline({
        onComplete: () => performWidthFlip(false),
      })
        .to(toolbarReveal, {
          autoAlpha: 0,
          y: -2,
          duration: 0.16,
          ease: "sine.inOut",
        }, 0);
      return;
    }

    performWidthFlip(nextExpanded);
  }), [cancelWidthMotion, contextSafe, performWidthFlip]);

  const expandIsland = useCallback(() => {
    if (!islandVisibleRef.current || visibilityMotionActiveRef.current) return;
    clearCollapseTimers();
    if (expandedRef.current && !widthMotionActiveRef.current) return;
    animateExpanded(true);
  }, [animateExpanded, clearCollapseTimers]);

  const scheduleCollapse = useCallback(() => {
    clearCollapseTimers();
    collapseTimerRef.current = setTimeout(() => {
      collapseTimerRef.current = null;
      if (!expandedRef.current) return;
      animateExpanded(false);
    }, HOVER_COLLAPSE_DELAY_MS);
  }, [animateExpanded, clearCollapseTimers]);

  const applyHoverState = useCallback((hovered: boolean) => {
    if (hovered) expandIsland();
    else scheduleCollapse();
  }, [expandIsland, scheduleCollapse]);
  applyHoverStateRef.current = applyHoverState;

  const handlePointerHover = useCallback((hovered: boolean) => {
    latestHoverRef.current = hovered;
    if (
      islandVisibleRef.current
      && !visibilityMotionActiveRef.current
      && !previewActiveRef.current
    ) applyHoverState(hovered);
  }, [applyHoverState]);

  const applyIslandVisibility = useCallback((visible: boolean) => {
    visibilityMotionActiveRef.current = !reducedMotionRef.current;
    setVisibilityMotionActive(!reducedMotionRef.current);
    islandVisibleRef.current = visible;
    setIslandVisible(visible);
    if (!visible) {
      clearCollapseTimers();
      if (expandedRef.current || widthMotionActiveRef.current) {
        animateExpanded(false);
      }
    }
    animateIslandVisibility(visible);
  }, [animateExpanded, animateIslandVisibility, clearCollapseTimers]);

  const renderSlot = (slot: NotchSlotContent, side: "left" | "right") => {
    const align = side === "left" ? "left" : "right";
    if (slot === "empty") return null;
    if (slot === "artwork") {
      return <img className={styles.slotArtwork} alt="" draggable={false} src={playback.artworkUrl ?? appIconUrl} />;
    }
    if (slot === "spectrum") return <SpectrumBars bands={spectrum.bands} />;
    const value = slot === "title" ? playback.snapshot.title ?? "Lyrics Plus" : playback.snapshot.artist ?? "";
    return (
      <OverflowText
        align={align}
        contentKey={`${playback.snapshot.trackId ?? "fallback"}:${slot}:${value}`}
        paused={marqueePaused}
      >
        {value}
      </OverflowText>
    );
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
      listen<NotchWidthPreviewPayload>(NOTCH_WIDTH_PREVIEW_EVENT, ({ payload }) => {
        if (payload.phase === "update") {
          const startingPreview = !previewActiveRef.current;
          previewActiveRef.current = true;
          previewTargetRef.current = payload.target;
          previewWidthRef.current = payload.width;
          setPreviewActive(true);
          setPreviewTarget(payload.target);
          setPreviewWidth(payload.width);
          if (startingPreview) {
            clearCollapseTimers();
            cancelWidthMotion();
          }
          fitWindow(
            expandedRef.current,
            payload.target === "collapsed" ? payload.width : undefined,
          );
          return;
        }

        previewActiveRef.current = false;
        setPreviewActive(false);
        if (payload.phase === "commit") {
          previewTargetRef.current = payload.target;
          previewWidthRef.current = payload.width;
          setPreviewTarget(payload.target);
          setPreviewWidth(payload.width);
          fitWindow(
            expandedRef.current,
            payload.target === "collapsed" ? payload.width : undefined,
          );
        } else {
          previewTargetRef.current = null;
          previewWidthRef.current = null;
          setPreviewTarget(null);
          setPreviewWidth(null);
          fitWindow(expandedRef.current);
        }
        applyHoverState(latestHoverRef.current);
      }),
    );
  }, [applyHoverState, cancelWidthMotion, clearCollapseTimers, fitWindow]);

  useLayoutEffect(() => {
    const content = contentRef.current;
    const player = toolbarRevealRef.current;
    if (!content || !player) return;
    let frame = 0;
    const measure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const contentHeight = Math.ceil(content.scrollHeight);
        const playerWidth = Math.ceil(player.scrollWidth);
        const previous = lastObservedGeometryRef.current;
        if (previous.contentHeight === contentHeight && previous.playerWidth === playerWidth) return;
        lastObservedGeometryRef.current = { contentHeight, playerWidth };
        if (previewActiveRef.current) {
          const collapsedWidth = previewTargetRef.current === "collapsed"
            ? previewWidthRef.current ?? notchRef.current.appearance.maxWidth
            : notchRef.current.appearance.maxWidth;
          const expandedMaxWidth = Math.min(
            NOTCH_MAX_WIDTH,
            Math.max(
              collapsedWidth,
              previewTargetRef.current === "expanded"
                ? previewWidthRef.current ?? notchRef.current.appearance.expandedMaxWidth
                : notchRef.current.appearance.expandedMaxWidth,
            ),
          );
          setExpandedWidth(Math.min(
            expandedMaxWidth,
            Math.max(collapsedWidth, playerWidth + TOOLBAR_HORIZONTAL_PADDING),
          ));
          return;
        }
        fitWindow(expandedRef.current);
      });
    };
    const observer = new ResizeObserver(measure);
    observer.observe(content);
    observer.observe(player);
    measure();
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [fitWindow]);

  useEffect(() => () => {
    clearCollapseTimers();
  }, [clearCollapseTimers]);

  return (
    <main
      className={styles.shell}
      data-expanded={expanded || undefined}
      data-has-notch={layout.hasNotch || undefined}
      data-island-visible={islandVisible || undefined}
      data-width-motion={widthMotionActive || undefined}
      data-width-preview={previewActive || undefined}
      ref={shellRef}
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
        "--notch-expanded-width": `${Math.min(effectiveExpandedMaxWidth, Math.max(effectiveWidth, expandedWidth || effectiveWidth))}px`,
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
        ref={islandRef}
      >
        <div className={styles.islandSurface} ref={islandSurfaceRef}>
          <div className={styles.content} ref={contentRef}>
            <header className={styles.metadata}>
              <div className={styles.slot} data-side="left" data-slot={notch.leftSlot}>{renderSlot(notch.leftSlot, "left")}</div>
              <span className={styles.notchGap} aria-hidden="true" />
              <div className={styles.slot} data-side="right" data-slot={notch.rightSlot}>{renderSlot(notch.rightSlot, "right")}</div>
              {layout.hasNotch && (
                <div aria-hidden="true" className={styles.brandCapsule}>
                  <img alt="" draggable={false} src={appIconUrl} />
                  <span>Lyrics Plus</span>
                </div>
              )}
            </header>
            {notch.showLyrics && primaryLine && (
              <div className={styles.currentLine} key={`${primaryLine.startMs}:${primaryLine.text}`}>
                <OverflowText
                  align="center"
                  behavior="once"
                  contentKey={`${primaryLine.startMs}:${primaryLine.text}`}
                  maxDurationMs={lyricMarqueeTimeLimitMs}
                  paused={marqueePaused}
                >
                  <KaraokeLine line={primaryLine} positionMs={lyrics.adjustedPositionMs} karaokeStyle={appearance.karaokeStyle} />
                </OverflowText>
              </div>
            )}
            {notch.showLyrics && supportingLines.map(({ kind, line }) => (
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
          <div className={styles.toolbarReveal} ref={toolbarRevealRef}>
            <div className={styles.toolbarRevealInner}>
              <ExpandedPlayer playback={playback} t={t} />
            </div>
          </div>
        </div>
      </section>
    </main>
  );
}
