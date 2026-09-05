import { useCallback, useRef } from "react";
import { usePlaybackSpectrum } from "../player/usePlaybackSpectrum";
import type { PlaybackSpectrumBands } from "../../shared/types";

// 静止时频谱柱收缩到圆点中心，圆点本身由 SVG 底图稳定绘制。
const SPECTRUM_IDLE_SCALE = 0;
// 分散强低频，形成固定的非对称错序；第一项 bands[5] 保持第一柱的特殊增强。
const SPECTRUM_DISPLAY_ORDER = [5, 0, 2, 4, 1, 3] as const;
const SPECTRUM_BEZIER_X1 = 0.42;
const SPECTRUM_BEZIER_Y1 = 0;
const SPECTRUM_BEZIER_X2 = 1;
const SPECTRUM_BEZIER_Y2 = 1;
const SPECTRUM_BEZIER_ITERATIONS = 10;

type SpectrumMotion = {
  lines: SVGLineElement[];
};

function cubicBezierCoordinate(t: number, firstControl: number, secondControl: number) {
  const inverse = 1 - t;
  return 3 * inverse * inverse * t * firstControl
    + 3 * inverse * t * t * secondControl
    + t * t * t;
}

function spectrumHeightProgress(value: number) {
  const input = Math.max(0, Math.min(1, value));
  if (input === 0 || input === 1) return input;

  let lower = 0;
  let upper = 1;
  for (let iteration = 0; iteration < SPECTRUM_BEZIER_ITERATIONS; iteration += 1) {
    const middle = (lower + upper) / 2;
    if (cubicBezierCoordinate(middle, SPECTRUM_BEZIER_X1, SPECTRUM_BEZIER_X2) < input) {
      lower = middle;
    } else {
      upper = middle;
    }
  }
  return cubicBezierCoordinate(
    (lower + upper) / 2,
    SPECTRUM_BEZIER_Y1,
    SPECTRUM_BEZIER_Y2,
  );
}

export function useNotchSpectrum(enabled: boolean) {
  const spectrumNodesRef = useRef(new Map<SVGSVGElement, SpectrumMotion>());
  const registerSpectrumNode = useCallback((node: SVGSVGElement) => {
    const lines = Array.from(node.querySelectorAll<SVGLineElement>("[data-spectrum-line]"));
    lines.forEach((line) => {
      line.style.transform = `scaleY(${SPECTRUM_IDLE_SCALE})`;
    });
    const motion = { lines };
    spectrumNodesRef.current.set(node, motion);
    return () => {
      if (spectrumNodesRef.current.get(node) !== motion) return;
      lines.forEach((line) => {
        line.style.removeProperty("transform");
      });
      spectrumNodesRef.current.delete(node);
    };
  }, []);

  const paintSpectrum = useCallback((bands: PlaybackSpectrumBands) => {
    // 后端已经完成频段合并与响应处理，前端把 0..1 统一映射为柱高。
    for (const motion of spectrumNodesRef.current.values()) {
      motion.lines.forEach((line, index) => {
        const bandIndex = SPECTRUM_DISPLAY_ORDER[index] ?? index;
        const value = bands[bandIndex];
        const normalizedValue = Number.isFinite(value)
          ? Math.max(0, Math.min(1, value))
          : 0;
        const curvedValue = spectrumHeightProgress(normalizedValue);
        const level = SPECTRUM_IDLE_SCALE
          + (1 - SPECTRUM_IDLE_SCALE) * curvedValue;
        line.style.transform = `scaleY(${level})`;
      });
    }
  }, []);

  usePlaybackSpectrum(enabled, paintSpectrum);

  return { registerSpectrumNode };
}
