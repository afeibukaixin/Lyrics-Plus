import { useMemo, useRef } from "react";
import type { LyricsLine, OverlayStyle } from "../../shared/types";
import { KaraokeWord, useKaraokeSweepTimeline } from "../lyrics/KaraokeWord";
import styles from "./Overlay.module.scss";

const karaokeWordClasses = {
  word: styles.karaokeWord,
  base: styles.karaokeWordBase,
  fill: styles.karaokeWordFill,
  fillText: styles.karaokeWordFillText,
};

export function OverlayKaraokeLine({ line, fallback, positionMs, style, playing }: {
  line: LyricsLine | null;
  fallback: string;
  positionMs: number;
  style: OverlayStyle;
  playing: boolean;
}) {
  const text = line ? line.text : fallback;
  const words = useMemo(
    () => line?.words?.filter((word) => word.text.length > 0) ?? [],
    [line?.words],
  );
  const scopeRef = useRef<HTMLSpanElement>(null);
  useKaraokeSweepTimeline({
    axis: style.orientation === "vertical" ? "y" : "x",
    enabled: style.karaokeStyle === "sweep",
    lineStartMs: line?.startMs ?? 0,
    playing,
    positionMs,
    scopeRef,
    words,
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
            text={word.text}
          />
        );
      })}
    </span>
  );
}
