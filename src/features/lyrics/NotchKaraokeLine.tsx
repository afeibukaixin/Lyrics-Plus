import type { CSSProperties } from "react";
import type { CompactKaraokeStyle, LyricsLine } from "../../shared/types";
import styles from "./NotchLyricsWindow.module.scss";

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
            data-complete={positionMs >= word.endMs || (duration === 0 && positionMs >= word.startMs)}
            data-current={positionMs >= word.startMs && positionMs < word.endMs}
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
