import { memo, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { useGSAP } from "@gsap/react";
import { gsap } from "gsap";
import type { LyricsWord } from "../../shared/types";

gsap.registerPlugin(useGSAP);

const KARAOKE_FILL_SELECTOR = "[data-karaoke-fill]";
const KARAOKE_RESYNC_THRESHOLD_SECONDS = 0.15;

export type KaraokeWordClasses = {
  word: string;
  base: string;
  fill: string;
  fillText: string;
};

type KaraokeWordProps = {
  text: string;
  current: boolean;
  complete: boolean;
  axis: "x" | "y";
  classes: KaraokeWordClasses;
};

type KaraokeSweepTimelineOptions = {
  scopeRef: RefObject<HTMLElement | null>;
  lineStartMs: number;
  positionMs: number;
  words: readonly LyricsWord[];
  enabled: boolean;
  playing: boolean;
};

/**
 * 按歌词行创建一个稳定的扫光时间轴。时间轴只更新 mask 进度，不参与 React 的
 * 100ms 状态刷新；拖动或播放器校时产生较大偏差时才重新定位。
 */
export function useKaraokeSweepTimeline({
  scopeRef,
  lineStartMs,
  positionMs,
  words,
  enabled,
  playing,
}: KaraokeSweepTimelineOptions) {
  const positionRef = useRef(positionMs);
  positionRef.current = positionMs;
  const timelineRef = useRef<gsap.core.Timeline | null>(null);
  const timelineOriginRef = useRef(lineStartMs);
  const [reducedMotion, setReducedMotion] = useReducedMotion();
  const wordsSignature = useMemo(
    () => words.map((word) => `${word.startMs}:${word.endMs}:${word.text}`).join("\u001f"),
    [words],
  );

  useEffect(() => {
    if (typeof window === "undefined" || !window.matchMedia) return;
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReducedMotion(media.matches);
    update();
    media.addEventListener?.("change", update);
    return () => media.removeEventListener?.("change", update);
  }, [setReducedMotion]);

  useGSAP(() => {
    const scope = scopeRef.current;
    if (!scope || !enabled || words.length === 0) {
      timelineRef.current = null;
      return;
    }

    const fills = Array.from(scope.querySelectorAll<HTMLElement>(KARAOKE_FILL_SELECTOR));
    const timeline = gsap.timeline({ paused: true });
    const timelineOriginMs = Math.min(
      lineStartMs,
      ...words.map((word) => word.startMs),
    );
    timelineOriginRef.current = timelineOriginMs;

    fills.forEach((fill, index) => {
      fill.style.setProperty("--karaoke-progress", "0%");
      const word = words[index];
      if (!word) return;
      const startSeconds = Math.max(0, (word.startMs - timelineOriginMs) / 1_000);
      const durationSeconds = Math.max(0, word.endMs - word.startMs) / 1_000;
      if (durationSeconds === 0) {
        timeline.set(fill, { "--karaoke-progress": "100%" }, startSeconds);
        return;
      }
      timeline.to(
        fill,
        {
          "--karaoke-progress": "100%",
          duration: durationSeconds,
          ease: "none",
        },
        startSeconds,
      );
    });

    timelineRef.current = timeline;
    const desiredTime = getTimelineTime(
      positionRef.current,
      timelineOriginMs,
      timeline.duration(),
    );
    timeline.time(desiredTime, false);
    if (playing && !reducedMotion && desiredTime < timeline.duration()) {
      timeline.play();
    }

    return () => {
      timelineRef.current = null;
      timelineOriginRef.current = lineStartMs;
      timeline.kill();
      fills.forEach((fill) => fill.style.removeProperty("--karaoke-progress"));
    };
  }, {
    dependencies: [enabled, lineStartMs, scopeRef, wordsSignature],
    scope: scopeRef,
    revertOnUpdate: true,
  });

  useEffect(() => {
    const timeline = timelineRef.current;
    if (!timeline) return;
    const desiredTime = getTimelineTime(
      positionMs,
      timelineOriginRef.current,
      timeline.duration(),
    );
    if (!playing || reducedMotion) {
      timeline.pause(desiredTime);
      return;
    }
    if (Math.abs(timeline.time() - desiredTime) > KARAOKE_RESYNC_THRESHOLD_SECONDS) {
      timeline.time(desiredTime, false);
    }
    if (desiredTime < timeline.duration() && !timeline.isActive()) {
      timeline.play();
    }
  }, [lineStartMs, playing, positionMs, reducedMotion]);
}

export const KaraokeWord = memo(function KaraokeWord({
  text,
  current,
  complete,
  axis,
  classes,
}: KaraokeWordProps) {
  return (
    <span
      className={classes.word}
      data-complete={complete}
      data-current={current}
      data-karaoke-axis={axis}
    >
      <span className={classes.base}>{text}</span>
      <span
        aria-hidden="true"
        className={classes.fill}
        data-karaoke-axis={axis}
        data-karaoke-fill="true"
      >
        <span className={classes.fillText}>{text}</span>
      </span>
    </span>
  );
});

function getTimelineTime(positionMs: number, originMs: number, durationSeconds: number) {
  return Math.min(durationSeconds, Math.max(0, (positionMs - originMs) / 1_000));
}

function useReducedMotion() {
  const [reducedMotion, setReducedMotion] = useState(false);
  return [reducedMotion, setReducedMotion] as const;
}
