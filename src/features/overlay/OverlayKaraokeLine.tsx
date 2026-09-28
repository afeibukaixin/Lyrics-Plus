import { useMemo, useRef } from "react";
import type { LyricsLine, OverlayStyle } from "../../shared/types";
import { KaraokeWord, useKaraokeTimeline } from "../lyrics/KaraokeWord";
import styles from "./Overlay.module.scss";

const karaokeWordClasses = {
  word: styles.karaokeWord,
  base: styles.karaokeWordBase,
  fill: styles.karaokeWordFill,
  fillText: styles.karaokeWordFillText,
};

const karaokeGlowEffectClasses = {
  stack: styles.karaokeWordEffects,
  farGlow: styles.karaokeWordGlowFar,
  nearGlow: styles.karaokeWordGlowNear,
  glowText: styles.karaokeWordGlowText,
};

export function OverlayKaraokeLine({ line, fallback, positionMs, positionObservedAtMs, style, playing, fitScale }: {
  line: LyricsLine | null;
  fallback: string;
  positionMs: number;
  positionObservedAtMs: number;
  style: OverlayStyle;
  playing: boolean;
  fitScale: number;
}) {
  const text = line ? line.text : fallback;
  const words = useMemo(
    () => line?.words?.filter((word) => word.text.length > 0) ?? [],
    [line?.words],
  );
  const scopeRef = useRef<HTMLSpanElement>(null);
  useKaraokeTimeline({
    axis: style.orientation === "vertical" ? "y" : "x",
    effect: style.karaokeStyle === "glow" ? "glow-lift" : "sweep",
    enabled: style.karaokeStyle === "sweep" || style.karaokeStyle === "glow",
    lineStartMs: line?.startMs ?? 0,
    playing,
    positionMs,
    positionObservedAtMs,
    scopeRef,
    words,
    fontLayoutKey: `${style.fontFamily}:${style.fontFamilies}:${style.fontWeight}:${style.fontSize * fitScale}:${style.lineHeight}:${style.safetyInsetX}:${style.safetyInsetY}`,
  });

  if (words.length === 0) return <span>{text || "\u00a0"}</span>;
  return (
    <span ref={scopeRef} className={styles.karaokeText} data-karaoke={style.karaokeStyle}>
      {words.map((word, index) => {
        const duration = Math.max(0, word.endMs - word.startMs);
        const current = positionMs >= word.startMs && positionMs < word.endMs;
        const complete = positionMs >= word.endMs || (duration === 0 && positionMs >= word.startMs);
        return (
          <KaraokeWord
            axis={style.orientation === "vertical" ? "y" : "x"}
            classes={karaokeWordClasses}
            complete={complete}
            key={`${word.startMs}-${index}`}
            current={current}
            effectClasses={style.karaokeStyle === "glow" ? karaokeGlowEffectClasses : undefined}
            text={word.text}
          />
        );
      })}
    </span>
  );
}
