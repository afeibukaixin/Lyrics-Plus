import {
  useCallback,
  useRef,
  type CSSProperties,
} from "react";
import { useTranslation } from "react-i18next";
import appIconUrl from "../../../src-tauri/icons/128x128@2x.png";
import { api } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import { useAppConfig } from "../config/AppConfigProvider";
import { usePlayback } from "../player/usePlayback";
import { useLyricsPresentation } from "./useLyricsPresentation";
import { useNotchLyricsOffset } from "./useNotchLyricsOffset";
import { useNotchIslandMotion } from "./useNotchIslandMotion";
import { useNotchSpectrum } from "./useNotchSpectrum";
import { useNotchWindowGeometry } from "./useNotchWindowGeometry";
import { useNotchWindowEvents } from "./useNotchWindowEvents";
import { useNotchWindowState } from "./useNotchWindowState";
import type {
  LyricsLine,
  NotchLyricsPreferences,
  NotchSlotContent,
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
  notchCollapsedHeightFloor,
  notchSlotPadding,
  NOTCH_MAX_WIDTH,
  resolvedNotchTopInset,
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
  const {
    layout,
    setLayout,
    islandState,
    setIslandState,
    expandedWidth,
    setExpandedWidth,
    collapsedHeight,
    setCollapsedHeight,
    expandedHeight,
    setExpandedHeight,
    previewValues,
    setPreviewValues,
    previewActive,
    setPreviewActive,
    widthMotionActive,
    setWidthMotionActive,
    islandVisible,
    setIslandVisible,
    visibilityMotionActive,
    setVisibilityMotionActive,
    shellRef,
    hoverAreaRef,
    islandRef,
    islandVisualRef,
    contentRef,
    toolbarRevealRef,
    islandStateRef,
    hostFitReadyRef,
    pendingHoverApplyRef,
    pendingVisibilityRef,
    flushHostReadyRef,
    islandVisibleRef,
    visibilityMotionActiveRef,
    widthMotionActiveRef,
    previewActiveRef,
    previewValuesRef,
    dimensionsRef,
    pendingDimensionsRef,
    lastFitRequestRef,
    lastObservedGeometryRef,
    reconcileHoverStateRef,
  } = useNotchWindowState({ appearance });
  const usesSpectrum = notch.leftSlot === "spectrum" || notch.rightSlot === "spectrum";
  const spectrumColors = playback.artworkSpectrumColors ?? {
    left: { top: "#ffffff", middle: "#ffffff", bottom: "#ffffff" },
    center: { top: "#ffffff", middle: "#ffffff", bottom: "#ffffff" },
    right: { top: "#ffffff", middle: "#ffffff", bottom: "#ffffff" },
  };
  const { registerSpectrumNode } = useNotchSpectrum(usesSpectrum && playback.active);
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
  // 同行布局也决定隐藏 compact 内容的收起高度，不能跟随展开动画状态切换。
  const inlineLyricsOnNonNotch = !layout.hasNotch
    && notch.inlineLyricsOnNonNotch
    && notch.showLyrics
    && Boolean(primaryLine?.text.trim());
  const primaryLineElement = primaryLine && (
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
  );
  const supportingLine = supportingLines[0];
  const supportingLineElement = supportingLine && (
    <div className={styles.supportingLine} data-empty={!supportingLine.line.text.trim() || undefined} data-kind={supportingLine.kind} key={`${supportingLine.kind}:${supportingLine.line.startMs}:${supportingLine.line.text}`}>
      <OverflowText
        align="center"
        behavior="once"
        contentKey={`${supportingLine.kind}:${supportingLine.line.startMs}:${supportingLine.line.text}`}
        maxDurationMs={lyricMarqueeTimeLimitMs}
        paused={marqueePaused}
      >
        {supportingLine.line.text}
      </OverflowText>
    </div>
  );
  const inlineDoubleLine = inlineLyricsOnNonNotch
    && notch.layout === "double"
    && Boolean(supportingLineElement);
  const inlineDoubleReversed = inlineDoubleLine && doubleLineOrder === "reversed";
  const inlineTopLineElement = inlineDoubleReversed ? supportingLineElement : primaryLineElement;
  const inlineBottomLineElement = inlineDoubleReversed ? primaryLineElement : supportingLineElement;
  const inlineBottomLineKind = inlineDoubleLine && !inlineDoubleReversed
    ? supportingLine?.kind
    : undefined;

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
    islandVisualRef,
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

  useNotchWindowEvents({
    setLayout,
    applyIslandVisibility,
    updateHoverFromPoint,
    previewValuesRef,
    setPreviewValues,
    previewActiveRef,
    setPreviewActive,
    cancelWidthMotion,
    fitWindow,
    hostFitReadyRef,
    pendingHoverApplyRef,
    reconcileHoverStateRef,
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
        "--notch-spectrum-left-top-color": spectrumColors.left.top,
        "--notch-spectrum-left-middle-color": spectrumColors.left.middle,
        "--notch-spectrum-left-bottom-color": spectrumColors.left.bottom,
        "--notch-spectrum-center-top-color": spectrumColors.center.top,
        "--notch-spectrum-center-middle-color": spectrumColors.center.middle,
        "--notch-spectrum-center-bottom-color": spectrumColors.center.bottom,
        "--notch-spectrum-right-top-color": spectrumColors.right.top,
        "--notch-spectrum-right-middle-color": spectrumColors.right.middle,
        "--notch-spectrum-right-bottom-color": spectrumColors.right.bottom,
        "--notch-line-gap": `${appearance.lineGap}px`,
        "--notch-radius": `${appearance.borderRadius}px`,
        "--notch-top-radius": `${appearance.topBorderRadius}px`,
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
          <div className={styles.islandVisual} ref={islandVisualRef}>
            <div className={styles.islandSurface}>
              {layout.hasNotch && (
                <div aria-hidden="true" className={styles.brandCapsule}>
                  <img alt="" draggable={false} src={appIconUrl} />
                  <span>Lyrics Plus</span>
                </div>
              )}
              <div className={styles.content} data-inline-double-line={inlineDoubleLine || undefined} ref={contentRef}>
                <header className={styles.metadata} data-inline-lyrics={inlineLyricsOnNonNotch || undefined}>
                  <div className={styles.slot} data-side="left" data-slot={notch.leftSlot}>{renderSlot(notch.leftSlot, "left")}</div>
                  {inlineLyricsOnNonNotch ? inlineTopLineElement : <span className={styles.notchGap} aria-hidden="true" />}
                  <div className={styles.slot} data-side="right" data-slot={notch.rightSlot}>{renderSlot(notch.rightSlot, "right")}</div>
                </header>
                {notch.showLyrics && ((primaryLine && !inlineLyricsOnNonNotch) || supportingLines.length > 0) && (
                  <div
                    className={styles.lyricLines}
                    data-double-line-order={doubleLineOrder}
                    data-double-line-mode={alternatingDoubleLine ? "alternating" : undefined}
                    data-has-supporting-line={(!inlineLyricsOnNonNotch && supportingLines.length > 0) || undefined}
                    data-supporting-line-kind={inlineBottomLineKind ?? (!inlineLyricsOnNonNotch ? supportingLine?.kind : undefined)}
                  >
                    {inlineDoubleLine
                      ? inlineBottomLineElement
                      : <>
                        {!inlineLyricsOnNonNotch && primaryLineElement}
                        {supportingLineElement}
                      </>}
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
          </div>
        </section>
      </div>
    </main>
  );
}
