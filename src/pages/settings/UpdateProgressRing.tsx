import { useRef } from "react";
import { useGSAP } from "@gsap/react";
import { gsap } from "gsap";
import styles from "../settings.module.scss";

gsap.registerPlugin(useGSAP);
function clampUpdateProgress(value: number) {
  return Math.max(0, Math.min(100, value));
}

export function UpdateProgressRing({ value }: { value: number }) {
  const initialValueRef = useRef(clampUpdateProgress(value));
  const rootRef = useRef<HTMLSpanElement>(null);
  const indicatorRef = useRef<SVGCircleElement>(null);
  const valueRef = useRef<HTMLSpanElement>(null);
  const progressStateRef = useRef({ value: initialValueRef.current });
  const progressTweenRef = useRef<gsap.core.Tween | null>(null);
  const reducedMotionRef = useRef(false);

  useGSAP(() => {
    const media = gsap.matchMedia();
    media.add({
      reduceMotion: "(prefers-reduced-motion: reduce)",
      allowMotion: "(prefers-reduced-motion: no-preference)",
    }, (context) => {
      reducedMotionRef.current = Boolean(context.conditions?.reduceMotion);
      if (reducedMotionRef.current && progressTweenRef.current) {
        const tween = progressTweenRef.current;
        tween.progress(1);
        tween.kill();
        progressTweenRef.current = null;
      }
    });
    return () => media.revert();
  }, { scope: rootRef });

  useGSAP(() => {
    const indicator = indicatorRef.current;
    const valueElement = valueRef.current;
    if (!indicator || !valueElement) return;

    const target = clampUpdateProgress(value);
    const renderProgress = () => {
      const progress = clampUpdateProgress(progressStateRef.current.value);
      indicator.style.strokeDashoffset = String(100 - progress);
      valueElement.textContent = `${Math.round(progress)}%`;
    };

    renderProgress();
    if (reducedMotionRef.current) {
      progressTweenRef.current?.kill();
      progressTweenRef.current = null;
      progressStateRef.current.value = target;
      renderProgress();
      return;
    }

    const tween = gsap.to(progressStateRef.current, {
      value: target,
      duration: 0.35,
      ease: "power1.out",
      overwrite: "auto",
      onUpdate: renderProgress,
      onComplete: () => {
        renderProgress();
        if (progressTweenRef.current === tween) progressTweenRef.current = null;
      },
    });
    progressTweenRef.current = tween;
    return () => {
      tween.kill();
      if (progressTweenRef.current === tween) progressTweenRef.current = null;
    };
  }, { dependencies: [value], scope: rootRef });

  return (
    <span ref={rootRef} className={styles.updateProgressVisual} aria-hidden="true">
      <svg className="size-full" viewBox="0 0 32 32" shapeRendering="geometricPrecision" focusable="false">
        <circle className={styles.updateProgressTrack} cx="16" cy="16" r="13" pathLength="100" strokeWidth="3" />
        <circle
          ref={indicatorRef}
          className={styles.updateProgressIndicator}
          cx="16"
          cy="16"
          r="13"
          pathLength="100"
          strokeDasharray="100"
          strokeDashoffset="100"
          strokeLinecap="butt"
          strokeWidth="3"
          transform="rotate(-90 16 16)"
        />
      </svg>
      <span ref={valueRef} className={styles.updateStatusValue}>{Math.round(initialValueRef.current)}%</span>
    </span>
  );
}
