import { gsap } from "gsap";
import { useCallback, useLayoutEffect, useRef, useState } from "react";

const LIST_SCROLL_DURATION_SECONDS = 0.6;

function centeredScrollTop(viewport: HTMLElement, activeLine: HTMLElement) {
  const viewportBounds = viewport.getBoundingClientRect();
  const lineBounds = activeLine.getBoundingClientRect();
  const target = viewport.scrollTop
    + lineBounds.top
    - viewportBounds.top
    - (viewport.clientHeight - lineBounds.height) / 2;
  const maximum = Math.max(0, viewport.scrollHeight - viewport.clientHeight);
  return Math.min(maximum, Math.max(0, target));
}

type UseListLyricsFollowingOptions = {
  active: boolean;
  trackKey: string | null;
  activeIndex: number;
  hasLines: boolean;
};

export function useListLyricsFollowing({
  active,
  trackKey,
  activeIndex,
  hasLines,
}: UseListLyricsFollowingOptions) {
  const activeRef = useRef<HTMLDivElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const scrollTweenRef = useRef<gsap.core.Tween | null>(null);
  const resumeFrameRef = useRef<number | null>(null);
  const instantScrollPendingRef = useRef(true);
  const followingRef = useRef(true);
  const [following, setFollowing] = useState(true);

  const cancelScroll = useCallback(() => {
    scrollTweenRef.current?.kill();
    scrollTweenRef.current = null;
    if (resumeFrameRef.current !== null) {
      cancelAnimationFrame(resumeFrameRef.current);
      resumeFrameRef.current = null;
    }
  }, []);

  const scrollToActive = useCallback((instant: boolean) => {
    const viewport = viewportRef.current;
    const activeLine = activeRef.current;
    if (!viewport || !activeLine) return;
    cancelScroll();
    const scrollTop = centeredScrollTop(viewport, activeLine);
    if (instant || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      viewport.scrollTop = scrollTop;
      return;
    }
    scrollTweenRef.current = gsap.to(viewport, {
      scrollTop,
      duration: LIST_SCROLL_DURATION_SECONDS,
      ease: "power2.out",
      overwrite: "auto",
      onComplete: () => {
        scrollTweenRef.current = null;
      },
    });
  }, [cancelScroll]);

  useLayoutEffect(() => {
    cancelScroll();
    instantScrollPendingRef.current = true;
    followingRef.current = true;
    setFollowing(true);
  }, [cancelScroll, trackKey]);

  useLayoutEffect(() => {
    instantScrollPendingRef.current = true;
    if (active) {
      followingRef.current = true;
      setFollowing(true);
    } else {
      cancelScroll();
    }
  }, [active, cancelScroll]);

  useLayoutEffect(() => {
    if (!active || !following || !activeRef.current) return;
    const instant = instantScrollPendingRef.current;
    scrollToActive(instant);
    instantScrollPendingRef.current = false;
  }, [active, following, activeIndex, scrollToActive, trackKey]);

  useLayoutEffect(() => cancelScroll, [cancelScroll]);

  const pauseFollowing = useCallback(() => {
    cancelScroll();
    if (!hasLines) return;
    followingRef.current = false;
    setFollowing(false);
  }, [cancelScroll, hasLines]);

  const resumeFollowing = useCallback(() => {
    instantScrollPendingRef.current = false;
    const alreadyFollowing = followingRef.current;
    followingRef.current = true;
    setFollowing(true);
    if (alreadyFollowing) {
      cancelScroll();
      resumeFrameRef.current = requestAnimationFrame(() => {
        resumeFrameRef.current = null;
        scrollToActive(false);
      });
    }
  }, [cancelScroll, scrollToActive]);

  return {
    activeRef,
    viewportRef,
    following,
    pauseFollowing,
    resumeFollowing,
  };
}
