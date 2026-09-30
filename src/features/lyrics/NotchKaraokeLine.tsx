import { useMemo, useRef, type CSSProperties } from "react";
import type { LyricsLine, NotchKaraokeStyle } from "../../shared/types";
import { KaraokeWord, useKaraokeTimeline } from "./KaraokeWord";
import styles from "./NotchLyricsWindow.module.scss";

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

export function KaraokeLine({ line, positionMs, positionObservedAtMs, karaokeStyle, playing, fontSizePx }: {
  line: LyricsLine;
  positionMs: number;
  positionObservedAtMs: number;
  karaokeStyle: NotchKaraokeStyle;
  playing: boolean;
  fontSizePx: number;
}) {
  const words = useMemo(
    () => line.words?.filter((word) => word.text.length > 0) ?? [],
    [line.words],
  );
  const scopeRef = useRef<HTMLSpanElement>(null);
  // 列表歌词以 25px 为视觉基准；刘海屏按实际歌词字号缩小光晕和位移。
  const glowScale = Math.min(1, Math.max(0.45, fontSizePx / 25));
  useKaraokeTimeline({
    axis: "x",
    effect: karaokeStyle === "glow" ? "glow-lift" : "sweep",
    enabled: karaokeStyle === "sweep" || karaokeStyle === "glow",
    liftDistancePx: 3 * glowScale,
    lineStartMs: line.startMs,
    playing,
    positionMs,
    positionObservedAtMs,
    scopeRef,
    words,
  });

  if (words.length === 0) return <span>{line.text}</span>;

  return (
    <span ref={scopeRef} className={styles.karaokeText} data-karaoke={karaokeStyle} style={{ "--karaoke-glow-scale": glowScale } as CSSProperties}>
      {words.map((word, index) => {
        const duration = Math.max(0, word.endMs - word.startMs);
        const complete = positionMs >= word.endMs || (duration === 0 && positionMs >= word.startMs);
        const current = positionMs >= word.startMs && positionMs < word.endMs;
        return (
          <KaraokeWord
            axis="x"
            classes={karaokeWordClasses}
            complete={complete}
            key={`${word.startMs}-${index}`}
            current={current}
            effectClasses={karaokeStyle === "glow" ? karaokeGlowEffectClasses : undefined}
            text={word.text}
          />
        );
      })}
    </span>
  );
}
