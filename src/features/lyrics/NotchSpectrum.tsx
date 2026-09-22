import { useEffect, useId, useRef } from "react";
import styles from "./NotchLyricsWindow.module.scss";

const SPECTRUM_SIZE = 28;
const SPECTRUM_BAR_COUNT = 6;
const SPECTRUM_BAR_WIDTH = 3;
const SPECTRUM_GAP_RATIO = 0.618;
// 以柱宽为基准，用 0.618 计算柱间距并将整组频谱水平居中。
const SPECTRUM_BAR_GAP = SPECTRUM_BAR_WIDTH * SPECTRUM_GAP_RATIO;
const SPECTRUM_BAR_STEP = SPECTRUM_BAR_WIDTH + SPECTRUM_BAR_GAP;
const SPECTRUM_CONTENT_WIDTH = SPECTRUM_BAR_COUNT * SPECTRUM_BAR_WIDTH
  + (SPECTRUM_BAR_COUNT - 1) * SPECTRUM_BAR_GAP;
const SPECTRUM_START_X = (SPECTRUM_SIZE - SPECTRUM_CONTENT_WIDTH) / 2
  + SPECTRUM_BAR_WIDTH / 2;

export function SpectrumBars({
  active,
  register,
}: {
  active: boolean;
  register: (node: SVGSVGElement) => () => void;
}) {
  const gradientId = `spectrum-gradient-${useId().replace(/:/g, "")}`;
  const rootRef = useRef<SVGSVGElement>(null);
  useEffect(() => {
    const node = rootRef.current;
    if (!node) return;
    return register(node);
  }, [register]);

  return (
    <svg
      aria-hidden="true"
      className={styles.spectrum}
      data-active={active || undefined}
      focusable="false"
      preserveAspectRatio="xMidYMid meet"
      ref={rootRef}
      viewBox={`0 0 ${SPECTRUM_SIZE} ${SPECTRUM_SIZE}`}
      xmlns="http://www.w3.org/2000/svg"
    >
      <defs>
        {(["left", "center", "right"] as const).map((column) => (
          <linearGradient
            gradientUnits="userSpaceOnUse"
            id={`${gradientId}-${column}`}
            key={column}
            x1="0"
            x2="0"
            y1="25"
            y2="3"
          >
            <stop offset="0%" stopColor={`var(--notch-spectrum-${column}-bottom-color, currentColor)`} />
            <stop offset="50%" stopColor={`var(--notch-spectrum-${column}-middle-color, currentColor)`} />
            <stop offset="100%" stopColor={`var(--notch-spectrum-${column}-top-color, currentColor)`} />
          </linearGradient>
        ))}
      </defs>
      {Array.from({ length: SPECTRUM_BAR_COUNT }, (_, index) => {
        const x = SPECTRUM_START_X + index * SPECTRUM_BAR_STEP;
        const gradient = `url(#${gradientId}-${index < 2 ? "left" : index < 4 ? "center" : "right"})`;
        return (
          <g key={index}>
            <circle cx={x} cy="14" fill={gradient} r={SPECTRUM_BAR_WIDTH / 2} />
            <line
              className={styles.spectrumBar}
              data-spectrum-line="true"
              stroke={gradient}
              strokeWidth={SPECTRUM_BAR_WIDTH}
              x1={x}
              x2={x}
              y1="3"
              y2="25"
            />
          </g>
        );
      })}
    </svg>
  );
}
