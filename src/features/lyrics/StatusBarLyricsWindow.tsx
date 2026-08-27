import { type CSSProperties } from "react";
import { useAppConfig } from "../config/AppConfigProvider";
import { usePlayback } from "../player/usePlayback";
import { useLyricsPresentation } from "./useLyricsPresentation";
import styles from "./StatusBarLyricsWindow.module.scss";

export default function StatusBarLyricsWindow() {
  const { config } = useAppConfig();
  const playback = usePlayback();
  const lyrics = useLyricsPresentation(playback.snapshot, playback.positionMs);
  const preferences = config.lyrics.displays.statusBar;
  const appearance = preferences.appearance;
  const value = lyrics.currentLine?.text?.trim()
    || (playback.snapshot.title ? `♪ ${playback.snapshot.title.trim()}` : "Lyrics Plus");

  return (
    <main
      className={styles.shell}
      data-alignment={appearance.alignment}
      style={{
        "--status-font-family": appearance.fontFamily,
        "--status-font-size": `${appearance.fontSize}px`,
        "--status-font-weight": appearance.fontWeight,
        "--status-text-color": appearance.textColor,
      } as CSSProperties}
      title={value}
    >
      <span>{value}</span>
    </main>
  );
}
