import { useLayoutEffect, useRef, type ReactNode } from "react";
import styles from "./NotchLyricsWindow.module.scss";

const LOOP_MARQUEE_SPEED_PX_PER_SECOND = 28;
const LOOP_MARQUEE_START_PAUSE_MS = 1_000;
const LOOP_MARQUEE_END_PAUSE_MS = 900;
const LOOP_MARQUEE_HOME_PAUSE_MS = 900;
const LYRIC_MARQUEE_SPEED_PX_PER_SECOND = 35;
const DEFAULT_LYRIC_MARQUEE_DURATION_MS = 4_000;
export const MIN_LYRIC_MARQUEE_DURATION_MS = 100;

export function OverflowText({ children, contentKey, paused, align = "left", behavior = "loop", maxDurationMs = null }: {
  children: ReactNode;
  contentKey: string;
  paused: boolean;
  align?: "left" | "center" | "right";
  behavior?: "loop" | "once";
  maxDurationMs?: number | null;
}) {
  const viewportRef = useRef<HTMLSpanElement>(null);
  const contentRef = useRef<HTMLSpanElement>(null);
  const animationRef = useRef<Animation | null>(null);

  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    const content = contentRef.current;
    if (!viewport || !content) return;

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    let frame = 0;
    let resetForPause = false;

    const stopAtHome = () => {
      if (resetForPause) return;
      cancelAnimationFrame(frame);
      animationRef.current?.cancel();
      animationRef.current = null;
      content.style.transform = "translateX(0)";
      resetForPause = true;
    };

    const measure = () => {
      if (paused) {
        stopAtHome();
        return;
      }
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        animationRef.current?.cancel();
        animationRef.current = null;
        content.style.transform = "translateX(0)";

        const distance = Math.max(0, content.scrollWidth - viewport.clientWidth);
        if (distance <= 1 || reducedMotion.matches) return;

        if (behavior === "once") {
          const preferredDurationMs = Math.max(
            DEFAULT_LYRIC_MARQUEE_DURATION_MS,
            distance / LYRIC_MARQUEE_SPEED_PX_PER_SECOND * 1_000,
          );
          const duration = maxDurationMs === null
            ? preferredDurationMs
            : Math.min(preferredDurationMs, maxDurationMs);
          animationRef.current = content.animate([
            { transform: "translateX(0)", offset: 0 },
            { transform: "translateX(0)", offset: 0.12 },
            { transform: `translateX(-${distance}px)`, offset: 0.88 },
            { transform: `translateX(-${distance}px)`, offset: 1 },
          ], {
            duration,
            easing: "ease-in-out",
            fill: "forwards",
            iterations: 1,
          });
          return;
        }

        const travelMs = distance / LOOP_MARQUEE_SPEED_PX_PER_SECOND * 1_000;
        const totalMs = LOOP_MARQUEE_START_PAUSE_MS
          + travelMs
          + LOOP_MARQUEE_END_PAUSE_MS
          + travelMs
          + LOOP_MARQUEE_HOME_PAUSE_MS;
        const atStartEnd = LOOP_MARQUEE_START_PAUSE_MS / totalMs;
        const atFarEdge = (LOOP_MARQUEE_START_PAUSE_MS + travelMs) / totalMs;
        const leaveFarEdge = (LOOP_MARQUEE_START_PAUSE_MS + travelMs + LOOP_MARQUEE_END_PAUSE_MS) / totalMs;
        const arriveHome = (LOOP_MARQUEE_START_PAUSE_MS + travelMs + LOOP_MARQUEE_END_PAUSE_MS + travelMs) / totalMs;

        animationRef.current = content.animate([
          { transform: "translateX(0)", offset: 0 },
          { transform: "translateX(0)", offset: atStartEnd },
          { transform: `translateX(-${distance}px)`, offset: atFarEdge },
          { transform: `translateX(-${distance}px)`, offset: leaveFarEdge },
          { transform: "translateX(0)", offset: arriveHome },
          { transform: "translateX(0)", offset: 1 },
        ], {
          duration: totalMs,
          easing: "linear",
          iterations: Infinity,
        });
      });
    };

    const observer = new ResizeObserver(measure);
    observer.observe(viewport);
    observer.observe(content);
    reducedMotion.addEventListener("change", measure);
    measure();

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      reducedMotion.removeEventListener("change", measure);
      animationRef.current?.cancel();
      animationRef.current = null;
    };
  }, [behavior, contentKey, maxDurationMs, paused]);

  return (
    <span className={styles.overflowViewport} data-align={align} ref={viewportRef}>
      <span className={styles.overflowContent} ref={contentRef}>{children}</span>
    </span>
  );
}
