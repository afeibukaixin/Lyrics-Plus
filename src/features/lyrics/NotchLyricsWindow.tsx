import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import appIconUrl from "../../../src-tauri/icons/32x32.png";
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
import { useNotchLyricsOffset } from "./useNotchLyricsOffset";
import { useNotchIslandMotion } from "./useNotchIslandMotion";
import { useNotchWindowGeometry } from "./useNotchWindowGeometry";
import type {
  LyricsLine,
  NotchLayoutMetrics,
  NotchLyricsPreferences,
  NotchSlotContent,
  PlaybackSpectrumBands,
} from "../../shared/types";
import {
  ArtworkTransitionImage,
  ExpandedPlayer,
  KaraokeLine,
  MIN_LYRIC_MARQUEE_DURATION_MS,
  NotchLyricsQuickControls,
  OverflowText,
  SpectrumBars,
} from "./NotchLyricsComponents";
import {
  COLLAPSED_HEIGHT_FALLBACK,
  EXPANDED_HEIGHT_FALLBACK,
  emptyLayout,
  notchCollapsedHeightFloor,
  notchSlotPadding,
  NOTCH_MAX_WIDTH,
  resolvedNotchTopInset,
  type IslandDimensions,
  type IslandState,
  type NotchWindowFitRequest,
  type NotchWidthPreviewValues,
} from "./NotchLyricsLayout";
import styles from "./NotchLyricsWindow.module.scss";

function previewLineAtPosition(lines: LyricsLine[], positionMs: number) {
  let activeIndex = -1;
  for (let index = 0; index < lines.length; index += 1) {
    if (lines[index].startMs > positionMs) break;
    activeIndex = index;
  }

  const line = lines[activeIndex] ?? lines[0] ?? null;
  const nextLine = activeIndex < 0 ? lines[1] ?? null : lines[activeIndex + 1] ?? null;
  return { line, nextLine };
}

export default function NotchLyricsWindow() {
  const { t } = useTranslation();
  const { config, setLyricsDisplayPreferences } = useAppConfig();
  const playback = usePlayback({ loadArtwork: true });
  const lyrics = useLyricsPresentation(playback.snapshot, playback.positionMs, playback.active);
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
  const hoverAreaRef = useRef<HTMLDivElement>(null);
  const islandRef = useRef<HTMLElement>(null);
  const islandSurfaceRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const toolbarRevealRef = useRef<HTMLDivElement>(null);
  const islandStateRef = useRef<IslandState>("collapsed");
  const hostFitReadyRef = useRef(!isTauriRuntime());
  const pendingHoverApplyRef = useRef(false);
  const pendingVisibilityRef = useRef<boolean | null>(null);
  const flushHostReadyRef = useRef<() => void>(() => undefined);
  const islandVisibleRef = useRef(islandVisible);
  const visibilityMotionActiveRef = useRef(visibilityMotionActive);
  const widthMotionActiveRef = useRef(false);
  const previewActiveRef = useRef(false);
  const previewValuesRef = useRef<NotchWidthPreviewValues | null>(null);
  const dimensionsRef = useRef<IslandDimensions>({
    collapsedWidth: appearance.maxWidth,
    collapsedHeight: COLLAPSED_HEIGHT_FALLBACK,
    expandedWidth: Math.min(NOTCH_MAX_WIDTH, Math.max(appearance.maxWidth, appearance.expandedMaxWidth)),
    expandedHeight: EXPANDED_HEIGHT_FALLBACK,
  });
  const pendingDimensionsRef = useRef<IslandDimensions | null>(null);
  const lastFitRequestRef = useRef<NotchWindowFitRequest | null>(null);
  const lastObservedGeometryRef = useRef({ collapsedHeight: -1, expandedHeight: -1 });
  const reconcileHoverStateRef = useRef<() => void>(() => undefined);
  const usesSpectrum = notch.leftSlot === "spectrum" || notch.rightSlot === "spectrum";
  const spectrumColor = useArtworkAccentColor(
    usesSpectrum ? playback.snapshot.artworkId : null,
    usesSpectrum ? playback.artworkUrl : null,
  );
  const spectrumNodesRef = useRef(new Set<HTMLSpanElement>());
  const registerSpectrumNode = useCallback((node: HTMLSpanElement) => {
    spectrumNodesRef.current.add(node);
    return () => spectrumNodesRef.current.delete(node);
  }, []);
  const paintSpectrum = useCallback((bands: PlaybackSpectrumBands) => {
    for (const node of spectrumNodesRef.current) {
      Array.from(node.children).forEach((bar, index) => {
        const level = Math.max(0.12, bands[index] ?? 0);
        (bar as HTMLElement).style.transform = `scaleY(${level})`;
      });
    }
  }, []);
  usePlaybackSpectrum(usesSpectrum && playback.active, paintSpectrum);
  const effectiveWidth = previewValues?.maxWidth ?? appearance.maxWidth;
  const effectiveExpandedMaxWidth = Math.min(
    NOTCH_MAX_WIDTH,
    Math.max(
      effectiveWidth,
      previewValues?.expandedMaxWidth ?? appearance.expandedMaxWidth,
    ),
  );
  const resolvedTopInset = resolvedNotchTopInset(layout);
  const collapsedHeightFloor = notchCollapsedHeightFloor(layout);
  const compactSlotSize = Math.max(0, Math.min(30, resolvedTopInset - 8));
  const slotPadding = notchSlotPadding(appearance.borderRadius);
  const marqueePaused = previewActive || widthMotionActive || visibilityMotionActive;
  const runtimeOffsetMs = lyrics.document?.offsetMs ?? 0;
  const {
    changeLyricsOffset,
    offsetAvailable,
    offsetMs,
    resetLyricsOffset,
  } = useNotchLyricsOffset({
    hasDocument: Boolean(lyrics.document),
    runtimeOffsetMs,
    trackKey: lyrics.trackKey,
  });
  const originalLines = lyrics.document?.tracks.original.lines ?? [];
  const previewPositionMs = playback.positionMs + offsetMs;
  const preview = previewLineAtPosition(originalLines, previewPositionMs);
  const previewLineDisplayEndMs = preview.nextLine?.startMs ?? preview.line?.endMs;
  const previewLyricMarqueeTimeLimitMs = preview.line && previewLineDisplayEndMs != null
    ? Math.max(
      MIN_LYRIC_MARQUEE_DURATION_MS,
      previewLineDisplayEndMs - preview.line.startMs,
    )
    : null;
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
  const alternatingDoubleLine = notch.layout === "double"
    && notch.doubleLineMode === "alternating"
    && lyrics.activeIndex >= 0
    && !selectedSupportingLine;
  const supportingLines = notch.layout !== "double"
    ? []
    : selectedSupportingLine
      ? [selectedSupportingLine]
      : secondaryLine?.text.trim()
        ? [{ kind: "next" as const, line: secondaryLine }]
        : alternatingDoubleLine
          ? [{
            kind: "next" as const,
            line: {
              startMs: primaryLine?.startMs ?? -1,
              endMs: null,
              text: "",
              words: null,
            },
          }]
          : [];
  const doubleLineOrder = alternatingDoubleLine && supportingLines[0]?.kind === "next" && lyrics.activeIndex % 2 === 1
    ? "reversed"
    : "normal";
  const currentLineDisplayEndMs = lyrics.nextLine?.startMs ?? lyrics.currentLine?.endMs;
  const lyricMarqueeTimeLimitMs = lyrics.currentLine && currentLineDisplayEndMs != null
    ? Math.max(
      MIN_LYRIC_MARQUEE_DURATION_MS,
      currentLineDisplayEndMs - lyrics.currentLine.startMs,
    )
    : null;

  const patchNotch = useCallback((patch: Partial<NotchLyricsPreferences>) => {
    const next = { ...notchRef.current, ...patch };
    notchRef.current = next;
    void setLyricsDisplayPreferences("notch", next).catch((error) => {
      reportFrontendError("Failed to update Dynamic Island lyrics preferences", error);
    });
  }, [setLyricsDisplayPreferences]);

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
  const { applyPendingDimensions, fitWindow } = useNotchWindowGeometry({
    appearance,
    collapsedHeight,
    contentRef,
    dimensionsRef,
    effectiveWidth,
    expandedHeight,
    expandedWidth,
    flushHostReadyRef,
    hostFitReadyRef,
    islandRef,
    islandState,
    islandStateRef,
    islandVisibleRef,
    lastFitRequestRef,
    lastObservedGeometryRef,
    layout,
    pendingDimensionsRef,
    pendingHoverApplyRef,
    previewActiveRef,
    previewValuesRef,
    reconcileHoverStateRef,
    setCollapsedHeight,
    setExpandedHeight,
    setExpandedWidth,
    toolbarRevealRef,
    visibilityMotionActiveRef,
    widthMotionActiveRef,
  });
  const {
    applyIslandVisibility,
    cancelWidthMotion,
    handleIslandPointerEnter,
    handleIslandPointerLeave,
    handleIslandPointerMove,
    updateHoverFromPoint,
  } = useNotchIslandMotion({
    appearance,
    applyPendingDimensions,
    contentRef,
    dimensionsRef,
    flushHostReadyRef,
    hostFitReadyRef,
    islandRef,
    islandStateRef,
    islandSurfaceRef,
    islandVisibleRef,
    layout,
    pendingDimensionsRef,
    pendingHoverApplyRef,
    pendingVisibilityRef,
    previewActiveRef,
    reconcileHoverStateRef,
    setIslandState,
    setIslandVisible,
    setVisibilityMotionActive,
    setWidthMotionActive,
    shellRef,
    hoverAreaRef,
    toolbarRevealRef,
    visibilityMotionActiveRef,
    widthMotionActiveRef,
  });

  const renderSlot = (slot: NotchSlotContent, side: "left" | "right") => {
    const align = side === "left" ? "left" : "right";
    if (slot === "empty") return null;
    if (slot === "artwork") {
      return (
        <ArtworkTransitionImage
          alt=""
          artworkLoading={playback.artworkLoading}
          artworkUrl={playback.artworkUrl}
          className={styles.slotArtwork}
          draggable={false}
          fallbackSrc={appIconUrl}
        />
      );
    }
    if (slot === "spectrum") {
      return <SpectrumBars active={usesSpectrum && playback.active} register={registerSpectrumNode} />;
    }
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
        "--notch-secondary-font-weight": appearance.secondaryFontWeight,
        "--notch-active-color": appearance.activeColor,
        "--notch-inactive-color": appearance.inactiveColor,
        "--notch-translation-color": appearance.translationColor,
        "--notch-romanization-color": appearance.romanizationColor,
        "--notch-spectrum-color": spectrumColor,
        "--notch-line-gap": `${appearance.lineGap}px`,
        "--notch-radius": `${appearance.borderRadius}px`,
        "--notch-slot-padding": `${slotPadding}px`,
        "--notch-max-width": `${effectiveWidth}px`,
        "--notch-collapsed-height": `${Math.max(collapsedHeightFloor, collapsedHeight)}px`,
        "--notch-expanded-width": `${Math.min(effectiveExpandedMaxWidth, Math.max(effectiveWidth, expandedWidth))}px`,
        "--notch-expanded-height": `${Math.max(COLLAPSED_HEIGHT_FALLBACK, expandedHeight)}px`,
        "--notch-top-inset": `${resolvedTopInset}px`,
        "--notch-compact-slot-size": `${compactSlotSize}px`,
        "--notch-center-gap": `${layout.centerGapWidth}px`,
      } as CSSProperties}
    >
      <div className={styles.hoverArea} ref={hoverAreaRef}>
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
              {notch.showLyrics && (primaryLine || supportingLines.length > 0) && (
                <div
                  className={styles.lyricLines}
                  data-double-line-order={doubleLineOrder}
                  data-double-line-mode={alternatingDoubleLine ? "alternating" : undefined}
                  data-has-supporting-line={supportingLines.length > 0 || undefined}
                  data-supporting-line-kind={supportingLines[0]?.kind}
                >
                  {primaryLine && (
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
                  {supportingLines.map(({ kind, line }) => (
                    <div className={styles.supportingLine} data-empty={!line.text.trim() || undefined} data-kind={kind} key={`${kind}:${line.startMs}:${line.text}`}>
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
              )}
            </div>
            <div className={styles.toolbarReveal} ref={toolbarRevealRef}>
              <div className={styles.toolbarRevealInner}>
                <ExpandedPlayer
                  karaokeStyle={appearance.karaokeStyle}
                  marqueePaused={marqueePaused}
                  playback={playback}
                  previewLine={notch.showLyrics ? preview.line : null}
                  previewMaxDurationMs={previewLyricMarqueeTimeLimitMs}
                  previewPositionMs={previewPositionMs}
                  quickControls={quickControls}
                  t={t}
                />
              </div>
            </div>
          </div>
        </section>
      </div>
    </main>
  );
}
