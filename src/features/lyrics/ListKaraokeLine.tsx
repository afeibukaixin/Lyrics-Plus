import { useMemo, useRef } from "react";
import type { ListLyricsKaraokeStyle, LyricsLine } from "../../shared/types";
import { KaraokeWord, useKaraokeTimeline } from "./KaraokeWord";
import styles from "./LyricsListWindow.module.scss";

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

type ListKaraokeLineProps = {
  line: LyricsLine;
  positionMs: number;
  positionObservedAtMs: number;
  playing: boolean;
  karaokeStyle: ListLyricsKaraokeStyle;
  fontLayoutKey: string;
};

/** 仅为当前列表歌词行创建逐词时间轴，避免非活动行持续占用动画资源。 */
export function ListKaraokeLine({
  line,
  positionMs,
  positionObservedAtMs,
  playing,
  karaokeStyle,
  fontLayoutKey,
}: ListKaraokeLineProps) {
  const words = useMemo(
    () => line.words?.filter((word) => word.text.length > 0) ?? [],
    [line.words],
  );
  const scopeRef = useRef<HTMLSpanElement>(null);

  useKaraokeTimeline({
    axis: "x",
    effect: karaokeStyle === "glow" ? "glow-lift" : "sweep",
    enabled: karaokeStyle !== "highlight" && words.length > 0,
    lineStartMs: line.startMs,
    playing,
    positionMs,
    positionObservedAtMs,
    scopeRef,
    words,
    fontLayoutKey,
  });

  // 纯高亮直接继承当前行颜色，不创建逐词绘制层。
  if (karaokeStyle === "highlight" || words.length === 0) return <span>{line.text}</span>;

  return (
    <span ref={scopeRef} className={styles.karaokeText} data-karaoke={karaokeStyle}>
      {words.map((word, index) => {
        const duration = Math.max(0, word.endMs - word.startMs);
        const current = positionMs >= word.startMs && positionMs < word.endMs;
        const complete = positionMs >= word.endMs || (duration === 0 && positionMs >= word.startMs);
        return (
          <KaraokeWord
            axis="x"
            classes={karaokeWordClasses}
            complete={complete}
            current={current}
            effectClasses={karaokeStyle === "glow" ? karaokeGlowEffectClasses : undefined}
            key={`${word.startMs}-${index}`}
            text={word.text}
          />
        );
      })}
    </span>
  );
}
