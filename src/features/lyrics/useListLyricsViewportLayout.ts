import { useLayoutEffect, type RefObject } from "react";

type UseListLyricsViewportLayoutOptions = {
  viewportRef: RefObject<HTMLDivElement | null>;
  linesRef: RefObject<HTMLDivElement | null>;
  activeRef: RefObject<HTMLDivElement | null>;
  following: boolean;
  visibleTopInset: number;
  layoutKey: string;
};

/**
 * 根据首末歌词的实际高度补齐滚动边界，并隐藏视口边缘未完整显示的歌词行。
 * 只写入 CSS 变量和 data 属性，滚动期间不会触发 React 重渲染。
 */
export function useListLyricsViewportLayout({
  viewportRef,
  linesRef,
  activeRef,
  following,
  visibleTopInset,
  layoutKey,
}: UseListLyricsViewportLayoutOptions) {
  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    const container = linesRef.current;
    if (!viewport || !container) return;

    const rows = Array.from(container.querySelectorAll<HTMLElement>("[data-lyrics-line='true']"));
    if (rows.length === 0) return;
    const firstRow = rows[0];
    const lastRow = rows[rows.length - 1];

    let centerFrame: number | null = null;
    const updateLayout = () => {
      const viewportHeight = viewport.clientHeight;
      const start = Math.max(0, (viewportHeight - firstRow.getBoundingClientRect().height) / 2);
      const end = Math.max(0, (viewportHeight - lastRow.getBoundingClientRect().height) / 2);
      container.style.setProperty("--list-edge-padding-start", `${start}px`);
      container.style.setProperty("--list-edge-padding-end", `${end}px`);

      if (following) {
        if (centerFrame !== null) cancelAnimationFrame(centerFrame);
        centerFrame = requestAnimationFrame(() => {
          const activeLine = activeRef.current;
          if (activeLine) {
            const viewportBounds = viewport.getBoundingClientRect();
            const lineBounds = activeLine.getBoundingClientRect();
            const target = viewport.scrollTop
              + lineBounds.top
              - viewportBounds.top
              - (viewport.clientHeight - lineBounds.height) / 2;
            const maximum = Math.max(0, viewport.scrollHeight - viewport.clientHeight);
            viewport.scrollTop = Math.min(
              maximum,
              Math.max(0, target),
            );
          }
          centerFrame = null;
        });
      }
    };

    const resizeObserver = new ResizeObserver(updateLayout);
    resizeObserver.observe(viewport);
    resizeObserver.observe(container);
    rows.forEach((row) => resizeObserver.observe(row));

    const visibilityObserver = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        const row = entry.target as HTMLElement;
        row.toggleAttribute("data-edge-clipped", entry.intersectionRatio < 0.999);
      });
    }, {
      root: viewport,
      rootMargin: `-${visibleTopInset}px 0px 0px 0px`,
      threshold: [0, 0.999, 1],
    });
    rows.forEach((row) => visibilityObserver.observe(row));
    updateLayout();

    return () => {
      resizeObserver.disconnect();
      visibilityObserver.disconnect();
      if (centerFrame !== null) cancelAnimationFrame(centerFrame);
      container.style.removeProperty("--list-edge-padding-start");
      container.style.removeProperty("--list-edge-padding-end");
      rows.forEach((row) => row.removeAttribute("data-edge-clipped"));
    };
  }, [activeRef, following, layoutKey, linesRef, viewportRef, visibleTopInset]);
}
