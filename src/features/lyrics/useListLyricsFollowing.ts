import { useCallback, useEffect, useRef, useState } from "react";

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
  const instantScrollPendingRef = useRef(true);
  const [following, setFollowing] = useState(true);

  useEffect(() => {
    instantScrollPendingRef.current = true;
    setFollowing(true);
  }, [trackKey]);

  useEffect(() => {
    instantScrollPendingRef.current = true;
    if (active) setFollowing(true);
  }, [active]);

  useEffect(() => {
    if (!active || !following || !activeRef.current) return;
    const instant = instantScrollPendingRef.current;
    activeRef.current.scrollIntoView({
      block: "center",
      behavior: instant || window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? "auto"
        : "smooth",
    });
    instantScrollPendingRef.current = false;
  }, [active, following, activeIndex, trackKey]);

  const pauseFollowing = useCallback(() => {
    if (hasLines) setFollowing(false);
  }, [hasLines]);

  const resumeFollowing = useCallback(() => {
    instantScrollPendingRef.current = false;
    setFollowing(true);
    requestAnimationFrame(() => activeRef.current?.scrollIntoView({
      block: "center",
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
    }));
  }, []);

  return {
    activeRef,
    following,
    pauseFollowing,
    resumeFollowing,
  };
}
