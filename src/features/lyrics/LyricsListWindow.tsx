import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { ArrowDownToLine, Music2, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api } from "../../shared/api";
import type { LyricsLine } from "../../shared/types";
import { useAppConfig } from "../config/AppConfigProvider";
import { usePlayback } from "../player/usePlayback";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { findAlignedAuxiliaryLine } from "./useLyrics";
import { useLyricsPresentation } from "./useLyricsPresentation";
import styles from "./LyricsListWindow.module.scss";

export default function LyricsListWindow() {
  const { t } = useTranslation();
  const { config } = useAppConfig();
  const playback = usePlayback();
  const lyrics = useLyricsPresentation(playback.snapshot, playback.positionMs);
  const viewportRef = useRef<HTMLDivElement>(null);
  const activeRef = useRef<HTMLDivElement>(null);
  const [following, setFollowing] = useState(true);
  const options = config.lyrics.displays.listWindow;
  const appearance = options.appearance;
  const lines = lyrics.document?.tracks.original.lines ?? [];

  useEffect(() => setFollowing(true), [lyrics.trackKey]);

  useEffect(() => {
    if (!following || !activeRef.current) return;
    activeRef.current.scrollIntoView({
      block: "center",
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
    });
  }, [following, lyrics.activeIndex]);

  const auxiliary = useMemo(() => lines.map((line) => ({
    translation: options.showTranslation && lyrics.document?.tracks.translation
      ? findAlignedAuxiliaryLine(lyrics.document.tracks.translation.lines, line)
      : null,
    romanization: options.showRomanization && lyrics.document?.tracks.romanization
      ? findAlignedAuxiliaryLine(lyrics.document.tracks.romanization.lines, line)
      : null,
  })), [lines, lyrics.document, options.showRomanization, options.showTranslation]);

  const pauseFollowing = () => {
    if (lines.length > 0) setFollowing(false);
  };
  const resumeFollowing = () => {
    setFollowing(true);
    requestAnimationFrame(() => activeRef.current?.scrollIntoView({ block: "center", behavior: "smooth" }));
  };

  const title = playback.snapshot.title ?? t("lyricsList.noTrack");
  const artist = playback.snapshot.artist ?? t("lyricsList.waiting");

  return (
    <main className={styles.shell} style={{
      "--list-font-family": appearance.fontFamily,
      "--list-font-size": `${appearance.fontSize}px`,
      "--list-font-weight": appearance.fontWeight,
      "--list-secondary-scale": appearance.secondaryFontScale,
      "--list-line-height": appearance.lineHeight,
      "--list-line-gap": `${appearance.lineGap}px`,
      "--list-active-color": appearance.activeColor,
      "--list-inactive-color": appearance.inactiveColor,
      "--list-translation-color": appearance.translationColor,
      "--list-romanization-color": appearance.romanizationColor,
      "--list-active-background": appearance.activeBackgroundColor,
      "--list-background": appearance.backgroundColor,
      "--list-alignment": appearance.alignment,
    } as CSSProperties}>
      <header className={styles.header}>
        <div className={styles.track}>
          <h1 className="truncate text-xl font-semibold">{title}</h1>
          <p className="truncate text-sm text-muted-foreground">{artist}</p>
        </div>
        {lyrics.document && <span className="text-xs text-muted-foreground">{lyrics.document.metadata.source}</span>}
      </header>

      {lines.length > 0 ? (
        <div className={styles.workspace}>
          <ScrollArea
            className={styles.scroller}
            viewportRef={viewportRef}
            onWheel={pauseFollowing}
            onPointerDown={pauseFollowing}
          >
            <div className={styles.lines} role="list" aria-label={t("lyricsList.lyrics")}>
              {lines.map((line: LyricsLine, index) => {
                const active = index === lyrics.activeIndex;
                const supporting = auxiliary[index];
                return (
                  <div
                    className={cn(styles.line, active && styles.activeLine)}
                    data-active={active || undefined}
                    key={`${line.startMs}:${index}`}
                    ref={active ? activeRef : undefined}
                    role="listitem"
                    aria-current={active ? "true" : undefined}
                  >
                    <p>{line.text || "\u00a0"}</p>
                    {supporting?.translation && <small data-kind="translation">{supporting.translation.text}</small>}
                    {supporting?.romanization && <small data-kind="romanization">{supporting.romanization.text}</small>}
                  </div>
                );
              })}
            </div>
          </ScrollArea>
          {!following && (
            <Button className={styles.followButton} variant="secondary" size="sm" onClick={resumeFollowing}>
              <ArrowDownToLine data-icon="inline-start" />{t("lyricsList.returnCurrent")}
            </Button>
          )}
        </div>
      ) : (
        <Empty className={styles.empty}>
          <EmptyHeader>
            <EmptyMedia variant="icon"><Music2 /></EmptyMedia>
            <EmptyTitle>{lyrics.status === "loading" ? t("lyricsList.loading") : t("lyricsList.empty")}</EmptyTitle>
            <EmptyDescription>{lyrics.error ?? t("lyricsList.emptyHint")}</EmptyDescription>
          </EmptyHeader>
          {playback.snapshot.title && playback.snapshot.artist && (
            <Button variant="outline" onClick={() => void api.showQuickLyricsWindow()}>
              <Search data-icon="inline-start" />{t("lyricsList.chooseLyrics")}
            </Button>
          )}
        </Empty>
      )}
    </main>
  );
}
