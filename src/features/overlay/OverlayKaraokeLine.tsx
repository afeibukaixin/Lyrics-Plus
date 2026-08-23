import type { CSSProperties } from "react";
import type { LyricsLine, OverlayStyle } from "../../shared/types";
import styles from "./Overlay.module.scss";

export function OverlayKaraokeLine({ line, fallback, positionMs, style }: {
  line: LyricsLine | null;
  fallback: string;
  positionMs: number;
  style: OverlayStyle;
}) {
  const text = line?.text || fallback;
  const words = line?.words?.filter((word) => word.text.length > 0) ?? [];
  if (words.length === 0) return <span>{text || "\u00a0"}</span>;
  return (
    <span className={styles.karaokeText} data-karaoke={style.karaokeStyle}>
      {words.map((word, index) => {
        const duration = Math.max(0, word.endMs - word.startMs);
        const progress = positionMs <= word.startMs
          ? 0
          : duration === 0 || positionMs >= word.endMs
            ? 100
            : ((positionMs - word.startMs) / duration) * 100;
        const current = positionMs >= word.startMs && positionMs < word.endMs;
        return (
          <span
            className={styles.karaokeWord}
            data-complete={positionMs >= word.endMs || (duration === 0 && positionMs >= word.startMs)}
            data-current={current}
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
