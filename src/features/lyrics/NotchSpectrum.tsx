import { useEffect, useId, useRef } from "react";
import styles from "./NotchLyricsWindow.module.scss";

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
      viewBox="0 0 28 28"
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
      {Array.from({ length: 6 }, (_, index) => {
        const x = 1.5 + index * 5;
        const gradient = `url(#${gradientId}-${index < 2 ? "left" : index < 4 ? "center" : "right"})`;
        return (
          <g key={index}>
            <circle cx={x} cy="14" fill={gradient} r="1.5" />
            <line
              className={styles.spectrumBar}
              data-spectrum-line="true"
              stroke={gradient}
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
