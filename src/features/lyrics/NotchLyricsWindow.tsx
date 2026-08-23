import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";
import { flushSync } from "react-dom";
import type { TFunction } from "i18next";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useGSAP } from "@gsap/react";
import { gsap } from "gsap";
import { CustomEase } from "gsap/CustomEase";
import {
  Captions,
  ClockArrowLeft,
  ClockArrowRight,
  PanelTop,
  PanelsTopBottom,
  Pause,
  Play,
  Settings,
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
import { useArtworkAccentColor } from "../player/useArtworkAccentColor";
import { usePlayback } from "../player/usePlayback";
import { usePlaybackSpectrum } from "../player/usePlaybackSpectrum";
import {
  createTauriListenerCleanup,
  NOTCH_POINTER_SAMPLE_EVENT,
  NOTCH_VISIBILITY_TRANSITION_EVENT,
  NOTCH_WIDTH_PREVIEW_EVENT,
  type NotchPointerSamplePayload,
  type NotchVisibilityTransitionPayload,
  type NotchWidthPreviewPayload,
} from "../../shared/tauriEvent";
import { useLyricsPresentation } from "./useLyricsPresentation";
import type {
  CompactKaraokeStyle,
  LyricsLine,
  NotchLayoutMetrics,
  NotchLyricsPreferences,
  NotchSlotContent,
  PlaybackSpectrumBands,
} from "../../shared/types";
import styles from "./NotchLyricsWindow.module.scss";

gsap.registerPlugin(useGSAP, CustomEase);

const LOOP_MARQUEE_SPEED_PX_PER_SECOND = 28;
const LOOP_MARQUEE_START_PAUSE_MS = 1_000;
const LOOP_MARQUEE_END_PAUSE_MS = 900;
const LOOP_MARQUEE_HOME_PAUSE_MS = 900;
const LYRIC_MARQUEE_SPEED_PX_PER_SECOND = 35;
const DEFAULT_LYRIC_MARQUEE_DURATION_MS = 4_000;
const MIN_LYRIC_MARQUEE_DURATION_MS = 100;
const MORPH_DURATION_SECONDS = 0.26;
// 仅给真实 Visual Island 边缘留出极小容差，不把透明窗口算进 hover。
const HOVER_PADDING = 4;
const NOTCH_MAX_WIDTH = 640;
const WINDOW_HORIZONTAL_PADDING = 16;
const NO_NOTCH_TOP_GAP = 6;
const COLLAPSED_HEIGHT_FALLBACK = 30;
const EXPANDED_HEIGHT_FALLBACK = 180;
const SEEK_SYNC_TOLERANCE_MS = 1_500;
const SEEK_SYNC_TIMEOUT_MS = 5_000;

const EXPAND_MORPH_EASE = CustomEase.create("notch-expand-morph", "0.16,1,0.3,1");
const COLLAPSE_MORPH_EASE = CustomEase.create("notch-collapse-morph", "0.4,0,0.2,1");

function notchSlotPadding(borderRadius: number) {
  const radius = Number.isFinite(borderRadius)
    ? Math.min(40, Math.max(0, borderRadius))
    : 0;
  return 8 + radius * 0.3;
}

const emptyLayout: NotchLayoutMetrics = {
  hasNotch: false,
  topInset: 0,
  centerGapWidth: 0,
};

type NotchWidthPreviewValues = {
  maxWidth: number;
  expandedMaxWidth: number;
};

type NotchWindowFitRequest = {
  key: string;
  ready: Promise<boolean>;
  cancel: () => void;
};

type IslandState = "collapsed" | "expanding" | "expanded" | "collapsing";

type IslandDimensions = {
  collapsedWidth: number;
  collapsedHeight: number;
  expandedWidth: number;
  expandedHeight: number;
};

function physicalSizeMatches(
  actual: { width: number; height: number },
  expected: { physicalWidth: number; physicalHeight: number },
) {
  return Math.abs(actual.width - expected.physicalWidth) <= 1
    && Math.abs(actual.height - expected.physicalHeight) <= 1;
}

function waitForWebviewLayout() {
  return new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
}

function islandRadii(hasNotch: boolean, borderRadius: number, expanded: boolean) {
  const radius = `${borderRadius + (hasNotch && expanded ? 4 : 0)}px`;
  return hasNotch
    ? {
      borderTopLeftRadius: "0px",
      borderTopRightRadius: "0px",
      borderBottomRightRadius: radius,
      borderBottomLeftRadius: radius,
    }
    : { borderRadius: `${borderRadius}px` };
}

function formatPlaybackTime(valueMs: number | null) {
  const totalSeconds = Math.max(0, Math.floor((valueMs ?? 0) / 1_000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}

function formatLyricsOffset(offsetMs: number) {
  if (offsetMs === 0) return "0ms";
  return `${offsetMs > 0 ? "+" : "−"}${Math.abs(offsetMs)}ms`;
}

function formatCompactLyricsOffset(offsetMs: number) {
  if (offsetMs === 0) return "0s";
  const seconds = (Math.abs(offsetMs) / 1_000).toFixed(3).replace(/\.?0+$/, "");
  return `${offsetMs > 0 ? "+" : "−"}${seconds}s`;
}

function SpectrumBars({ bands }: { bands: PlaybackSpectrumBands }) {
  return (
    <span className={styles.spectrum} aria-hidden="true">
      {bands.slice(0, 6).map((band, index) => (
        <span className={styles.spectrumBar} key={index} style={{ "--spectrum-level": `${Math.max(12, Math.round(band * 100))}%` } as CSSProperties} />
      ))}
    </span>
  );
}

type PlaybackController = ReturnType<typeof usePlayback>;

type NotchLyricsQuickControlsProps = {
  notch: NotchLyricsPreferences;
  offsetAvailable: boolean;
  offsetMs: number;
  romanizationAvailable: boolean;
  translationAvailable: boolean;
  onChangeOffset: (deltaMs: number) => void;
  onOpenSettings: () => void;
  onPatchNotch: (patch: Partial<NotchLyricsPreferences>) => void;
  onResetOffset: () => void;
  t: TFunction;
};

function NotchLyricsQuickControls({
  notch,
  offsetAvailable,
  offsetMs,
  romanizationAvailable,
  translationAvailable,
  onChangeOffset,
  onOpenSettings,
  onPatchNotch,
  onResetOffset,
  t,
}: NotchLyricsQuickControlsProps) {
  const translation = t("common.feature.translation");
  const romanization = t("common.feature.romanization");
  const translationAction = notch.showTranslation
    ? t("overlay.toolbar.hideTrack", { track: translation })
    : t("overlay.toolbar.showTrack", { track: translation });
  const romanizationAction = notch.showRomanization
    ? t("overlay.toolbar.hideTrack", { track: romanization })
    : t("overlay.toolbar.showTrack", { track: romanization });
  const translationLabel = translationAvailable
    ? translationAction
    : t("notchLyrics.toolbar.unavailableTrack", { action: translationAction, track: translation });
  const romanizationLabel = romanizationAvailable
    ? romanizationAction
    : t("notchLyrics.toolbar.unavailableTrack", { action: romanizationAction, track: romanization });
  const layoutValue = t(`overlay.layout.${notch.layout}`);
  const offsetDisplayLabel = offsetAvailable ? formatCompactLyricsOffset(offsetMs) : "—";
  const offsetAriaLabel = offsetAvailable ? formatLyricsOffset(offsetMs) : "—";
  const offsetValueLabel = !offsetAvailable
    ? t("overlay.toolbar.noOffset")
    : offsetMs === 0
      ? t("overlay.toolbar.zeroOffset")
      : t("overlay.toolbar.offsetReset", { value: formatLyricsOffset(offsetMs) });
  const offsetValueTooltip = !offsetAvailable
    ? t("notchLyrics.toolbar.unavailableOffset")
    : offsetMs === 0
      ? t("overlay.toolbar.offsetZeroTitle")
      : t("overlay.toolbar.offsetTitle", { value: formatLyricsOffset(offsetMs) });

  return (
    <div className={styles.lyricsQuickControls} role="group" aria-label={t("notchLyrics.toolbar.label")}>
      {!notch.showLyrics ? (
        <div className={styles.lyricsQuickControlsOff}>
          <IconButton
            className={styles.quickToggle}
            label={t("notchLyrics.toolbar.showLyrics")}
            variant="ghost"
            size="icon-sm"
            aria-pressed={false}
            onClick={() => onPatchNotch({ showLyrics: true })}
          ><Captions aria-hidden="true" /></IconButton>
          <IconButton
            label={t("notchLyrics.toolbar.openSettings")}
            variant="ghost"
            size="icon-sm"
            onClick={onOpenSettings}
          ><Settings aria-hidden="true" /></IconButton>
        </div>
      ) : (
        <>
          <div className={styles.lyricsQuickControlRow}>
            <IconButton
              className={styles.quickToggle}
              label={t("notchLyrics.toolbar.hideLyrics")}
              variant="ghost"
              size="icon-sm"
              aria-pressed
              data-on="true"
              onClick={() => onPatchNotch({ showLyrics: false })}
            ><Captions aria-hidden="true" /></IconButton>
            <IconButton
              className={styles.quickToggle}
              label={t("overlay.toolbar.toggleLayout", { value: layoutValue })}
              tooltip={t("overlay.toolbar.toggleLayoutTitle", { value: layoutValue })}
              variant="ghost"
              size="icon-sm"
              aria-pressed={notch.layout === "double"}
              data-on={notch.layout === "double"}
              onClick={() => onPatchNotch({ layout: notch.layout === "double" ? "single" : "double" })}
            >{notch.layout === "double" ? <PanelsTopBottom aria-hidden="true" /> : <PanelTop aria-hidden="true" />}</IconButton>
            <IconButton
              className={styles.trackToggle}
              label={translationLabel}
              tooltip={translationLabel}
              variant="ghost"
              size="icon-sm"
              aria-pressed={notch.showTranslation}
              data-available={translationAvailable}
              data-on={notch.showTranslation}
              onClick={() => onPatchNotch({ showTranslation: !notch.showTranslation })}
            >{t("overlay.toolbar.translationGlyph")}</IconButton>
            <IconButton
              className={styles.trackToggle}
              label={romanizationLabel}
              tooltip={romanizationLabel}
              variant="ghost"
              size="icon-sm"
              aria-pressed={notch.showRomanization}
              data-available={romanizationAvailable}
              data-on={notch.showRomanization}
              onClick={() => onPatchNotch({ showRomanization: !notch.showRomanization })}
            >{t("overlay.toolbar.romanizationGlyph")}</IconButton>
          </div>
          <div className={styles.lyricsQuickControlRow}>
            <div
              className={styles.offsetControl}
              role="group"
              aria-label={t("overlay.toolbar.offsetGroup", { value: offsetAriaLabel })}
            >
              <IconButton
                label={t("overlay.toolbar.delay")}
                tooltip={t("overlay.toolbar.delayTitle")}
                variant="ghost"
                size="icon-sm"
                disabled={!offsetAvailable}
                onClick={(event) => onChangeOffset(event.shiftKey ? -500 : -100)}
              ><ClockArrowLeft aria-hidden="true" /></IconButton>
              <IconButton
                className={styles.offsetValue}
                label={offsetValueLabel}
                tooltip={offsetValueTooltip}
                variant="ghost"
                size="icon-sm"
                disabled={!offsetAvailable || offsetMs === 0}
                onClick={onResetOffset}
              >{offsetDisplayLabel}</IconButton>
              <IconButton
                label={t("overlay.toolbar.advance")}
                tooltip={t("overlay.toolbar.advanceTitle")}
                variant="ghost"
                size="icon-sm"
                disabled={!offsetAvailable}
                onClick={(event) => onChangeOffset(event.shiftKey ? 500 : 100)}
              ><ClockArrowRight aria-hidden="true" /></IconButton>
            </div>
            <IconButton
              label={t("notchLyrics.toolbar.openSettings")}
              variant="ghost"
              size="icon-sm"
              onClick={onOpenSettings}
            ><Settings aria-hidden="true" /></IconButton>
          </div>
        </>
      )}
    </div>
  );
}

function ExpandedPlayer({
  playback,
  quickControls,
  marqueePaused,
  t,
}: {
  playback: PlaybackController;
  quickControls: ReactNode;
  marqueePaused: boolean;
  t: TFunction;
}) {
  const trackKey = playback.snapshot.trackId ?? "fallback";
  const title = playback.snapshot.title ?? "Lyrics Plus";
  const artist = playback.snapshot.artist ?? "";
  const durationMs = playback.snapshot.durationMs ?? 0;
  const canSeek = durationMs > 0 && Boolean(playback.snapshot.player);
  const [draftPositionMs, setDraftPositionMs] = useState<number | null>(null);
  const [pendingSeek, setPendingSeek] = useState<{ trackKey: string; positionMs: number } | null>(null);
  const pendingPositionMs = pendingSeek?.trackKey === trackKey ? pendingSeek.positionMs : null;
  const positionMs = durationMs > 0
    ? Math.min(durationMs, Math.max(0, draftPositionMs ?? pendingPositionMs ?? playback.positionMs))
    : 0;

  useEffect(() => {
    setDraftPositionMs(null);
    setPendingSeek(null);
  }, [trackKey]);

  useEffect(() => {
    if (
      pendingPositionMs === null
      || playback.isControlling
      || Math.abs(playback.positionMs - pendingPositionMs) > SEEK_SYNC_TOLERANCE_MS
    ) {
      return;
    }
    setPendingSeek((current) => (
      current?.trackKey === trackKey && current.positionMs === pendingPositionMs
        ? null
        : current
    ));
  }, [pendingPositionMs, playback.isControlling, playback.positionMs, trackKey]);

  useEffect(() => {
    if (pendingPositionMs === null) return;
    const timeout = setTimeout(() => {
      setPendingSeek((current) => (
        current?.trackKey === trackKey && current.positionMs === pendingPositionMs
          ? null
          : current
      ));
    }, SEEK_SYNC_TIMEOUT_MS);
    return () => clearTimeout(timeout);
  }, [pendingPositionMs, trackKey]);

  const commitPosition = (value: number) => {
    const nextPosition = Math.min(durationMs, Math.max(0, Math.round(value)));
    setDraftPositionMs(null);
    setPendingSeek({ trackKey, positionMs: nextPosition });
    void playback.seekTo(nextPosition)
      .catch(() => {
        setPendingSeek((current) => (
          current?.trackKey === trackKey && current.positionMs === nextPosition
            ? null
            : current
        ));
      });
  };

  return (
    <div className={styles.player}>
      <div className={styles.playerTop}>
        <div className={styles.playerMetadata}>
          <div className={styles.playerArtwork}>
            <img alt="" draggable={false} src={playback.artworkUrl ?? appIconUrl} />
          </div>
          <div className={styles.playerTrack}>
            <strong className={styles.playerTitle} title={playback.snapshot.title ?? undefined}>
              <OverflowText contentKey={`${trackKey}:title:${title}`} paused={marqueePaused}>{title}</OverflowText>
            </strong>
            <span className={styles.playerArtist} title={playback.snapshot.artist ?? undefined}>
              <OverflowText contentKey={`${trackKey}:artist:${artist}`} paused={marqueePaused}>{artist}</OverflowText>
            </span>
          </div>
        </div>
        {quickControls}
      </div>
      <div className={styles.playerProgress}>
        <span className={styles.playerTime}>{formatPlaybackTime(positionMs)}</span>
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
        <span className={styles.playerTime}>−{formatPlaybackTime(Math.max(0, durationMs - positionMs))}</span>
      </div>
      <div className={styles.playerControls} role="group" aria-label={t("notchLyrics.player.label")}>
        <IconButton className={styles.playerControl} label={t("notchLyrics.player.previous")} variant="ghost" size="icon" onClick={() => void playback.previousTrack().catch(() => undefined)}><SkipBack fill="currentColor" strokeWidth={1.75} /></IconButton>
        <IconButton className={styles.playerPrimaryControl} label={playback.snapshot.isPlaying ? t("notchLyrics.player.pause") : t("notchLyrics.player.play")} variant="ghost" size="icon" onClick={() => void playback.togglePlayPause().catch(() => undefined)}>{playback.snapshot.isPlaying ? <Pause fill="currentColor" strokeWidth={1.5} /> : <Play fill="currentColor" strokeWidth={1.5} />}</IconButton>
        <IconButton className={styles.playerControl} label={t("notchLyrics.player.next")} variant="ghost" size="icon" onClick={() => void playback.nextTrack().catch(() => undefined)}><SkipForward fill="currentColor" strokeWidth={1.75} /></IconButton>
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
  const { config, setLyricsDisplayPreferences } = useAppConfig();
  const playback = usePlayback();
  const lyrics = useLyricsPresentation(playback.snapshot, playback.positionMs);
  const notch = config.lyrics.displays.notch;
  const appearance = notch.appearance;
  const notchRef = useRef(notch);
  notchRef.current = notch;
  const [layout, setLayout] = useState(emptyLayout);
  const [islandState, setIslandState] = useState<IslandState>("collapsed");
  const [expandedWidth, setExpandedWidth] = useState(
    () => Math.min(NOTCH_MAX_WIDTH, Math.max(appearance.maxWidth, appearance.expandedMaxWidth)),
  );
  const [collapsedHeight, setCollapsedHeight] = useState(COLLAPSED_HEIGHT_FALLBACK);
  const [expandedHeight, setExpandedHeight] = useState(EXPANDED_HEIGHT_FALLBACK);
  const [previewValues, setPreviewValues] = useState<NotchWidthPreviewValues | null>(null);
  const [previewActive, setPreviewActive] = useState(false);
  const [widthMotionActive, setWidthMotionActive] = useState(false);
  const [islandVisible, setIslandVisible] = useState(() => !isTauriRuntime());
  const [visibilityMotionActive, setVisibilityMotionActive] = useState(isTauriRuntime);
  const shellRef = useRef<HTMLElement>(null);
  const islandRef = useRef<HTMLElement>(null);
  const islandSurfaceRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const toolbarRevealRef = useRef<HTMLDivElement>(null);
  const presenceTimelineRef = useRef<gsap.core.Timeline | null>(null);
  const islandMorphRef = useRef<gsap.core.Timeline | null>(null);
  const islandStateRef = useRef<IslandState>("collapsed");
  const hostFitReadyRef = useRef(!isTauriRuntime());
  const pendingHoverApplyRef = useRef(false);
  const pendingVisibilityRef = useRef<boolean | null>(null);
  const flushHostReadyRef = useRef<() => void>(() => undefined);
  const islandVisibleRef = useRef(islandVisible);
  const visibilityMotionActiveRef = useRef(visibilityMotionActive);
  const widthMotionActiveRef = useRef(false);
  const reducedMotionRef = useRef(false);
  const previewActiveRef = useRef(false);
  const previewValuesRef = useRef<NotchWidthPreviewValues | null>(null);
  const dimensionsRef = useRef<IslandDimensions>({
    collapsedWidth: appearance.maxWidth,
    collapsedHeight: COLLAPSED_HEIGHT_FALLBACK,
    expandedWidth: Math.min(NOTCH_MAX_WIDTH, Math.max(appearance.maxWidth, appearance.expandedMaxWidth)),
    expandedHeight: EXPANDED_HEIGHT_FALLBACK,
  });
  const pendingDimensionsRef = useRef<IslandDimensions | null>(null);
  const pointerInsideRef = useRef(false);
  const pendingExpandRef = useRef(false);
  const lastFitRequestRef = useRef<NotchWindowFitRequest | null>(null);
  const lastObservedGeometryRef = useRef({ collapsedHeight: -1, expandedHeight: -1 });
  const [offsetPreview, setOffsetPreview] = useState<{ trackKey: string; offsetMs: number } | null>(null);
  const pendingOffsetRef = useRef(0);
  const offsetWriteQueue = useRef<Promise<void>>(Promise.resolve());
  const offsetWriteVersionRef = useRef(0);
  const reconcileHoverStateRef = useRef<() => void>(() => undefined);
  const usesSpectrum = notch.leftSlot === "spectrum" || notch.rightSlot === "spectrum";
  const spectrumColor = useArtworkAccentColor(playback.snapshot.artworkId, playback.artworkUrl);
  const spectrum = usePlaybackSpectrum(usesSpectrum);
  const effectiveWidth = previewValues?.maxWidth ?? appearance.maxWidth;
  const effectiveExpandedMaxWidth = Math.min(
    NOTCH_MAX_WIDTH,
    Math.max(
      effectiveWidth,
      previewValues?.expandedMaxWidth ?? appearance.expandedMaxWidth,
    ),
  );
  const slotPadding = notchSlotPadding(appearance.borderRadius);
  const marqueePaused = previewActive || widthMotionActive || visibilityMotionActive;
  const runtimeOffsetMs = lyrics.document?.offsetMs ?? 0;
  const runtimeOffsetRef = useRef({ trackKey: lyrics.trackKey, offsetMs: runtimeOffsetMs });
  runtimeOffsetRef.current = { trackKey: lyrics.trackKey, offsetMs: runtimeOffsetMs };
  const offsetMs = offsetPreview?.trackKey === lyrics.trackKey
    ? offsetPreview.offsetMs
    : runtimeOffsetMs;
  const offsetAvailable = Boolean(lyrics.document && lyrics.trackKey);
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

  useEffect(() => {
    offsetWriteVersionRef.current += 1;
    pendingOffsetRef.current = runtimeOffsetMs;
    setOffsetPreview(null);
  }, [lyrics.trackKey]);

  useEffect(() => {
    pendingOffsetRef.current = runtimeOffsetMs;
    if (offsetPreview?.trackKey !== lyrics.trackKey) return;
    if (offsetPreview.offsetMs === runtimeOffsetMs) setOffsetPreview(null);
  }, [lyrics.trackKey, offsetPreview, runtimeOffsetMs]);

  const patchNotch = useCallback((patch: Partial<NotchLyricsPreferences>) => {
    const next = { ...notchRef.current, ...patch };
    notchRef.current = next;
    void setLyricsDisplayPreferences("notch", next).catch((error) => {
      reportFrontendError("Failed to update Dynamic Island lyrics preferences", error);
    });
  }, [setLyricsDisplayPreferences]);

  const setLyricsOffset = useCallback((nextOffsetMs: number) => {
    const trackKey = lyrics.trackKey;
    if (!trackKey || !lyrics.document) return;
    const version = offsetWriteVersionRef.current + 1;
    offsetWriteVersionRef.current = version;
    const nextOffset = Math.trunc(nextOffsetMs);
    pendingOffsetRef.current = nextOffset;
    setOffsetPreview({ trackKey, offsetMs: nextOffset });
    const operation = offsetWriteQueue.current
      .then(() => api.setLyricsOffset(trackKey, nextOffset))
      .catch((error) => {
        if (offsetWriteVersionRef.current === version) {
          setOffsetPreview(null);
          if (runtimeOffsetRef.current.trackKey === trackKey) {
            pendingOffsetRef.current = runtimeOffsetRef.current.offsetMs;
          }
        }
        reportFrontendError("Failed to update the Dynamic Island lyrics offset", error);
      });
    offsetWriteQueue.current = operation;
  }, [lyrics.document, lyrics.trackKey]);

  const changeLyricsOffset = useCallback((deltaMs: number) => {
    if (!offsetAvailable) return;
    setLyricsOffset(pendingOffsetRef.current + deltaMs);
  }, [offsetAvailable, setLyricsOffset]);

  const resetLyricsOffset = useCallback(() => {
    if (!offsetAvailable) return;
    setLyricsOffset(0);
  }, [offsetAvailable, setLyricsOffset]);

  const openLyricsSettings = useCallback(() => {
    void api.showLyricsStyleSettings("notch").catch((error) => {
      reportFrontendError("Failed to open Dynamic Island lyrics settings", error);
    });
  }, []);

  const quickControls = (
    <NotchLyricsQuickControls
      notch={notch}
      offsetAvailable={offsetAvailable}
      offsetMs={offsetMs}
      romanizationAvailable={Boolean(lyrics.document?.tracks.romanization)}
      translationAvailable={Boolean(lyrics.document?.tracks.translation)}
      onChangeOffset={changeLyricsOffset}
      onOpenSettings={openLyricsSettings}
      onPatchNotch={patchNotch}
      onResetOffset={resetLyricsOffset}
      t={t}
    />
  );
  const setIslandStateValue = useCallback((next: IslandState) => {
    if (import.meta.env.DEV && islandStateRef.current !== next) {
      console.debug("notch island state", { from: islandStateRef.current, to: next });
    }
    islandStateRef.current = next;
    setIslandState(next);
  }, []);

  const requestNativeFit = useCallback((dimensions: IslandDimensions) => {
    if (!isTauriRuntime()) {
      hostFitReadyRef.current = true;
      return;
    }
    const width = dimensions.expandedWidth + WINDOW_HORIZONTAL_PADDING;
    const height = dimensions.expandedHeight + (layout.hasNotch ? 0 : NO_NOTCH_TOP_GAP);
    const requestKey = `${width}:${height}`;
    if (lastFitRequestRef.current?.key === requestKey) return;
    lastFitRequestRef.current?.cancel();
    hostFitReadyRef.current = false;
    let cancelRequest: () => void = () => undefined;
    const ready = (async () => {
      const currentWindow = getCurrentWindow();
      let expectedSize: { physicalWidth: number; physicalHeight: number } | null = null;
      let latestResize: { width: number; height: number } | null = null;
      let cancelled = false;
      let resolveMatchedResize: () => void = () => undefined;
      const matchedResize = new Promise<void>((resolve) => {
        resolveMatchedResize = resolve;
      });
      let unlistenResize: (() => void) | null = null;
      cancelRequest = () => {
        cancelled = true;
        resolveMatchedResize();
      };

      try {
        // 先监听再请求原生窗口调整，避免漏掉 AppKit 很快发出的 resize 回执。
        unlistenResize = await currentWindow.onResized(({ payload }) => {
          latestResize = payload;
          if (expectedSize && physicalSizeMatches(payload, expectedSize)) {
            resolveMatchedResize();
          }
        });
        if (cancelled) return false;

        const result = await api.fitNotchLyricsContent(width, height);
        if (cancelled) return false;
        expectedSize = result;
        if (result.sizeChanged) {
          const currentSize = await currentWindow.outerSize();
          const resizeAlreadyMatched = latestResize
            ? physicalSizeMatches(latestResize, result)
            : false;
          if (!resizeAlreadyMatched && !physicalSizeMatches(currentSize, result)) {
            await matchedResize;
          }
        }
        if (cancelled) return false;
        await waitForWebviewLayout();
        return !cancelled;
      } catch (error) {
        reportFrontendError("Failed to fit the Dynamic Island lyrics window", error);
        return false;
      } finally {
        unlistenResize?.();
      }
    })();
    const request = { key: requestKey, ready, cancel: () => cancelRequest() };
    lastFitRequestRef.current = request;
    void ready.then(() => {
      if (lastFitRequestRef.current !== request) return;
      lastFitRequestRef.current = null;
      hostFitReadyRef.current = true;
      flushHostReadyRef.current();
      if (
        pendingHoverApplyRef.current
        && islandVisibleRef.current
        && !visibilityMotionActiveRef.current
        && !previewActiveRef.current
      ) {
        pendingHoverApplyRef.current = false;
        requestAnimationFrame(() => reconcileHoverStateRef.current());
      }
    });
  }, [layout.hasNotch]);

  const applyMeasuredDimensions = useCallback((next: IslandDimensions) => {
    dimensionsRef.current = next;
    setExpandedWidth(next.expandedWidth);
    setCollapsedHeight(next.collapsedHeight);
    setExpandedHeight(next.expandedHeight);
    requestNativeFit(next);
  }, [requestNativeFit]);

  const fitWindow = useCallback((collapsedWidthOverride?: number) => {
    const compactContent = contentRef.current;
    const expandedContent = toolbarRevealRef.current;
    if (!compactContent || !expandedContent) return;
    const preview = previewValuesRef.current;
    const collapsedWidth = collapsedWidthOverride ?? preview?.maxWidth ?? notch.appearance.maxWidth;
    const expandedMaxWidth = Math.min(
      NOTCH_MAX_WIDTH,
      Math.max(
        collapsedWidth,
        preview?.expandedMaxWidth ?? notch.appearance.expandedMaxWidth,
      ),
    );
    const nextDimensions: IslandDimensions = {
      collapsedWidth,
      collapsedHeight: Math.max(COLLAPSED_HEIGHT_FALLBACK, Math.ceil(compactContent.scrollHeight)),
      expandedWidth: expandedMaxWidth,
      expandedHeight: Math.max(EXPANDED_HEIGHT_FALLBACK, Math.ceil(expandedContent.scrollHeight)),
    };
    if (widthMotionActiveRef.current) {
      pendingDimensionsRef.current = nextDimensions;
      return;
    }
    applyMeasuredDimensions(nextDimensions);
  }, [applyMeasuredDimensions, notch.appearance.expandedMaxWidth, notch.appearance.maxWidth]);

  useLayoutEffect(() => {
    if (!previewActiveRef.current) fitWindow();
  }, [appearance.expandedMaxWidth, appearance.maxWidth, fitWindow]);

  useLayoutEffect(() => {
    if (widthMotionActiveRef.current) return;
    const island = islandRef.current;
    if (!island) return;
    const dimensions = dimensionsRef.current;
    const expanded = islandStateRef.current === "expanded";
    // 稳定态也由 GSAP 写入数值，SCSS 只提供首帧回退尺寸，不参与状态切换。
    gsap.set(island, {
      width: expanded ? dimensions.expandedWidth : dimensions.collapsedWidth,
      height: expanded ? dimensions.expandedHeight : dimensions.collapsedHeight,
      ...islandRadii(layout.hasNotch, appearance.borderRadius, expanded),
    });
  }, [appearance.borderRadius, collapsedHeight, effectiveWidth, expandedHeight, expandedWidth, islandState, layout.hasNotch, widthMotionActive]);

  useEffect(() => {
    if (
      previewActive
      || previewValues === null
      || previewValues.maxWidth !== appearance.maxWidth
      || previewValues.expandedMaxWidth !== Math.max(appearance.maxWidth, appearance.expandedMaxWidth)
    ) return;
    previewValuesRef.current = null;
    setPreviewValues(null);
  }, [appearance.expandedMaxWidth, appearance.maxWidth, previewActive, previewValues]);

  const applyPendingDimensions = useCallback(() => {
    const pending = pendingDimensionsRef.current;
    if (!pending) return;
    pendingDimensionsRef.current = null;
    applyMeasuredDimensions(pending);
  }, [applyMeasuredDimensions]);

  const finishWidthMotion = useCallback((finalExpanded: boolean) => {
    islandMorphRef.current = null;
    flushSync(() => setIslandStateValue(finalExpanded ? "expanded" : "collapsed"));
    widthMotionActiveRef.current = false;
    setWidthMotionActive(false);
    const island = islandRef.current;
    const surface = islandSurfaceRef.current;
    const content = contentRef.current;
    const toolbarReveal = toolbarRevealRef.current;
    if (island) {
      gsap.set(island, { clearProps: "willChange" });
    }
    if (surface) gsap.set(surface, { clearProps: "height,willChange" });
    if (content) {
      gsap.set(content, {
        autoAlpha: finalExpanded ? 0 : 1,
        y: finalExpanded ? -4 : 0,
        scale: 1,
        clearProps: "willChange",
      });
    }
    if (toolbarReveal) {
      gsap.set(toolbarReveal, {
        autoAlpha: finalExpanded ? 1 : 0,
        y: finalExpanded ? 0 : -4,
        scale: 1,
        clearProps: "willChange",
      });
    }
    applyPendingDimensions();
    requestAnimationFrame(() => reconcileHoverStateRef.current());
  }, [applyPendingDimensions, setIslandStateValue]);

  const cancelWidthMotion = useCallback(() => {
    islandMorphRef.current?.kill();
    islandMorphRef.current = null;
    pendingDimensionsRef.current = null;
    pendingExpandRef.current = false;
    widthMotionActiveRef.current = false;
    setWidthMotionActive(false);
    flushSync(() => setIslandStateValue("collapsed"));
    const island = islandRef.current;
    const surface = islandSurfaceRef.current;
    const content = contentRef.current;
    const toolbarReveal = toolbarRevealRef.current;
    if (island) gsap.set(island, { clearProps: "width,height,borderRadius,borderTopLeftRadius,borderTopRightRadius,borderBottomRightRadius,borderBottomLeftRadius,willChange" });
    if (surface) gsap.set(surface, { clearProps: "height,willChange" });
    if (content) gsap.set(content, { autoAlpha: 1, y: 0, scale: 1, clearProps: "willChange" });
    if (toolbarReveal) gsap.set(toolbarReveal, { autoAlpha: 0, y: -4, scale: 1, clearProps: "willChange" });
  }, [setIslandStateValue]);

  const finishVisibilityMotion = useCallback(() => {
    visibilityMotionActiveRef.current = false;
    setVisibilityMotionActive(false);
    const surface = islandSurfaceRef.current;
    const content = contentRef.current;
    if (surface) gsap.set(surface, { clearProps: "willChange" });
    if (content) gsap.set(content, { clearProps: "willChange" });
    if (islandVisibleRef.current && hostFitReadyRef.current && !previewActiveRef.current) {
      pendingHoverApplyRef.current = false;
      requestAnimationFrame(() => reconcileHoverStateRef.current());
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

  const flushHostReady = useCallback(() => {
    const pendingVisibility = pendingVisibilityRef.current;
    if (pendingVisibility === null) return;
    pendingVisibilityRef.current = null;
    animateIslandVisibility(pendingVisibility);
  }, [animateIslandVisibility]);
  flushHostReadyRef.current = flushHostReady;

  const performLiquidMorph = useMemo(() => contextSafe((nextExpanded: boolean) => {
    const island = islandRef.current;
    const surface = islandSurfaceRef.current;
    const compactContent = contentRef.current;
    const expandedContent = toolbarRevealRef.current;
    if (!island || !surface || !compactContent || !expandedContent) {
      finishWidthMotion(nextExpanded);
      return;
    }
    const dimensions = dimensionsRef.current;
    const start = nextExpanded
      ? { width: dimensions.collapsedWidth, height: dimensions.collapsedHeight }
      : { width: dimensions.expandedWidth, height: dimensions.expandedHeight };
    const target = nextExpanded
      ? { width: dimensions.expandedWidth, height: dimensions.expandedHeight }
      : { width: dimensions.collapsedWidth, height: dimensions.collapsedHeight };
    const collapsedRadii = islandRadii(layout.hasNotch, appearance.borderRadius, false);
    const expandedRadii = islandRadii(layout.hasNotch, appearance.borderRadius, true);
    const startRadii = nextExpanded ? collapsedRadii : expandedRadii;
    const targetRadii = nextExpanded ? expandedRadii : collapsedRadii;
    const complete = () => finishWidthMotion(nextExpanded);

    gsap.set(island, { ...start, ...startRadii });
    flushSync(() => setIslandStateValue(nextExpanded ? "expanding" : "collapsing"));
    gsap.set(island, { ...start, ...startRadii });

    if (reducedMotionRef.current) {
      gsap.set(island, { ...target, ...targetRadii });
      gsap.set(compactContent, nextExpanded ? { autoAlpha: 0, y: -4, scale: 1 } : { autoAlpha: 1, y: 0, scale: 1 });
      gsap.set(expandedContent, nextExpanded ? { autoAlpha: 1, y: 0, scale: 1 } : { autoAlpha: 0, y: -4, scale: 1 });
      finishWidthMotion(nextExpanded);
      return;
    }

    gsap.set(surface, {
      height: "100%",
      scaleX: 1,
      scaleY: 1,
      transformOrigin: "50% 0%",
    });
    // width、height 和圆角共用同一个 tween，确保二维几何同轨开始、同轨结束。
    gsap.set(island, { willChange: "width,height,border-radius" });

    if (nextExpanded) {
      gsap.set(compactContent, {
        autoAlpha: 1,
        y: 0,
        scale: 1,
        willChange: "transform,opacity",
      });
      gsap.set(expandedContent, {
        autoAlpha: 0,
        y: -8,
        scale: 0.96,
        transformOrigin: "50% 0%",
        willChange: "transform,opacity",
      });
    } else {
      gsap.set(expandedContent, { autoAlpha: 0, y: -4, scale: 0.98 });
      gsap.set(compactContent, {
        autoAlpha: 0,
        y: -3,
        scale: 0.97,
        willChange: "transform,opacity",
      });
    }

    const timeline = gsap.timeline({ onComplete: complete });
    timeline.to(island, {
      width: target.width,
      height: target.height,
      ...targetRadii,
      duration: MORPH_DURATION_SECONDS,
      ease: nextExpanded ? EXPAND_MORPH_EASE : COLLAPSE_MORPH_EASE,
      overwrite: "auto",
    }, 0);

    if (nextExpanded) {
      timeline
        .to(compactContent, {
          autoAlpha: 0,
          y: -3,
          scale: 0.96,
          duration: 0.07,
          ease: "power2.in",
        }, 0)
        .to(expandedContent, {
          autoAlpha: 1,
          y: 0,
          scale: 1,
          duration: 0.16,
          ease: "power3.out",
        }, 0.08);
    } else {
      timeline.to(compactContent, {
        autoAlpha: 1,
        y: 0,
        scale: 1,
        duration: 0.12,
        ease: "power2.out",
      }, 0.12);
    }

    islandMorphRef.current = timeline;
  }), [appearance.borderRadius, contextSafe, finishWidthMotion, layout.hasNotch, setIslandStateValue]);

  const startExpansion = useMemo(() => contextSafe(() => {
    if (
      islandStateRef.current !== "collapsed"
      || !islandVisibleRef.current
      || visibilityMotionActiveRef.current
      || previewActiveRef.current
    ) return;
    pendingExpandRef.current = false;
    widthMotionActiveRef.current = true;
    setWidthMotionActive(true);
    performLiquidMorph(true);
  }), [contextSafe, performLiquidMorph]);

  const startCollapse = useMemo(() => contextSafe(() => {
    if (
      islandStateRef.current !== "expanded"
      || !islandVisibleRef.current
      || visibilityMotionActiveRef.current
      || previewActiveRef.current
    ) return;
    widthMotionActiveRef.current = true;
    setWidthMotionActive(true);
    performLiquidMorph(false);
  }), [contextSafe, performLiquidMorph]);

  const reconcileHoverState = useCallback(() => {
    if (
      !hostFitReadyRef.current
      || !islandVisibleRef.current
      || visibilityMotionActiveRef.current
      || previewActiveRef.current
    ) {
      pendingHoverApplyRef.current = true;
      return;
    }
    const currentState = islandStateRef.current;
    if (currentState === "collapsed") {
      if (pointerInsideRef.current || pendingExpandRef.current) startExpansion();
      return;
    }
    if (currentState === "expanded" && !pointerInsideRef.current) {
      startCollapse();
      return;
    }
    if (currentState === "collapsing") {
      pendingExpandRef.current = pointerInsideRef.current;
    }
  }, [startCollapse, startExpansion]);
  reconcileHoverStateRef.current = reconcileHoverState;

  const processPointerState = useCallback(() => {
    if (!hostFitReadyRef.current || !islandVisibleRef.current || visibilityMotionActiveRef.current || previewActiveRef.current) {
      pendingHoverApplyRef.current = true;
      return;
    }
    const currentState = islandStateRef.current;
    if (currentState === "collapsed") {
      if (pointerInsideRef.current || pendingExpandRef.current) startExpansion();
      return;
    }
    if (currentState === "expanding") return;
    if (currentState === "expanded") {
      if (!pointerInsideRef.current) startCollapse();
      return;
    }
    pendingExpandRef.current = pointerInsideRef.current;
  }, [startCollapse, startExpansion]);

  const updateHoverFromPoint = useCallback((x: number, y: number, source: "pointerenter" | "pointermove" | "pointerleave" | "native") => {
    const rect = islandRef.current?.getBoundingClientRect();
    const isInsideIsland = rect
      ? x >= rect.left - HOVER_PADDING
        && x <= rect.right + HOVER_PADDING
        && y >= rect.top - HOVER_PADDING
        && y <= rect.bottom + HOVER_PADDING
      : false;
    const changed = pointerInsideRef.current !== isInsideIsland;
    pointerInsideRef.current = isInsideIsland;
    if (import.meta.env.DEV && (changed || source === "pointerenter" || source === "pointerleave")) {
      console.debug("notch pointer sample", {
        source,
        x,
        y,
        rect,
        isInsideIsland,
        state: islandStateRef.current,
      });
    }
    processPointerState();
  }, [processPointerState]);

  const handleIslandPointerEnter = useCallback((event: ReactPointerEvent<HTMLElement>) => {
    updateHoverFromPoint(event.clientX, event.clientY, "pointerenter");
  }, [updateHoverFromPoint]);

  const handleIslandPointerMove = useCallback((event: ReactPointerEvent<HTMLElement>) => {
    updateHoverFromPoint(event.clientX, event.clientY, "pointermove");
  }, [updateHoverFromPoint]);

  const handleIslandPointerLeave = useCallback((event: ReactPointerEvent<HTMLElement>) => {
    updateHoverFromPoint(event.clientX, event.clientY, "pointerleave");
  }, [updateHoverFromPoint]);

  const applyIslandVisibility = useCallback((visible: boolean) => {
    visibilityMotionActiveRef.current = !reducedMotionRef.current;
    setVisibilityMotionActive(!reducedMotionRef.current);
    islandVisibleRef.current = visible;
    setIslandVisible(visible);
    if (!visible) {
      pendingVisibilityRef.current = null;
      pointerInsideRef.current = false;
      pendingExpandRef.current = false;
      if (islandStateRef.current !== "collapsed" || widthMotionActiveRef.current) cancelWidthMotion();
    } else if (!hostFitReadyRef.current) {
      // 原生窗口完成最大展开尺寸适配前保持播放器隐藏，避免首帧几何不完整。
      pendingVisibilityRef.current = true;
      return;
    }
    animateIslandVisibility(visible);
  }, [animateIslandVisibility, cancelWidthMotion]);

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
      listen<NotchPointerSamplePayload>(NOTCH_POINTER_SAMPLE_EVENT, ({ payload }) => {
        updateHoverFromPoint(payload.clientX, payload.clientY, "native");
      }),
    );
  }, [updateHoverFromPoint]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    return createTauriListenerCleanup(
      listen<NotchWidthPreviewPayload>(NOTCH_WIDTH_PREVIEW_EVENT, ({ payload }) => {
        if (payload.phase === "update" || payload.phase === "commit") {
          const nextPreviewValues = {
            maxWidth: payload.maxWidth,
            expandedMaxWidth: Math.max(payload.maxWidth, payload.expandedMaxWidth),
          };
          previewValuesRef.current = nextPreviewValues;
          setPreviewValues(nextPreviewValues);
          if (payload.phase === "update") {
            const startingPreview = !previewActiveRef.current;
            previewActiveRef.current = true;
            setPreviewActive(true);
            if (startingPreview) {
              cancelWidthMotion();
            }
            fitWindow(nextPreviewValues.maxWidth);
            return;
          }

          previewActiveRef.current = false;
          setPreviewActive(false);
          fitWindow(nextPreviewValues.maxWidth);
          if (hostFitReadyRef.current) {
            pendingHoverApplyRef.current = false;
            reconcileHoverStateRef.current();
          } else pendingHoverApplyRef.current = true;
          return;
        }

        previewActiveRef.current = false;
        setPreviewActive(false);
        previewValuesRef.current = null;
        setPreviewValues(null);
        fitWindow();
        if (hostFitReadyRef.current) {
          pendingHoverApplyRef.current = false;
          reconcileHoverStateRef.current();
        } else pendingHoverApplyRef.current = true;
      }),
    );
  }, [cancelWidthMotion, fitWindow]);

  useLayoutEffect(() => {
    const content = contentRef.current;
    const player = toolbarRevealRef.current;
    if (!content || !player) return;
    let frame = 0;
    const measure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const collapsedHeight = Math.ceil(content.scrollHeight);
        const expandedHeight = Math.ceil(player.scrollHeight);
        const previous = lastObservedGeometryRef.current;
        if (previous.collapsedHeight === collapsedHeight && previous.expandedHeight === expandedHeight) return;
        lastObservedGeometryRef.current = { collapsedHeight, expandedHeight };
        fitWindow();
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
    lastFitRequestRef.current?.cancel();
    lastFitRequestRef.current = null;
  }, []);

  return (
    <main
      className={styles.shell}
      data-expanded={islandState !== "collapsed" || undefined}
      data-island-state={islandState}
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
        "--notch-spectrum-color": spectrumColor,
        "--notch-radius": `${appearance.borderRadius}px`,
        "--notch-slot-padding": `${slotPadding}px`,
        "--notch-max-width": `${effectiveWidth}px`,
        "--notch-collapsed-height": `${Math.max(COLLAPSED_HEIGHT_FALLBACK, collapsedHeight)}px`,
        "--notch-expanded-width": `${Math.min(effectiveExpandedMaxWidth, Math.max(effectiveWidth, expandedWidth))}px`,
        "--notch-expanded-height": `${Math.max(COLLAPSED_HEIGHT_FALLBACK, expandedHeight)}px`,
        "--notch-top-inset": `${layout.topInset}px`,
        "--notch-center-gap": `${layout.centerGapWidth}px`,
      } as CSSProperties}
    >
      <div className={styles.hoverArea}>
        <section
          aria-expanded={islandState === "expanded"}
          aria-live="polite"
          className={styles.island}
          onPointerEnter={handleIslandPointerEnter}
          onPointerLeave={handleIslandPointerLeave}
          onPointerMove={handleIslandPointerMove}
          ref={islandRef}
        >
          <div className={styles.islandSurface} ref={islandSurfaceRef}>
            {layout.hasNotch && (
              <div aria-hidden="true" className={styles.brandCapsule}>
                <img alt="" draggable={false} src={appIconUrl} />
                <span>Lyrics Plus</span>
              </div>
            )}
            <div className={styles.content} ref={contentRef}>
              <header className={styles.metadata}>
                <div className={styles.slot} data-side="left" data-slot={notch.leftSlot}>{renderSlot(notch.leftSlot, "left")}</div>
                <span className={styles.notchGap} aria-hidden="true" />
                <div className={styles.slot} data-side="right" data-slot={notch.rightSlot}>{renderSlot(notch.rightSlot, "right")}</div>
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
                    <KaraokeLine line={primaryLine} positionMs={playback.positionMs + offsetMs} karaokeStyle={appearance.karaokeStyle} />
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
                <ExpandedPlayer marqueePaused={marqueePaused} playback={playback} quickControls={quickControls} t={t} />
              </div>
            </div>
          </div>
        </section>
      </div>
    </main>
  );
}
