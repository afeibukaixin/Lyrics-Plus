import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import type { TFunction } from "i18next";
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
import appIconUrl from "../../../src-tauri/icons/32x32.png";
import { IconButton } from "@/components/ui/icon-button";
import { Slider } from "@/components/ui/slider";
import { usePlayback } from "../player/usePlayback";
import type {
  CompactKaraokeStyle,
  LyricsLine,
  NotchLyricsPreferences,
  PlaybackSpectrumBands,
} from "../../shared/types";
import styles from "./NotchLyricsWindow.module.scss";

const LOOP_MARQUEE_SPEED_PX_PER_SECOND = 28;
const LOOP_MARQUEE_START_PAUSE_MS = 1_000;
const LOOP_MARQUEE_END_PAUSE_MS = 900;
const LOOP_MARQUEE_HOME_PAUSE_MS = 900;
const LYRIC_MARQUEE_SPEED_PX_PER_SECOND = 35;
const DEFAULT_LYRIC_MARQUEE_DURATION_MS = 4_000;
export const MIN_LYRIC_MARQUEE_DURATION_MS = 100;
const SEEK_SYNC_TOLERANCE_MS = 1_500;
const SEEK_SYNC_TIMEOUT_MS = 5_000;

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

export function SpectrumBars({ bands }: { bands: PlaybackSpectrumBands }) {
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

export function NotchLyricsQuickControls({
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

export function ExpandedPlayer({
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

export function OverflowText({ children, contentKey, paused, align = "left", behavior = "loop", maxDurationMs = null }: {
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

export function KaraokeLine({ line, positionMs, karaokeStyle }: { line: LyricsLine; positionMs: number; karaokeStyle: CompactKaraokeStyle }) {
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
