import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, isTauriRuntime } from "../../shared/api";
import {
  defaultOverlayStyle,
  secondaryDisplayFlags,
  secondaryDisplayFromFlags,
  type OverlaySettings,
  type OverlayStyle,
  type ToolbarPlacement,
} from "../../shared/types";
import { useLyricsPresentation } from "../lyrics/useLyricsPresentation";
import { usePlayback } from "../player/usePlayback";
import styles from "./Overlay.module.scss";
import { OverlayKaraokeLine } from "./OverlayKaraokeLine";
import { useOverlayLyricsOffset } from "./useOverlayLyricsOffset";
import { useOverlayContentFit } from "./useOverlayContentFit";
import {
  formatOffset,
  formatOffsetMs,
  type MarqueeMetric,
  type SupportingLine,
} from "./OverlayLayout";
import { OverlayToolbar } from "./OverlayToolbar";
import { useOverlayResize } from "./useOverlayResize";
import { useOverlayWindowLayout } from "./useOverlayWindowLayout";

const HORIZONTAL_SURFACE_TOOLBAR_INSET = 46;
const VERTICAL_SURFACE_TOOLBAR_INSET = 48;
const MIN_HORIZONTAL_WIDTH = 320;
const MIN_VERTICAL_HEIGHT = 280;
const DEFAULT_HORIZONTAL_MAX_WIDTH = 760;
const DEFAULT_VERTICAL_MAX_HEIGHT = 620;
const DEFAULT_MARQUEE_DURATION_SECONDS = 4;
const MIN_MARQUEE_DURATION_SECONDS = 0.1;
const MARQUEE_EDGE_INSET = 16;

// 自动换行时行高也决定竖排列距，需要为文字描边预留额外空间。
function wrapLineHeight(fontSize: number, lineHeight: number, textStrokeWidth: number) {
  return `${Math.max(fontSize * lineHeight, fontSize + textStrokeWidth)}px`;
}

export default function Overlay() {
  const { t } = useTranslation();
  const playback = usePlayback();
  const lyrics = useLyricsPresentation(playback.snapshot, playback.positionMs);
  const [style, setStyle] = useState<OverlayStyle>(defaultOverlayStyle);
  const [settings, setSettings] = useState<OverlaySettings>({ visible: true, locked: false });
  const linesRef = useRef<HTMLDivElement>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);
  const activeRef = useRef<HTMLDivElement>(null);
  const supportingRefs = useRef<Array<HTMLDivElement | null>>([]);
  const fitFrame = useRef<number | null>(null);
  const fitRetryTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const shrinkTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const unlockFeedbackTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const styleRef = useRef(style);
  const lastRequestedSize = useRef<{ width: number; height: number } | null>(null);
  const lastMeasuredLayoutKey = useRef<string | null>(null);
  const [fitLimits, setFitLimits] = useState(() => ({
    width: Math.max(190, window.screen.availWidth - 48),
    height: Math.max(76, window.screen.availHeight - 48),
  }));
  const [fitScale, setFitScale] = useState(1);
  const [wrapped, setWrapped] = useState(false);
  const [marqueeMetrics, setMarqueeMetrics] = useState<MarqueeMetric[]>([]);
  const [overlayHovered, setOverlayHovered] = useState(false);
  const [unlockFeedback, setUnlockFeedback] = useState(false);
  const [toolbarSide, setToolbarSide] = useState<ToolbarPlacement>("top");
  const [toolbarMinimums, setToolbarMinimums] = useState({
    horizontal: MIN_HORIZONTAL_WIDTH,
    vertical: MIN_VERTICAL_HEIGHT,
  });
  const transparentMode = style.backgroundMode === "transparent" || style.background === "transparent";
  const glassEnabled = !transparentMode && style.background === "glass";
  const effectiveBackgroundOpacity = transparentMode ? 0 : style.backgroundOpacity;
  const nativeVibrancy = isTauriRuntime() && /Macintosh|Mac OS X/.test(navigator.userAgent);
  const backdropFilter = glassEnabled && !nativeVibrancy
    ? `blur(${style.backgroundBlur}px) saturate(1.2)`
    : "none";
  const minimumHorizontalWidth = Math.min(fitLimits.width, toolbarMinimums.horizontal);
  const minimumVerticalHeight = Math.min(fitLimits.height, toolbarMinimums.vertical);
  const {
    activeResizeEdge,
    beginResize,
    cancelResize,
    clearResizeState,
    continueResize,
    endResize,
    finishResizeRef,
    lostResizeCapture,
    resizing,
  } = useOverlayResize({
    locked: settings.locked,
    minimumHorizontalWidth,
    minimumVerticalHeight,
    setStyle,
    styleRef,
  });

  const updateStyle = async (patch: Partial<OverlayStyle>) => {
    const next = { ...styleRef.current, ...patch };
    styleRef.current = next;
    setStyle(next);
    const saved = await api.setOverlayStyle(next);
    styleRef.current = saved;
    setStyle(saved);
  };

  const primaryText = lyrics.currentLine
    ? lyrics.currentLine.text
    : playback.snapshot.title || "Lyrics Plus";
  const activeLineEmpty = Boolean(lyrics.currentLine && !lyrics.currentLine.text.trim());
  const primaryLineKey = `${lyrics.currentLine?.startMs ?? "fallback"}:${primaryText}`;
  const currentLineDisplayEndMs = lyrics.nextLine?.startMs ?? lyrics.currentLine?.endMs;
  const marqueeTimeLimit = lyrics.currentLine && currentLineDisplayEndMs != null
    ? Math.max(
      MIN_MARQUEE_DURATION_SECONDS,
      (currentLineDisplayEndMs - lyrics.currentLine.startMs) / 1000,
    )
    : null;
  const vertical = style.orientation === "vertical";
  const overlayHorizontalPadding = style.backgroundPaddingX * 2
    + (vertical ? VERTICAL_SURFACE_TOOLBAR_INSET : 0);
  const overlayVerticalPadding = style.backgroundPaddingY * 2
    + (vertical ? 0 : HORIZONTAL_SURFACE_TOOLBAR_INSET);

  const supportsSecondary = style.layout === "double";
  const secondaryFlags = secondaryDisplayFlags(style.secondaryDisplay);
  const translationAvailable = Boolean(lyrics.document?.tracks.translation);
  const romanizationAvailable = Boolean(lyrics.document?.tracks.romanization);
  const selectedSupportingLines: SupportingLine[] = [
    ...(secondaryFlags.translation && lyrics.currentTranslation
      ? [{ kind: "translation" as const, text: lyrics.currentTranslation.text, baseSize: style.fontSize * style.translationFontScale, color: style.translationColor }]
      : []),
    ...(secondaryFlags.romanization && lyrics.currentRomanization
      ? [{ kind: "romanization" as const, text: lyrics.currentRomanization.text, baseSize: style.fontSize * style.romanizationFontScale, color: style.romanizationColor }]
      : []),
  ];
  const fallbackSupportingLine: SupportingLine = {
    kind: "next",
    text: !lyrics.document
      ? playback.snapshot.artist || t("overlay.fallback")
      : lyrics.nextLine?.text || "\u00a0",
    baseSize: style.fontSize * style.secondaryFontScale,
    color: style.inactiveColor,
  };
  const supportingLines: SupportingLine[] = !supportsSecondary
    ? []
    : [selectedSupportingLines[0] ?? fallbackSupportingLine];
  const alternatingDoubleLine = supportsSecondary
    && style.doubleLineMode === "alternating"
    && lyrics.activeIndex >= 0
    && supportingLines[0]?.kind === "next";
  const showingTranslationOrRomanization = supportingLines.some(
    (line) => line.kind === "translation" || line.kind === "romanization",
  );
  const primaryLineReversed = showingTranslationOrRomanization && style.primaryLinePosition === "second";
  const doubleLineOrder = (primaryLineReversed || (alternatingDoubleLine && lyrics.activeIndex % 2 === 1))
    ? "reversed"
    : "normal";
  const effectiveAlignment = !supportsSecondary
    || (style.autoCenterWithTranslationOrRomanization && showingTranslationOrRomanization)
    ? "center"
    : style.alignment;
  const supportingKey = supportingLines.map((line) => `${line.kind}:${line.text}`).join("|");
  const offsetAvailable = Boolean(lyrics.document);
  const offsetMs = lyrics.document?.offsetMs ?? 0;
  const { setLyricsOffset, changeLyricsOffset } = useOverlayLyricsOffset(lyrics.trackKey, offsetMs);
  const offsetLabel = offsetAvailable ? formatOffset(offsetMs) : "—";
  const offsetValueTitle = offsetAvailable
    ? offsetMs === 0
      ? t("overlay.toolbar.offsetZeroTitle")
      : t("overlay.toolbar.offsetTitle", { value: formatOffsetMs(offsetMs) })
    : t("overlay.toolbar.noOffset");
  const backgroundLabel = transparentMode
    ? t("overlay.toolbar.backgroundTransparent")
    : t("overlay.toolbar.backgroundVisible");

  useOverlayWindowLayout({
    clearResizeState,
    finishResizeRef,
    fitLimits,
    fitRetryTimer,
    lastRequestedSize,
    minimumHorizontalWidth,
    minimumVerticalHeight,
    offsetLabel,
    setFitLimits,
    setOverlayHovered,
    setSettings,
    setStyle,
    setToolbarMinimums,
    setToolbarSide,
    setUnlockFeedback,
    settings,
    style,
    styleRef,
    toolbarRef,
    unlockFeedbackTimer,
    vertical,
  });

  const horizontalWindowLimit = Math.min(
    fitLimits.width,
    Math.max(minimumHorizontalWidth, style.horizontalMaxWidth ?? DEFAULT_HORIZONTAL_MAX_WIDTH),
  );
  const verticalWindowLimit = Math.min(
    fitLimits.height,
    Math.max(minimumVerticalHeight, style.verticalMaxHeight ?? DEFAULT_VERTICAL_MAX_HEIGHT),
  );
  const horizontalContentLimit = Math.max(1, horizontalWindowLimit - overlayHorizontalPadding);
  const verticalContentLimit = Math.max(1, verticalWindowLimit - overlayVerticalPadding);
  const marqueeHorizontalLimit = Math.max(1, horizontalContentLimit - MARQUEE_EDGE_INSET * 2);
  const marqueeVerticalLimit = Math.max(1, verticalContentLimit - MARQUEE_EDGE_INSET * 2);
  const constrained = style.longText === "wrap"
    ? wrapped
    : style.longText === "marquee" && marqueeMetrics.some((metric) => metric.overflowing);

  useOverlayContentFit({
    activeRef,
    constrained,
    fitFrame,
    fitLimits,
    fitRetryTimer,
    fitScale,
    horizontalContentLimit,
    horizontalWindowLimit,
    lastMeasuredLayoutKey,
    lastRequestedSize,
    linesRef,
    marqueeHorizontalLimit,
    marqueeMetrics,
    marqueeTimeLimit,
    marqueeVerticalLimit,
    overlayHorizontalPadding,
    overlayVerticalPadding,
    primaryLineKey,
    primaryText,
    preserveSizeForEmptyLine: activeLineEmpty,
    resizing,
    setFitScale,
    setMarqueeMetrics,
    setWrapped,
    settingsVisible: settings.visible,
    shrinkTimer,
    style,
    supportingKey,
    supportingLines,
    supportingRefs,
    vertical,
    verticalContentLimit,
    verticalWindowLimit,
    wrapped,
  });

  const toggleSupportingTrack = (kind: "translation" | "romanization") => {
    const translation = kind === "translation" ? !secondaryFlags.translation : secondaryFlags.translation;
    const romanization = kind === "romanization" ? !secondaryFlags.romanization : secondaryFlags.romanization;
    void updateStyle({ secondaryDisplay: secondaryDisplayFromFlags(translation, romanization) });
  };

  const supportingToggleTitle = (track: string, enabled: boolean, available: boolean) => {
    const action = enabled ? t("overlay.toolbar.hideTrack", { track }) : t("overlay.toolbar.showTrack", { track });
    if (!supportsSecondary) return t("overlay.toolbar.unsupportedLayout", { action });
    if (!available) return t("overlay.toolbar.unavailableTrack", { action, track });
    return action;
  };

  return (
    <main
      className={styles.overlay}
      data-alignment={effectiveAlignment}
      data-background={glassEnabled ? "glass" : "solid"}
      data-background-mode={transparentMode ? "transparent" : "solid"}
      data-interactive={!settings.locked}
      data-layout={style.layout}
      data-orientation={style.orientation}
      data-toolbar-placement={toolbarSide}
      data-long-text={style.longText}
      data-constrained={constrained}
      data-hover={overlayHovered || unlockFeedback}
      data-resizing={resizing}
      data-tauri-drag-region={settings.locked ? "false" : "deep"}
      style={{
        "--lyric-font-family": style.fontFamily,
        "--lyric-size": `${style.fontSize}px`,
        "--lyric-font-weight": style.fontWeight,
        "--secondary-font-weight": style.secondaryFontWeight,
        "--lyric-line-height": style.lineHeight,
        "--active-color": style.activeColor,
        "--inactive-color": style.inactiveColor,
        "--overlay-opacity": style.opacity,
        "--background-opacity": effectiveBackgroundOpacity,
        "--background-radius": `${style.backgroundRadius}px`,
        "--background-padding-x": `${style.backgroundPaddingX}px`,
        "--background-padding-y": `${style.backgroundPaddingY}px`,
        "--line-gap": `${style.lineGap}px`,
        "--solid-color": style.solidColor,
        "--text-shadow": `${style.textShadowOffsetX}px ${style.textShadowOffsetY}px ${style.textShadowBlur}px ${style.textShadowColor}`,
        "--text-stroke-width": `${style.textStrokeWidth * fitScale}px`,
        "--text-stroke-color": style.textStrokeColor,
        "--translation-color": style.translationColor,
        "--romanization-color": style.romanizationColor,
        "--content-max-width": `${Math.max(1, fitLimits.width - overlayHorizontalPadding)}px`,
        "--content-max-height": `${Math.max(1, fitLimits.height - overlayVerticalPadding)}px`,
        "--line-width-limit": `${horizontalContentLimit}px`,
        "--line-height-limit": `${verticalContentLimit}px`,
        "--marquee-line-width-limit": `${marqueeHorizontalLimit}px`,
        "--marquee-line-height-limit": `${marqueeVerticalLimit}px`,
        "--content-min-width": `${horizontalContentLimit}px`,
      } as React.CSSProperties}
      tabIndex={settings.locked ? -1 : 0}
    >
      <div
        className={styles.surface}
        style={{
          backdropFilter,
          WebkitBackdropFilter: backdropFilter,
        }}
      >
        <div className={styles.lines} data-double-line-order={doubleLineOrder} ref={linesRef}>
          <div
            className={styles.active}
            data-empty={activeLineEmpty}
            data-marquee={style.longText === "marquee" && marqueeMetrics[0]?.overflowing}
            ref={activeRef}
            style={{
              fontSize: `${style.fontSize * fitScale}px`,
              "--wrap-line-height": wrapLineHeight(style.fontSize * fitScale, style.lineHeight, style.textStrokeWidth * fitScale),
              "--marquee-distance": `${marqueeMetrics[0]?.distance ?? 0}px`,
              "--marquee-duration": `${marqueeMetrics[0]?.duration ?? DEFAULT_MARQUEE_DURATION_SECONDS}s`,
            } as React.CSSProperties}
          >
            <OverlayKaraokeLine key={primaryLineKey} line={lyrics.currentLine} fallback={primaryText} positionMs={lyrics.adjustedPositionMs} style={style} />
          </div>
          {supportingLines.map((line, index) => (
            <div
              className={styles.next}
              data-kind={line.kind}
              data-marquee={style.longText === "marquee" && marqueeMetrics[index + 1]?.overflowing}
              key={`${line.kind}:${line.text}`}
              ref={(element) => { supportingRefs.current[index] = element; }}
              style={{
                color: line.color,
                fontSize: `${line.baseSize * fitScale}px`,
                "--wrap-line-height": wrapLineHeight(line.baseSize * fitScale, style.lineHeight, style.textStrokeWidth * fitScale),
                "--marquee-distance": `${marqueeMetrics[index + 1]?.distance ?? 0}px`,
                "--marquee-duration": `${marqueeMetrics[index + 1]?.duration ?? DEFAULT_MARQUEE_DURATION_SECONDS}s`,
              } as React.CSSProperties}
            ><span>{line.text}</span></div>
          ))}
        </div>
      </div>

      {!settings.locked && (
        <>
          {vertical ? (
            <>
              <div className={styles.resizeHandle} data-active={activeResizeEdge === "top"} data-edge="top" data-tauri-drag-region="false" role="separator" aria-label={t("overlay.toolbar.resizeVertical")} aria-orientation="horizontal" onLostPointerCapture={lostResizeCapture} onPointerCancel={cancelResize} onPointerDown={beginResize("top", "vertical")} onPointerMove={continueResize} onPointerUp={endResize} />
              <div className={styles.resizeHandle} data-active={activeResizeEdge === "bottom"} data-edge="bottom" data-tauri-drag-region="false" role="separator" aria-label={t("overlay.toolbar.resizeVertical")} aria-orientation="horizontal" onLostPointerCapture={lostResizeCapture} onPointerCancel={cancelResize} onPointerDown={beginResize("bottom", "vertical")} onPointerMove={continueResize} onPointerUp={endResize} />
            </>
          ) : (
            <>
              <div className={styles.resizeHandle} data-active={activeResizeEdge === "left"} data-edge="left" data-tauri-drag-region="false" role="separator" aria-label={t("overlay.toolbar.resizeHorizontal")} aria-orientation="vertical" onLostPointerCapture={lostResizeCapture} onPointerCancel={cancelResize} onPointerDown={beginResize("left", "horizontal")} onPointerMove={continueResize} onPointerUp={endResize} />
              <div className={styles.resizeHandle} data-active={activeResizeEdge === "right"} data-edge="right" data-tauri-drag-region="false" role="separator" aria-label={t("overlay.toolbar.resizeHorizontal")} aria-orientation="vertical" onLostPointerCapture={lostResizeCapture} onPointerCancel={cancelResize} onPointerDown={beginResize("right", "horizontal")} onPointerMove={continueResize} onPointerUp={endResize} />
            </>
          )}
          <OverlayToolbar
            backgroundLabel={backgroundLabel}
            changeLyricsOffset={changeLyricsOffset}
            offsetAvailable={offsetAvailable}
            offsetLabel={offsetLabel}
            offsetMs={offsetMs}
            offsetValueTitle={offsetValueTitle}
            romanizationAvailable={romanizationAvailable}
            secondaryFlags={secondaryFlags}
            setLyricsOffset={setLyricsOffset}
            style={style}
            supportingToggleTitle={supportingToggleTitle}
            t={t}
            toggleSupportingTrack={toggleSupportingTrack}
            toolbarRef={toolbarRef}
            translationAvailable={translationAvailable}
            transparentMode={transparentMode}
            updateStyle={updateStyle}
            vertical={vertical}
          />
        </>
      )}
    </main>
  );
}
