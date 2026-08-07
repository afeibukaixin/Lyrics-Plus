import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { listen } from "@tauri-apps/api/event";
import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";
import { api, isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import {
  defaultOverlayStyle,
  secondaryDisplayFlags,
  secondaryDisplayFromFlags,
  type LyricsLine,
  type OverlayResizeBounds,
  type OverlayResizeEdge,
  type OverlaySettings,
  type OverlayStyle,
} from "../../shared/types";
import { useLyrics } from "../lyrics/useLyrics";
import { usePlayback } from "../player/usePlayback";
import styles from "./Overlay.module.scss";

const layoutLabels: Record<OverlayStyle["layout"], string> = {
  single: "单歌词",
  double: "双歌词",
};

const orientationLabels: Record<OverlayStyle["orientation"], string> = {
  horizontal: "横排",
  vertical: "竖排",
};

const HORIZONTAL_OVERLAY_HORIZONTAL_PADDING = 52;
const HORIZONTAL_OVERLAY_VERTICAL_PADDING = 66;
const VERTICAL_OVERLAY_HORIZONTAL_PADDING = 66;
const VERTICAL_OVERLAY_VERTICAL_PADDING = 52;
const HORIZONTAL_LINE_GAP = 8;
const VERTICAL_COLUMN_GAP = 14;
const MIN_LYRIC_FONT_SIZE = 12;
const MIN_HORIZONTAL_WIDTH = 320;
const MIN_VERTICAL_HEIGHT = 280;
const HORIZONTAL_TOOLBAR_WINDOW_INSET = 8;
const VERTICAL_TOOLBAR_WINDOW_INSET = 14;
const DEFAULT_HORIZONTAL_MAX_WIDTH = 760;
const DEFAULT_VERTICAL_MAX_HEIGHT = 620;
const SHRINK_DELAY_MS = 700;
const MARQUEE_SPEED_PX_PER_SECOND = 35;
const DEFAULT_MARQUEE_DURATION_SECONDS = 4;
const MIN_MARQUEE_DURATION_SECONDS = 0.1;
const MARQUEE_EDGE_INSET = 16;

type MarqueeMetric = {
  overflowing: boolean;
  distance: number;
  duration: number;
};

type ActiveResizeSession = {
  pointerId: number;
  edge: OverlayResizeEdge;
  axis: "horizontal" | "vertical";
  handle: HTMLDivElement;
  startCoordinate: number;
  latestCoordinate: number;
  startMainSize: number | null;
  minimumMainSize: number;
  pendingMainSize: number | null;
  lastBounds: OverlayResizeBounds | null;
  processing: boolean;
  ending: boolean;
  committing: boolean;
};

function sameMarqueeMetrics(left: MarqueeMetric[], right: MarqueeMetric[]) {
  return left.length === right.length && left.every((item, index) => {
    const other = right[index];
    return item.overflowing === other.overflowing
      && Math.abs(item.distance - other.distance) < 0.5
      && Math.abs(item.duration - other.duration) < 0.05;
  });
}

function nextValue<T extends string>(current: T, values: readonly T[]) {
  return values[(values.indexOf(current) + 1) % values.length];
}

function combinedContentSize(
  items: Array<{ width: number; height: number }>,
  layout: OverlayStyle["layout"],
  orientation: OverlayStyle["orientation"],
) {
  const [primary, ...secondary] = items;
  if (!primary) return { width: 0, height: 0 };
  if (secondary.length === 0 || layout === "single") return primary;
  if (orientation === "horizontal") {
    return {
      width: Math.max(...items.map((item) => item.width)),
      height: items.reduce((total, item) => total + item.height, 0) + HORIZONTAL_LINE_GAP * (items.length - 1),
    };
  }
  return {
    width: items.reduce((total, item) => total + item.width, 0) + VERTICAL_COLUMN_GAP * (items.length - 1),
    height: Math.max(...items.map((item) => item.height)),
  };
}

type SupportingKind = "next" | "translation" | "romanization";

type SupportingLine = {
  kind: SupportingKind;
  text: string;
  baseSize: number;
  color: string;
};

type ToolbarIconName = "lock" | "minus" | "plus" | "offsetEarlier" | "offsetLater" | "layout" | "orientation" | "hide" | "settings";

function ToolbarIcon({ name }: { name: ToolbarIconName }) {
  const paths: Record<ToolbarIconName, React.ReactNode> = {
    lock: <><rect x="5" y="10" width="14" height="10" rx="2" /><path d="M8 10V7a4 4 0 0 1 8 0v3" /></>,
    minus: <path d="M6 12h12" />,
    plus: <><path d="M6 12h12" /><path d="M12 6v12" /></>,
    offsetEarlier: <><path d="M9 8l-4 4 4 4M5 12h7" /><circle cx="16" cy="12" r="5" /><path d="M16 9.5V12l1.8 1.2" /></>,
    offsetLater: <><circle cx="8" cy="12" r="5" /><path d="M8 9.5V12l1.8 1.2M15 8l4 4-4 4M12 12h7" /></>,
    layout: <><rect x="4" y="5" width="16" height="5" rx="1" /><rect x="4" y="14" width="7" height="5" rx="1" /><rect x="13" y="14" width="7" height="5" rx="1" /></>,
    orientation: <><path d="M5 7h14M5 17h14" /><path d="m8 4-3 3 3 3M16 14l3 3-3 3" /></>,
    hide: <><path d="M3 3l18 18" /><path d="M10.6 10.7a2 2 0 0 0 2.7 2.7" /><path d="M9.9 4.2A10.8 10.8 0 0 1 12 4c5 0 9 4.5 9 8a8.7 8.7 0 0 1-2 4.5M6.6 6.6C4.4 8 3 10.1 3 12c0 3.5 4 8 9 8 1.3 0 2.6-.3 3.7-.8" /></>,
    settings: <><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-1.6v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z" /></>,
  };
  return <svg aria-hidden="true" viewBox="0 0 24 24">{paths[name]}</svg>;
}

function formatOffset(offsetMs: number) {
  if (offsetMs === 0) return "0s";
  const seconds = (Math.abs(offsetMs) / 1000).toFixed(3).replace(/\.?0+$/, "");
  return `${offsetMs > 0 ? "+" : "−"}${seconds}s`;
}

function formatOffsetMs(offsetMs: number) {
  if (offsetMs === 0) return "0ms";
  return `${offsetMs > 0 ? "+" : "−"}${Math.abs(offsetMs)}ms`;
}

function KaraokeLine({ line, fallback, positionMs, style }: {
  line: LyricsLine | null;
  fallback: string;
  positionMs: number;
  style: OverlayStyle;
}) {
  const text = line?.text || fallback;
  const words = line?.words?.filter((word) => word.text.length > 0) ?? [];
  if (words.length === 0) return <span>{text || "\u00a0"}</span>;
  return (
    <span className={styles.karaokeText} data-karaoke={style.karaokeStyle}>
      {words.map((word, index) => {
        const duration = Math.max(0, word.endMs - word.startMs);
        const progress = positionMs <= word.startMs
          ? 0
          : duration === 0 || positionMs >= word.endMs
            ? 100
            : ((positionMs - word.startMs) / duration) * 100;
        const current = positionMs >= word.startMs && positionMs < word.endMs;
        return (
          <span
            className={styles.karaokeWord}
            data-complete={positionMs >= word.endMs || (duration === 0 && positionMs >= word.startMs)}
            data-current={current}
            key={`${word.startMs}-${index}`}
            style={{ "--word-progress": `${progress}%` } as React.CSSProperties}
          >
            <span className={styles.karaokeWordBase}>{word.text}</span>
            <span aria-hidden="true" className={styles.karaokeWordFill}>{word.text}</span>
          </span>
        );
      })}
    </span>
  );
}

export default function Overlay() {
  const playback = usePlayback();
  const lyrics = useLyrics(playback.snapshot, playback.positionMs, true);
  const [style, setStyle] = useState<OverlayStyle>(defaultOverlayStyle);
  const [settings, setSettings] = useState<OverlaySettings>({ visible: true, locked: false, passthrough: false });
  const linesRef = useRef<HTMLDivElement>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);
  const activeRef = useRef<HTMLDivElement>(null);
  const supportingRefs = useRef<Array<HTMLDivElement | null>>([]);
  const fitFrame = useRef<number | null>(null);
  const shrinkTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const resizeSession = useRef<ActiveResizeSession | null>(null);
  const finishResizeRef = useRef<() => void>(() => undefined);
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
  const [activeResizeEdge, setActiveResizeEdge] = useState<OverlayResizeEdge | null>(null);
  const [toolbarMinimums, setToolbarMinimums] = useState({
    horizontal: MIN_HORIZONTAL_WIDTH,
    vertical: MIN_VERTICAL_HEIGHT,
  });
  const resizing = activeResizeEdge !== null;

  const clearResizeState = useCallback(() => {
    const session = resizeSession.current;
    resizeSession.current = null;
    if (session?.handle.hasPointerCapture(session.pointerId)) {
      try { session.handle.releasePointerCapture(session.pointerId); } catch { /* 已由系统释放 */ }
    }
    setActiveResizeEdge(null);
  }, []);

  useEffect(() => {
    styleRef.current = style;
  }, [style]);

  useEffect(() => {
    document.documentElement.dataset.window = "overlay";
    if (!isTauriRuntime()) return;
    void api.getOverlayStyle().then((saved) => {
      styleRef.current = saved;
      setStyle(saved);
    });
    void api.getOverlaySettings().then(setSettings);
    const cleanupStyleListener = createTauriListenerCleanup(listen<OverlayStyle>("overlay://style", ({ payload }) => {
      clearResizeState();
      styleRef.current = payload;
      setStyle(payload);
    }));
    const cleanupSettingsListener = createTauriListenerCleanup(listen<OverlaySettings>("overlay://settings", ({ payload }) => {
      if (payload.locked) clearResizeState();
      setSettings(payload);
    }));
    return () => {
      cleanupStyleListener();
      cleanupSettingsListener();
    };
  }, [clearResizeState]);

  useEffect(() => {
    const clearSelection = () => {
      const selection = window.getSelection();
      if (selection && selection.rangeCount > 0) selection.removeAllRanges();
    };
    const preventSelection = (event: Event) => {
      event.preventDefault();
      clearSelection();
    };
    clearSelection();
    document.addEventListener("selectstart", preventSelection);
    document.addEventListener("selectionchange", clearSelection);
    document.addEventListener("dragstart", preventSelection);
    return () => {
      document.removeEventListener("selectstart", preventSelection);
      document.removeEventListener("selectionchange", clearSelection);
      document.removeEventListener("dragstart", preventSelection);
    };
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    const refreshLimits = async () => {
      const monitor = await currentMonitor();
      if (!monitor) return;
      const width = monitor.workArea.size.width / monitor.scaleFactor - 48;
      const height = monitor.workArea.size.height / monitor.scaleFactor - 48;
      setFitLimits({ width: Math.max(190, width), height: Math.max(76, height) });
    };
    void refreshLimits();
    return createTauriListenerCleanup(
      getCurrentWindow().onMoved(() => void refreshLimits()),
    );
  }, []);

  useEffect(() => {
    const onBlur = () => finishResizeRef.current();
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("blur", onBlur);
      clearResizeState();
    };
  }, [clearResizeState]);

  useEffect(() => {
    if (settings.locked) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.key.startsWith("Arrow")) return;
      const step = event.shiftKey ? 10 : 1;
      const movement: Record<string, [number, number]> = {
        ArrowLeft: [-step, 0],
        ArrowRight: [step, 0],
        ArrowUp: [0, -step],
        ArrowDown: [0, step],
      };
      const delta = movement[event.key];
      if (!delta) return;
      event.preventDefault();
      void api.nudgeOverlay(delta[0], delta[1]);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [settings.locked]);

  const updateStyle = async (patch: Partial<OverlayStyle>) => {
    const next = { ...styleRef.current, ...patch };
    styleRef.current = next;
    setStyle(next);
    const saved = await api.setOverlayStyle(next);
    styleRef.current = saved;
    setStyle(saved);
  };

  const primaryText = lyrics.currentLine?.text || playback.snapshot.title || "Lyrics Plus";
  const primaryLineKey = `${lyrics.currentLine?.startMs ?? "fallback"}:${primaryText}`;
  const currentLineDisplayEndMs = lyrics.nextLine?.startMs ?? lyrics.currentLine?.endMs;
  const marqueeTimeLimit = lyrics.currentLine && currentLineDisplayEndMs != null
    ? Math.max(
      MIN_MARQUEE_DURATION_SECONDS,
      (currentLineDisplayEndMs - lyrics.currentLine.startMs) / 1000,
    )
    : null;
  const vertical = style.orientation === "vertical";
  const overlayHorizontalPadding = vertical
    ? VERTICAL_OVERLAY_HORIZONTAL_PADDING
    : HORIZONTAL_OVERLAY_HORIZONTAL_PADDING;
  const overlayVerticalPadding = vertical
    ? VERTICAL_OVERLAY_VERTICAL_PADDING
    : HORIZONTAL_OVERLAY_VERTICAL_PADDING;

  const minimumHorizontalWidth = Math.min(fitLimits.width, toolbarMinimums.horizontal);
  const minimumVerticalHeight = Math.min(fitLimits.height, toolbarMinimums.vertical);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    const overlayWindow = getCurrentWindow();
    void overlayWindow.setMinSize(new LogicalSize(
      vertical ? 190 : minimumHorizontalWidth,
      vertical ? minimumVerticalHeight : 76,
    ));
    void overlayWindow.setMaxSize(new LogicalSize(fitLimits.width, fitLimits.height));
  }, [fitLimits.height, fitLimits.width, minimumHorizontalWidth, minimumVerticalHeight, vertical]);

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
      ? playback.snapshot.artist || "播放音乐后自动显示歌词"
      : lyrics.nextLine?.text || "\u00a0",
    baseSize: style.fontSize * style.secondaryFontScale,
    color: style.inactiveColor,
  };
  const supportingLines: SupportingLine[] = !supportsSecondary
    ? []
    : [selectedSupportingLines[0] ?? fallbackSupportingLine];
  const showingTranslationOrRomanization = supportingLines.some(
    (line) => line.kind === "translation" || line.kind === "romanization",
  );
  const effectiveAlignment = !supportsSecondary
    || (style.autoCenterWithTranslationOrRomanization && showingTranslationOrRomanization)
    ? "center"
    : style.alignment;
  const supportingKey = supportingLines.map((line) => `${line.kind}:${line.text}`).join("|");
  const offsetAvailable = Boolean(lyrics.document);
  const offsetMs = lyrics.document?.offsetMs ?? 0;
  const offsetLabel = offsetAvailable ? formatOffset(offsetMs) : "—";
  const offsetValueTitle = offsetAvailable
    ? offsetMs === 0
      ? "当前歌词偏移：0ms"
      : `当前歌词偏移：${formatOffsetMs(offsetMs)}；点击重置`
    : "当前歌曲没有可调整的同步歌词";

  useLayoutEffect(() => {
    const toolbar = toolbarRef.current;
    if (!toolbar || settings.locked) return;
    const measureToolbar = () => {
      const measured = vertical
        ? Math.ceil(toolbar.scrollHeight + VERTICAL_TOOLBAR_WINDOW_INSET)
        : Math.ceil(toolbar.scrollWidth + HORIZONTAL_TOOLBAR_WINDOW_INSET);
      const minimum = Math.max(
        vertical ? MIN_VERTICAL_HEIGHT : MIN_HORIZONTAL_WIDTH,
        measured,
      );
      setToolbarMinimums((current) => {
        const key = vertical ? "vertical" : "horizontal";
        return current[key] === minimum ? current : { ...current, [key]: minimum };
      });
    };
    measureToolbar();
    const observer = new ResizeObserver(measureToolbar);
    observer.observe(toolbar);
    return () => observer.disconnect();
  }, [offsetLabel, settings.locked, vertical]);

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

  useLayoutEffect(() => {
    setWrapped(false);
    setFitScale(1);
    setMarqueeMetrics([]);
    lastRequestedSize.current = null;
  }, [fitLimits.height, fitLimits.width, primaryLineKey, supportingKey, style.fontSize, style.horizontalMaxWidth, style.layout, style.longText, style.orientation, style.romanizationFontScale, style.secondaryFontScale, style.translationFontScale, style.verticalMaxHeight]);

  useLayoutEffect(() => {
    if (!settings.visible) return;
    if (resizing) return;
    const layoutKey = `${style.layout}:${style.orientation}`;
    const layoutChanged = lastMeasuredLayoutKey.current !== layoutKey;
    if (layoutChanged) {
      lastMeasuredLayoutKey.current = layoutKey;
      lastRequestedSize.current = null;
      if (shrinkTimer.current !== null) clearTimeout(shrinkTimer.current);
      shrinkTimer.current = null;
      if (fitScale !== 1) {
        setFitScale(1);
        return;
      }
      if (wrapped) {
        setWrapped(false);
        return;
      }
      if (marqueeMetrics.length > 0) {
        setMarqueeMetrics([]);
        return;
      }
    }
    const lines = linesRef.current;
    const active = activeRef.current;
    if (!lines || !active) return;
    const supportingElements = supportingRefs.current.slice(0, supportingLines.length).filter((element): element is HTMLDivElement => Boolean(element));
    const elements = [active, ...supportingElements];
    const baseSizes = [style.fontSize, ...supportingLines.map((line) => line.baseSize)];
    const naturalItems = elements.map((element, index) => {
      const currentSize = Math.max(1, baseSizes[index] * fitScale);
      const ratio = baseSizes[index] / currentSize;
      return { width: element.scrollWidth * ratio, height: element.scrollHeight * ratio };
    });
    const natural = combinedContentSize(naturalItems, style.layout, style.orientation);
    const availableScreenWidth = Math.max(1, fitLimits.width - overlayHorizontalPadding);
    const availableScreenHeight = Math.max(1, fitLimits.height - overlayVerticalPadding);

    if (style.longText === "shrink") {
      const targetWidth = vertical ? availableScreenWidth : horizontalContentLimit;
      const targetHeight = vertical ? verticalContentLimit : availableScreenHeight;
      const minimumScale = Math.min(1, MIN_LYRIC_FONT_SIZE / Math.max(1, style.fontSize));
      const nextScale = Math.max(
        minimumScale,
        Math.min(1, targetWidth / Math.max(1, natural.width), targetHeight / Math.max(1, natural.height)),
      );
      if (Math.abs(nextScale - fitScale) > 0.005) {
        setFitScale(nextScale);
        return;
      }
    } else if (fitScale !== 1) {
      setFitScale(1);
      return;
    }

    const longAxisOverflow = vertical
      ? natural.height > verticalContentLimit + 1
      : natural.width > horizontalContentLimit + 1;
    if (style.longText === "wrap" && !wrapped && longAxisOverflow) {
      setWrapped(true);
      return;
    }
    if (style.longText === "marquee") {
      const nextMetrics = elements.map((element, index) => {
        const content = element.firstElementChild;
        const currentSize = Math.max(1, baseSizes[index] * fitScale);
        const ratio = baseSizes[index] / currentSize;
        const contentLength = content instanceof HTMLElement
          ? (vertical ? content.offsetHeight : content.offsetWidth) * ratio
          : 0;
        const naturalLength = contentLength > 0
          ? contentLength
          : vertical ? naturalItems[index].height : naturalItems[index].width;
        const limit = vertical ? marqueeVerticalLimit : marqueeHorizontalLimit;
        const distance = Math.max(0, naturalLength - limit);
        const preferredDuration = Math.max(
          DEFAULT_MARQUEE_DURATION_SECONDS,
          distance / MARQUEE_SPEED_PX_PER_SECOND,
        );
        return {
          overflowing: distance > 1,
          distance,
          duration: marqueeTimeLimit === null
            ? preferredDuration
            : Math.min(preferredDuration, marqueeTimeLimit),
        };
      });
      if (!sameMarqueeMetrics(marqueeMetrics, nextMetrics)) {
        setMarqueeMetrics(nextMetrics);
        return;
      }
    }

    const constrainedHorizontal = !vertical && constrained;
    const constrainedVertical = vertical && constrained;
    const measuredContentWidth = constrainedHorizontal
      ? horizontalContentLimit
      : Math.max(lines.clientWidth, Math.min(lines.scrollWidth, availableScreenWidth));
    const measuredContentHeight = constrainedVertical
      ? verticalContentLimit
      : Math.max(lines.clientHeight, Math.min(lines.scrollHeight, availableScreenHeight));
    const width = vertical
      ? Math.min(fitLimits.width, Math.max(190, Math.ceil(measuredContentWidth + overlayHorizontalPadding)))
      : horizontalWindowLimit;
    const height = vertical
      ? verticalWindowLimit
      : Math.min(fitLimits.height, Math.max(76, Math.ceil(measuredContentHeight + overlayVerticalPadding)));
    const previous = lastRequestedSize.current;
    if (previous && Math.abs(previous.width - width) <= 2 && Math.abs(previous.height - height) <= 2) return;
    const requestSize = (nextSize: { width: number; height: number }) => {
      if (fitFrame.current !== null) cancelAnimationFrame(fitFrame.current);
      fitFrame.current = requestAnimationFrame(() => {
        fitFrame.current = null;
        lastRequestedSize.current = nextSize;
        if (isTauriRuntime()) void api.fitOverlayContent(nextSize.width, nextSize.height);
      });
    };
    if (!previous) {
      requestSize({ width, height });
    } else {
      const immediate = { width: Math.max(previous.width, width), height: Math.max(previous.height, height) };
      if (immediate.width !== previous.width || immediate.height !== previous.height) requestSize(immediate);
      if (width < immediate.width || height < immediate.height) {
        if (shrinkTimer.current !== null) clearTimeout(shrinkTimer.current);
        shrinkTimer.current = setTimeout(() => {
          shrinkTimer.current = null;
          requestSize({ width, height });
        }, SHRINK_DELAY_MS);
      }
    }
    return () => {
      if (fitFrame.current !== null) cancelAnimationFrame(fitFrame.current);
      fitFrame.current = null;
      if (shrinkTimer.current !== null) clearTimeout(shrinkTimer.current);
      shrinkTimer.current = null;
    };
  }, [constrained, fitLimits.height, fitLimits.width, fitScale, horizontalContentLimit, horizontalWindowLimit, marqueeHorizontalLimit, marqueeMetrics, marqueeTimeLimit, marqueeVerticalLimit, overlayHorizontalPadding, overlayVerticalPadding, primaryText, resizing, settings.visible, style.fontSize, style.layout, style.longText, style.orientation, style.romanizationFontScale, style.secondaryFontScale, style.translationFontScale, supportingKey, vertical, verticalContentLimit, verticalWindowLimit, wrapped]);

  const resizeCoordinate = (event: Pick<React.PointerEvent<HTMLDivElement>, "screenX" | "screenY">, axis: "horizontal" | "vertical") =>
    axis === "horizontal" ? event.screenX : event.screenY;

  const requestedMainSize = (session: ActiveResizeSession) => {
    if (session.startMainSize === null) return null;
    const delta = session.latestCoordinate - session.startCoordinate;
    const direction = session.edge === "left" || session.edge === "top" ? -1 : 1;
    return session.startMainSize + delta * direction;
  };

  const commitResizeSession = (session: ActiveResizeSession) => {
    if (resizeSession.current !== session || session.committing || session.processing || session.pendingMainSize !== null || !session.ending) return;
    session.committing = true;
    const bounds = session.lastBounds;
    if (!bounds) {
      if (session.startMainSize === null) {
        session.committing = false;
        return;
      }
      clearResizeState();
      return;
    }
    const next = {
      ...styleRef.current,
      ...(session.axis === "horizontal"
        ? { horizontalMaxWidth: Math.max(session.minimumMainSize, bounds.width) }
        : { verticalMaxHeight: Math.max(session.minimumMainSize, bounds.height) }),
    };
    styleRef.current = next;
    setStyle(next);
    void api.setOverlayStyle(next).then((saved) => {
      styleRef.current = saved;
      setStyle(saved);
      if (resizeSession.current === session) clearResizeState();
    }).catch((error) => {
      reportFrontendError("保存桌面歌词尺寸失败", error);
      if (resizeSession.current === session) clearResizeState();
    });
  };

  const processResizeQueue = async (session: ActiveResizeSession) => {
    if (session.processing || resizeSession.current !== session) return;
    session.processing = true;
    try {
      while (resizeSession.current === session && session.pendingMainSize !== null) {
        const mainSize = session.pendingMainSize;
        session.pendingMainSize = null;
        const bounds = await api.resizeOverlayEdge(session.edge, mainSize, session.minimumMainSize);
        if (resizeSession.current !== session) return;
        session.lastBounds = bounds;
      }
    } catch (error) {
      if (resizeSession.current === session) {
        reportFrontendError("调整桌面歌词尺寸失败", error);
        clearResizeState();
      }
    } finally {
      session.processing = false;
      if (resizeSession.current === session) {
        if (session.pendingMainSize !== null) void processResizeQueue(session);
        else commitResizeSession(session);
      }
    }
  };

  const queueResize = (session: ActiveResizeSession) => {
    const mainSize = requestedMainSize(session);
    if (mainSize === null || resizeSession.current !== session) return;
    session.pendingMainSize = mainSize;
    void processResizeQueue(session);
  };

  const finishResizeSession = (session: ActiveResizeSession, coordinate?: number) => {
    if (resizeSession.current !== session || session.ending) return;
    if (coordinate !== undefined) session.latestCoordinate = coordinate;
    session.ending = true;
    queueResize(session);
    if (session.handle.hasPointerCapture(session.pointerId)) {
      try { session.handle.releasePointerCapture(session.pointerId); } catch { /* 已由系统释放 */ }
    }
    commitResizeSession(session);
  };

  finishResizeRef.current = () => {
    const session = resizeSession.current;
    if (session) finishResizeSession(session);
  };

  const beginResize = (
    edge: OverlayResizeEdge,
    axis: "horizontal" | "vertical",
  ) => (event: React.PointerEvent<HTMLDivElement>) => {
    if (settings.locked || event.button !== 0 || !isTauriRuntime()) return;
    event.preventDefault();
    event.stopPropagation();
    clearResizeState();
    const handle = event.currentTarget;
    const coordinate = resizeCoordinate(event, axis);
    const session: ActiveResizeSession = {
      pointerId: event.pointerId,
      edge,
      axis,
      handle,
      startCoordinate: coordinate,
      latestCoordinate: coordinate,
      startMainSize: null,
      minimumMainSize: axis === "horizontal" ? minimumHorizontalWidth : minimumVerticalHeight,
      pendingMainSize: null,
      lastBounds: null,
      processing: false,
      ending: false,
      committing: false,
    };
    resizeSession.current = session;
    setActiveResizeEdge(edge);
    handle.setPointerCapture(event.pointerId);
    const overlayWindow = getCurrentWindow();
    void Promise.all([overlayWindow.outerSize(), overlayWindow.scaleFactor()]).then(([size, scale]) => {
      if (resizeSession.current !== session) return;
      session.startMainSize = (axis === "horizontal" ? size.width : size.height) / scale;
      queueResize(session);
      commitResizeSession(session);
    }).catch((error) => {
      reportFrontendError("读取桌面歌词尺寸失败", error);
      if (resizeSession.current === session) clearResizeState();
    });
  };

  const continueResize = (event: React.PointerEvent<HTMLDivElement>) => {
    const session = resizeSession.current;
    if (!session || session.pointerId !== event.pointerId || session.ending) return;
    event.preventDefault();
    event.stopPropagation();
    session.latestCoordinate = resizeCoordinate(event, session.axis);
    queueResize(session);
  };

  const endResize = (event: React.PointerEvent<HTMLDivElement>) => {
    const session = resizeSession.current;
    if (!session || session.pointerId !== event.pointerId) return;
    event.preventDefault();
    event.stopPropagation();
    finishResizeSession(session, resizeCoordinate(event, session.axis));
  };

  const cancelResize = (event: React.PointerEvent<HTMLDivElement>) => {
    const session = resizeSession.current;
    if (!session || session.pointerId !== event.pointerId) return;
    event.preventDefault();
    event.stopPropagation();
    finishResizeSession(session);
  };

  const lostResizeCapture = (event: React.PointerEvent<HTMLDivElement>) => {
    const session = resizeSession.current;
    if (!session || session.pointerId !== event.pointerId || session.ending) return;
    finishResizeSession(session);
  };

  const toggleSupportingTrack = (kind: "translation" | "romanization") => {
    const translation = kind === "translation" ? !secondaryFlags.translation : secondaryFlags.translation;
    const romanization = kind === "romanization" ? !secondaryFlags.romanization : secondaryFlags.romanization;
    void updateStyle({ secondaryDisplay: secondaryDisplayFromFlags(translation, romanization) });
  };

  const supportingToggleTitle = (label: string, enabled: boolean, available: boolean) => {
    const action = enabled ? "关闭" : "显示";
    if (!supportsSecondary) return `${action}${label}（当前布局不显示副歌词）`;
    if (!available) return `${action}${label}（当前歌词无${label}，开启后暂显示下一句）`;
    return `${action}${label}`;
  };

  return (
    <main
      className={styles.overlay}
      data-alignment={effectiveAlignment}
      data-background={style.background}
      data-interactive={!settings.locked}
      data-layout={style.layout}
      data-orientation={style.orientation}
      data-long-text={style.longText}
      data-constrained={constrained}
      data-resizing={resizing}
      data-tauri-drag-region={settings.locked ? "false" : "deep"}
      style={{
        "--lyric-size": `${style.fontSize}px`,
        "--active-color": style.activeColor,
        "--inactive-color": style.inactiveColor,
        "--overlay-opacity": style.opacity,
        "--background-opacity": style.backgroundOpacity,
        "--solid-color": style.solidColor,
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
      <div className={styles.lines} ref={linesRef}>
        <div
          className={styles.active}
          data-empty={!primaryText}
          data-marquee={style.longText === "marquee" && marqueeMetrics[0]?.overflowing}
          ref={activeRef}
          style={{
            fontSize: `${style.fontSize * fitScale}px`,
            "--marquee-distance": `${marqueeMetrics[0]?.distance ?? 0}px`,
            "--marquee-duration": `${marqueeMetrics[0]?.duration ?? DEFAULT_MARQUEE_DURATION_SECONDS}s`,
          } as React.CSSProperties}
        >
          <KaraokeLine key={primaryLineKey} line={lyrics.currentLine} fallback={primaryText} positionMs={lyrics.adjustedPositionMs} style={style} />
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
              "--marquee-distance": `${marqueeMetrics[index + 1]?.distance ?? 0}px`,
              "--marquee-duration": `${marqueeMetrics[index + 1]?.duration ?? DEFAULT_MARQUEE_DURATION_SECONDS}s`,
            } as React.CSSProperties}
          ><span>{line.text}</span></div>
        ))}
      </div>

      {!settings.locked && (
        <>
          {vertical ? (
            <>
              <div className={styles.resizeHandle} data-active={activeResizeEdge === "top"} data-edge="top" data-tauri-drag-region="false" role="separator" aria-label="拖动设置竖排歌词高度" aria-orientation="horizontal" onLostPointerCapture={lostResizeCapture} onPointerCancel={cancelResize} onPointerDown={beginResize("top", "vertical")} onPointerMove={continueResize} onPointerUp={endResize} />
              <div className={styles.resizeHandle} data-active={activeResizeEdge === "bottom"} data-edge="bottom" data-tauri-drag-region="false" role="separator" aria-label="拖动设置竖排歌词高度" aria-orientation="horizontal" onLostPointerCapture={lostResizeCapture} onPointerCancel={cancelResize} onPointerDown={beginResize("bottom", "vertical")} onPointerMove={continueResize} onPointerUp={endResize} />
            </>
          ) : (
            <>
              <div className={styles.resizeHandle} data-active={activeResizeEdge === "left"} data-edge="left" data-tauri-drag-region="false" role="separator" aria-label="拖动设置横排歌词宽度" aria-orientation="vertical" onLostPointerCapture={lostResizeCapture} onPointerCancel={cancelResize} onPointerDown={beginResize("left", "horizontal")} onPointerMove={continueResize} onPointerUp={endResize} />
              <div className={styles.resizeHandle} data-active={activeResizeEdge === "right"} data-edge="right" data-tauri-drag-region="false" role="separator" aria-label="拖动设置横排歌词宽度" aria-orientation="vertical" onLostPointerCapture={lostResizeCapture} onPointerCancel={cancelResize} onPointerDown={beginResize("right", "horizontal")} onPointerMove={continueResize} onPointerUp={endResize} />
            </>
          )}
          <div className={styles.toolbar} data-tauri-drag-region="false" aria-label="桌面歌词工具栏" ref={toolbarRef}>
            <button aria-label="锁定并穿透鼠标" title="锁定并穿透鼠标" onClick={() => void api.setOverlayLocked(true)}><ToolbarIcon name="lock" /></button>
            <button aria-label="减小字号" title="减小字号" onClick={() => void updateStyle({ fontSize: style.fontSize - 2 })}><ToolbarIcon name="minus" /></button>
            <button aria-label="增大字号" title="增大字号" onClick={() => void updateStyle({ fontSize: style.fontSize + 2 })}><ToolbarIcon name="plus" /></button>
            <div className={styles.offsetControl} role="group" aria-label={`歌词偏移，当前${offsetAvailable ? formatOffsetMs(offsetMs) : "不可调整"}`}>
              <button
                aria-label="歌词延后 100 毫秒；按住 Shift 调整 500 毫秒"
                disabled={!offsetAvailable}
                title="歌词延后 100ms（按住 Shift 调整 500ms）"
                onClick={(event) => void lyrics.changeOffset(event.shiftKey ? -500 : -100)}
              ><ToolbarIcon name="offsetEarlier" /></button>
              <button
                className={styles.offsetValue}
                aria-label={!offsetAvailable
                  ? "当前歌曲没有可调整的同步歌词"
                  : offsetMs === 0
                    ? "当前歌词偏移为 0 毫秒"
                    : `当前歌词偏移${formatOffsetMs(offsetMs)}，点击重置`}
                disabled={!offsetAvailable || offsetMs === 0}
                title={offsetValueTitle}
                onClick={() => void lyrics.setOffset(0)}
              >{offsetLabel}</button>
              <button
                aria-label="歌词提前 100 毫秒；按住 Shift 调整 500 毫秒"
                disabled={!offsetAvailable}
                title="歌词提前 100ms（按住 Shift 调整 500ms）"
                onClick={(event) => void lyrics.changeOffset(event.shiftKey ? 500 : 100)}
              ><ToolbarIcon name="offsetLater" /></button>
            </div>
            <button aria-label={`切换单/双歌词，当前${layoutLabels[style.layout]}`} title={`切换单/双歌词（当前：${layoutLabels[style.layout]}）`} onClick={() => void updateStyle({
              layout: nextValue(style.layout, ["single", "double"] as const),
            })}><ToolbarIcon name="layout" /></button>
            <button aria-label={`切换横/竖排，当前${orientationLabels[style.orientation]}`} title={`切换横/竖排（当前：${orientationLabels[style.orientation]}）`} onClick={() => void updateStyle({
              orientation: nextValue(style.orientation, ["horizontal", "vertical"] as const),
            })}><ToolbarIcon name="orientation" /></button>
            <button
              className={styles.trackToggle}
              data-available={translationAvailable}
              data-on={secondaryFlags.translation}
              aria-label={supportingToggleTitle("翻译", secondaryFlags.translation, translationAvailable)}
              aria-pressed={secondaryFlags.translation}
              title={supportingToggleTitle("翻译", secondaryFlags.translation, translationAvailable)}
              onClick={() => toggleSupportingTrack("translation")}
            >文</button>
            <button
              className={styles.trackToggle}
              data-available={romanizationAvailable}
              data-on={secondaryFlags.romanization}
              aria-label={supportingToggleTitle("音译", secondaryFlags.romanization, romanizationAvailable)}
              aria-pressed={secondaryFlags.romanization}
              title={supportingToggleTitle("音译", secondaryFlags.romanization, romanizationAvailable)}
              onClick={() => toggleSupportingTrack("romanization")}
            >音</button>
            <button aria-label="隐藏桌面歌词" title="隐藏桌面歌词" onClick={() => void api.setOverlayVisible(false)}><ToolbarIcon name="hide" /></button>
            <button aria-label="打开桌面歌词设置" title="打开桌面歌词设置" onClick={() => void api.showMainWindow("settings")}><ToolbarIcon name="settings" /></button>
          </div>
        </>
      )}
    </main>
  );
}
