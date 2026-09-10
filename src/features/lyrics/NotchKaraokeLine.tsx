import { useMemo, useRef } from "react";
import type { CompactKaraokeStyle, LyricsLine } from "../../shared/types";
import { KaraokeWord, useKaraokeSweepTimeline } from "./KaraokeWord";
import styles from "./NotchLyricsWindow.module.scss";

const karaokeWordClasses = {
  word: styles.karaokeWord,
  base: styles.karaokeWordBase,
  fill: styles.karaokeWordFill,
  fillText: styles.karaokeWordFillText,
};

export function KaraokeLine({ line, positionMs, karaokeStyle, playing }: {
  line: LyricsLine;
  positionMs: number;
  karaokeStyle: CompactKaraokeStyle;
  playing: boolean;
}) {
  const words = useMemo(
    () => line.words?.filter((word) => word.text.length > 0) ?? [],
    [line.words],
  );
  const scopeRef = useRef<HTMLSpanElement>(null);
  useKaraokeSweepTimeline({
    enabled: karaokeStyle === "sweep",
    lineStartMs: line.startMs,
    playing,
    positionMs,
    scopeRef,
    words,
  });

  if (words.length === 0) return <span>{line.text}</span>;

  return (
    <span ref={scopeRef} className={styles.karaokeText} data-karaoke={karaokeStyle}>
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
            text={word.text}
          />
        );
      })}
    </span>
  );
}
