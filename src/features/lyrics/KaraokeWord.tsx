import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState, type RefObject } from "react";
import { useGSAP } from "@gsap/react";
import { gsap } from "gsap";
import type { LyricsWord } from "../../shared/types";

gsap.registerPlugin(useGSAP);

const KARAOKE_FILL_SELECTOR = "[data-karaoke-fill]";
const KARAOKE_RESYNC_THRESHOLD_SECONDS = 0.15;

const KARAOKE_CROSS_AXIS_BLEED = "-0.25em";

function karaokeClipPaths(axis: "x" | "y") {
  if (axis === "y") {
    return {
      initial: `inset(0% ${KARAOKE_CROSS_AXIS_BLEED} 100% ${KARAOKE_CROSS_AXIS_BLEED})`,
      complete: `inset(0% ${KARAOKE_CROSS_AXIS_BLEED} 0% ${KARAOKE_CROSS_AXIS_BLEED})`,
    };
  }

  return {
    initial: `inset(${KARAOKE_CROSS_AXIS_BLEED} 100% ${KARAOKE_CROSS_AXIS_BLEED} 0%)`,
    complete: `inset(${KARAOKE_CROSS_AXIS_BLEED} 0% ${KARAOKE_CROSS_AXIS_BLEED} 0%)`,
  };
}

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
  axis: "x" | "y";
  scopeRef: RefObject<HTMLElement | null>;
  lineStartMs: number;
  positionMs: number;
  positionObservedAtMs: number;
  words: readonly LyricsWord[];
  enabled: boolean;
  playing: boolean;
};

/**
 * 按歌词行创建一个稳定的扫光时间轴。时间轴只更新文字裁剪区域，不参与 React 的
 * 100ms 状态刷新；拖动或播放器校时产生较大偏差时才重新定位。
 */
export function useKaraokeSweepTimeline({
  axis,
  scopeRef,
  lineStartMs,
  positionMs,
  positionObservedAtMs,
  words,
  enabled,
  playing,
}: KaraokeSweepTimelineOptions) {
  const positionRef = useRef(positionMs);
  positionRef.current = positionMs;
  const timelineRef = useRef<gsap.core.Timeline | null>(null);
  const timelineOriginRef = useRef(lineStartMs);
  const playingRef = useRef(playing);
  const pausedPositionRef = useRef(positionMs);
  const resumeSnapshotRef = useRef<number | null>(null);
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
    // 只在扫光轴上裁切，交叉轴保留字形下伸部，避免 y/g/p/q 被切掉。
    const { initial: initialClipPath, complete: completeClipPath } = karaokeClipPaths(axis);
    const timelineOriginMs = Math.min(
      lineStartMs,
      ...words.map((word) => word.startMs),
    );
    timelineOriginRef.current = timelineOriginMs;

    fills.forEach((fill, index) => {
      gsap.set(fill, { clipPath: initialClipPath });
      const word = words[index];
      if (!word) return;
      const startSeconds = Math.max(0, (word.startMs - timelineOriginMs) / 1_000);
      const durationSeconds = Math.max(0, word.endMs - word.startMs) / 1_000;
      if (durationSeconds === 0) {
        timeline.set(fill, { clipPath: completeClipPath }, startSeconds);
        return;
      }
      timeline.fromTo(
        fill,
        { clipPath: initialClipPath },
        {
          clipPath: completeClipPath,
          duration: durationSeconds,
          ease: "none",
          immediateRender: false,
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
      fills.forEach((fill) => fill.style.removeProperty("clip-path"));
    };
  }, {
    dependencies: [axis, enabled, lineStartMs, scopeRef, wordsSignature],
    scope: scopeRef,
    revertOnUpdate: true,
  });

  useLayoutEffect(() => {
    const wasPlaying = playingRef.current;
    playingRef.current = playing;
    const timeline = timelineRef.current;
    if (!timeline) return;
    const desiredTime = getTimelineTime(
      positionMs,
      timelineOriginRef.current,
      timeline.duration(),
    );
    if (!playing) {
      // 普通暂停只冻结；展示层保留位置后，位置变化才代表拖动或歌词偏移调整。
      timeline.pause();
      if (!wasPlaying && positionMs !== pausedPositionRef.current) {
        timeline.time(desiredTime, false);
      }
      pausedPositionRef.current = positionMs;
      resumeSnapshotRef.current = null;
      return;
    }
    if (!wasPlaying) {
      // 原实例直接续播，不能拿恢复首帧的旧位置 seek，更不能把暂停时长补进来。
      resumeSnapshotRef.current = positionObservedAtMs;
      if (!reducedMotion && timeline.time() < timeline.duration()) timeline.play();
      return;
    }
    if (resumeSnapshotRef.current !== null) {
      if (positionObservedAtMs <= resumeSnapshotRef.current) return;
      resumeSnapshotRef.current = null;
    }
    if (reducedMotion) {
      timeline.pause(desiredTime);
      return;
    }
    if (Math.abs(timeline.time() - desiredTime) > KARAOKE_RESYNC_THRESHOLD_SECONDS) {
      timeline.time(desiredTime, false);
    }
    if (desiredTime < timeline.duration() && !timeline.isActive()) {
      timeline.play();
    }
  }, [lineStartMs, playing, positionMs, positionObservedAtMs, reducedMotion]);
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
