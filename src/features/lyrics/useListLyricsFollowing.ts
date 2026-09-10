import { useCallback, useEffect, useRef, useState } from "react";

type UseListLyricsFollowingOptions = {
  trackKey: string | null;
  activeIndex: number;
  hasLines: boolean;
};

export function useListLyricsFollowing({
  trackKey,
  activeIndex,
  hasLines,
}: UseListLyricsFollowingOptions) {
  const activeRef = useRef<HTMLDivElement>(null);
  const [following, setFollowing] = useState(true);

  useEffect(() => setFollowing(true), [trackKey]);

  useEffect(() => {
    if (!following || !activeRef.current) return;
    activeRef.current.scrollIntoView({
      block: "center",
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
    });
  }, [following, activeIndex]);

  const pauseFollowing = useCallback(() => {
    if (hasLines) setFollowing(false);
  }, [hasLines]);

  const resumeFollowing = useCallback(() => {
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
