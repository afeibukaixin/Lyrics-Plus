import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState, type RefObject } from "react";
import { useGSAP } from "@gsap/react";
import { gsap } from "gsap";
import type { LyricsWord } from "../../shared/types";

gsap.registerPlugin(useGSAP);

const KARAOKE_FILL_SELECTOR = "[data-karaoke-fill]";
const KARAOKE_EFFECT_STACK_SELECTOR = "[data-karaoke-effect-stack]";
const KARAOKE_WORD_SELECTOR = "[data-karaoke-word]";
const KARAOKE_RESYNC_THRESHOLD_SECONDS = 0.15;

// 重要：该值参与 GSAP 逐帧插值，必须与 inset 的其它值统一使用百分比。
// 不要改回 -0.25em；macOS WebKit 在混合单位 clip-path 扫光下会持续累积渲染内存。
// -25% 是交叉轴余量，用于保留下伸字母（如 y/g/p/q）。
const KARAOKE_CROSS_AXIS_BLEED = "-25%";

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

type KaraokeWordEffectClasses = {
  stack: string;
  farGlow: string;
  nearGlow: string;
  glowText: string;
};

type KaraokeWordProps = {
  text: string;
  current: boolean;
  complete: boolean;
  axis: "x" | "y";
  classes: KaraokeWordClasses;
  effectClasses?: KaraokeWordEffectClasses;
};

type KaraokeTimelineOptions = {
  axis: "x" | "y";
  scopeRef: RefObject<HTMLElement | null>;
  lineStartMs: number;
  positionMs: number;
  positionObservedAtMs: number;
  words: readonly LyricsWord[];
  enabled: boolean;
  playing: boolean;
  fontLayoutKey?: string;
  effect?: KaraokeTimelineEffect;
  liftDistancePx?: number;
};

export type KaraokeTimelineEffect = "sweep" | "glow-lift";

/**
 * 按歌词行创建一个稳定的卡拉 OK 时间轴。时间轴直接推进扫光裁剪或泛光进度变量，
 * 不参与 React 的 100ms 状态刷新；拖动或播放器校时产生较大偏差时才重新定位。
 */
export function useKaraokeTimeline({
  axis,
  scopeRef,
  lineStartMs,
  positionMs,
  positionObservedAtMs,
  words,
  enabled,
  playing,
  fontLayoutKey = "",
  effect = "sweep",
  liftDistancePx = 3,
}: KaraokeTimelineOptions) {
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
    const effectStacks = Array.from(scope.querySelectorAll<HTMLElement>(KARAOKE_EFFECT_STACK_SELECTOR));
    const wordElements = Array.from(scope.querySelectorAll<HTMLElement>(KARAOKE_WORD_SELECTOR));
    const timeline = gsap.timeline({ paused: true });
    const timelineOriginMs = Math.min(
      lineStartMs,
      ...words.map((word) => word.startMs),
    );
    timelineOriginRef.current = timelineOriginMs;

    if (effect === "glow-lift") {
      wordElements.forEach((wordElement, index) => {
        const word = words[index];
        if (!word) return;
        const effectStack = effectStacks[index];
        const startSeconds = Math.max(0, (word.startMs - timelineOriginMs) / 1_000);
        const nextWordStartMs = words[index + 1]?.startMs;
        // 重叠逐字时间以后一字的开始时间为视觉边界，避免两段泛光同时推进。
        const visualEndMs = nextWordStartMs === undefined
          ? word.endMs
          : Math.min(word.endMs, nextWordStartMs);
        const durationSeconds = Math.max(0, visualEndMs - word.startMs) / 1_000;
        gsap.set(wordElement, { "--karaoke-progress": "0%", y: 0 });
        if (effectStack) {
          gsap.set(effectStack, { autoAlpha: 0 });
          timeline.set(effectStack, { autoAlpha: 1 }, startSeconds);
        }
        if (durationSeconds === 0) {
          timeline.set(wordElement, {
            "--karaoke-progress": "100%",
            ...(!reducedMotion && { y: -liftDistancePx }),
          }, startSeconds);
          return;
        }
        timeline.fromTo(
          wordElement,
          { "--karaoke-progress": "0%" },
          {
            "--karaoke-progress": "100%",
            duration: durationSeconds,
            ease: "none",
            immediateRender: false,
          },
          startSeconds,
        );
        if (!reducedMotion) {
          timeline.fromTo(
            wordElement,
            { y: 0 },
            {
              y: -liftDistancePx,
              duration: durationSeconds,
              ease: "power1.out",
              immediateRender: false,
            },
            startSeconds,
          );
        }
      });
    } else {
      // 普通扫光继续只在播放轴上裁切，保持其它歌词窗口的现有行为。
      const { initial: initialClipPath, complete: completeClipPath } = karaokeClipPaths(axis);
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
    }

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
    dependencies: [axis, effect, enabled, lineStartMs, scopeRef, wordsSignature, fontLayoutKey, liftDistancePx, reducedMotion],
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
  effectClasses,
}: KaraokeWordProps) {
  return (
    <span
      className={classes.word}
      data-complete={complete}
      data-current={current}
      data-karaoke-axis={axis}
      data-karaoke-word="true"
    >
      <span className={classes.base}>{text}</span>
      {effectClasses ? (
        <span
          aria-hidden="true"
          className={effectClasses.stack}
          data-karaoke-effect-stack="true"
        >
          <span className={effectClasses.farGlow}>
            <span className={effectClasses.glowText}>{text}</span>
          </span>
          <span className={effectClasses.nearGlow}>
            <span className={effectClasses.glowText}>{text}</span>
          </span>
          <span
            className={classes.fill}
            data-karaoke-axis={axis}
            data-karaoke-fill="true"
          >
            <span className={classes.fillText}>{text}</span>
          </span>
        </span>
      ) : (
        <span
          aria-hidden="true"
          className={classes.fill}
          data-karaoke-axis={axis}
          data-karaoke-fill="true"
        >
          <span className={classes.fillText}>{text}</span>
        </span>
      )}
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
