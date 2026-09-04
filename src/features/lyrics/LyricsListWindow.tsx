import { useMemo, type CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import type { LyricsLine } from "../../shared/types";
import { useAppConfig } from "../config/AppConfigProvider";
import { usePlayback } from "../player/usePlayback";
import { findAlignedAuxiliaryLine } from "./useLyrics";
import { LyricsListContent, type LyricsListAuxiliaryLine } from "./LyricsListContent";
import { LyricsListToolbar } from "./LyricsListToolbar";
import { useListLyricsFollowing } from "./useListLyricsFollowing";
import { useListLyricsOffset } from "./useListLyricsOffset";
import { useListLyricsToolbar } from "./useListLyricsToolbar";
import { useListLyricsWindow } from "./useListLyricsWindow";
import { useLyricsPresentation } from "./useLyricsPresentation";
import { cn } from "@/lib/utils";
import styles from "./LyricsListWindow.module.scss";

export default function LyricsListWindow() {
  const { t } = useTranslation();
  const { config, setLyricsDisplayPreferences, setListLyricsLocked } = useAppConfig();
  const playback = usePlayback();
  const lyrics = useLyricsPresentation(playback.snapshot, playback.positionMs, playback.active);
  const options = config.lyrics.displays.listWindow;
  const locked = options.locked;
  const appearance = options.appearance;
  const lines = lyrics.document?.tracks.original.lines ?? [];
  const offsetMs = lyrics.document?.offsetMs ?? 0;
  const transparentBackground = appearance.backgroundMode === "transparent";
  const translationAvailable = Boolean(lyrics.document?.tracks.translation);
  const romanizationAvailable = Boolean(lyrics.document?.tracks.romanization);

  const following = useListLyricsFollowing({
    trackKey: lyrics.trackKey,
    activeIndex: lyrics.activeIndex,
    hasLines: lines.length > 0,
  });
  const offset = useListLyricsOffset({
    trackKey: lyrics.trackKey,
    hasDocument: Boolean(lyrics.document),
    offsetMs,
  });
  const toolbar = useListLyricsToolbar({
    options,
    appearance,
    setLyricsDisplayPreferences,
    setListLyricsLocked,
  });
  const windowInteractions = useListLyricsWindow({ locked });

  const auxiliary = useMemo<LyricsListAuxiliaryLine[]>(() => lines.map((line: LyricsLine) => ({
    translation: options.showTranslation && lyrics.document?.tracks.translation
      ? findAlignedAuxiliaryLine(lyrics.document.tracks.translation.lines, line)
      : null,
    romanization: options.showRomanization && lyrics.document?.tracks.romanization
      ? findAlignedAuxiliaryLine(lyrics.document.tracks.romanization.lines, line)
      : null,
  })), [lines, lyrics.document, options.showRomanization, options.showTranslation]);

  const title = playback.snapshot.title ?? t("lyricsList.noTrack");
  const artist = playback.snapshot.artist ?? t("lyricsList.waiting");

  return (
    <main
      className={styles.shell}
      data-background-mode={appearance.backgroundMode}
      data-following={following.following}
      data-locked={locked}
      data-toolbar-visible={toolbar.toolbarVisible}
      onMouseEnter={toolbar.showToolbar}
      onMouseLeave={toolbar.scheduleToolbarHide}
      style={{
        "--list-font-family": appearance.fontFamily,
        "--list-font-size": `${appearance.fontSize}px`,
        "--list-font-weight": appearance.fontWeight,
        "--list-secondary-scale": appearance.secondaryFontScale,
        "--list-line-height": appearance.lineHeight,
        "--list-line-gap": `${appearance.lineGap}px`,
        "--list-secondary-line-gap": `${appearance.secondaryLineGap}px`,
        "--list-active-color": appearance.activeColor,
        "--list-inactive-color": appearance.inactiveColor,
        "--list-active-opacity": appearance.activeOpacity,
        "--list-inactive-opacity": appearance.inactiveOpacity,
        "--list-translation-color": appearance.translationColor,
        "--list-romanization-color": appearance.romanizationColor,
        "--list-active-background": appearance.activeBackgroundColor,
        "--list-background": appearance.backgroundColor,
        "--list-background-opacity": transparentBackground ? 0 : appearance.backgroundOpacity,
        "--list-alignment": appearance.alignment,
      } as CSSProperties}
    >
      <div className={styles.background} aria-hidden="true" />

      {transparentBackground && (
        <div className={styles.dragRegion} aria-hidden="true" onPointerDown={windowInteractions.startWindowDrag} />
      )}

      {!locked && (
        <header className={styles.header} onPointerDown={windowInteractions.startWindowDrag}>
          <div className={styles.track}>
            <h1>{title}</h1>
            <p>{artist}</p>
          </div>
          {lyrics.document && <span className={styles.source}>{lyrics.document.metadata.source}</span>}
        </header>
      )}

      {!locked && (
        <LyricsListToolbar
          t={t}
          options={options}
          appearance={appearance}
          offsetAvailable={offset.offsetAvailable}
          offsetMs={offset.offsetMs}
          translationAvailable={translationAvailable}
          romanizationAvailable={romanizationAvailable}
          updatePreferences={toolbar.updatePreferences}
          updateAppearance={toolbar.updateAppearance}
          updateLocked={toolbar.updateLocked}
          changeLyricsOffset={offset.changeLyricsOffset}
          setLyricsOffset={offset.setLyricsOffset}
          openStyleSettings={windowInteractions.openStyleSettings}
          resetWindowSize={windowInteractions.resetWindowSize}
          onFocusCapture={toolbar.showToolbar}
          onBlurCapture={toolbar.scheduleToolbarHide}
        />
      )}

      <LyricsListContent
        t={t}
        lines={lines}
        auxiliary={auxiliary}
        activeIndex={lyrics.activeIndex}
        activeRef={following.activeRef}
        following={following.following}
        onPauseFollowing={following.pauseFollowing}
        onResumeFollowing={following.resumeFollowing}
        status={lyrics.status}
        error={lyrics.error}
        canChooseLyrics={Boolean(playback.snapshot.title && playback.snapshot.artist)}
        onChooseLyrics={windowInteractions.openQuickLyrics}
      />

      {!locked && windowInteractions.resizeDirections.map(({ direction, className }) => (
        <div key={direction} className={cn(styles.resizeHandle, className)} aria-hidden="true" onPointerDown={windowInteractions.startResize(direction)} />
      ))}
    </main>
  );
}
