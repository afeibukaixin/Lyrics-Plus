import { type CSSProperties } from "react";
import { useAppConfig } from "../config/AppConfigProvider";
import { usePlayback } from "../player/usePlayback";
import { useCompactLyricsPresentation } from "./compactPresentation";
import styles from "./StatusBarLyricsWindow.module.scss";

export default function StatusBarLyricsWindow() {
  const { config } = useAppConfig();
  const playback = usePlayback();
  const preferences = config.lyrics.displays.statusBar;
  const lyrics = useCompactLyricsPresentation({
    snapshot: playback.snapshot,
    positionMs: playback.positionMs,
    active: playback.active,
    presentation: preferences.presentation,
    offsetErrorMessage: "Failed to update the menu bar lyrics offset",
  });
  const appearance = preferences.appearance;
  const primary = lyrics.primaryLine.text.trim() || "\u00a0";
  const secondary = preferences.presentation.layout === "double"
    ? lyrics.supportingLine?.text.trim() || "\u00a0"
    : null;
  const value = secondary ? `${primary} · ${secondary}` : primary;

  return (
    <main
      className={styles.shell}
      data-alignment={preferences.presentation.alignment}
      data-layout={preferences.presentation.layout}
      data-line-order={lyrics.doubleLineOrder}
      style={{
        "--status-font-family": appearance.fontFamily,
        "--status-font-size": `${appearance.fontSize}px`,
        "--status-vertical-offset": `${appearance.verticalOffset}px`,
        "--status-font-weight": appearance.fontWeight,
        "--status-text-color": appearance.textColor,
      } as CSSProperties}
      title={value}
    >
      <div className={styles.lines}>
        {lyrics.doubleLineOrder === "reversed" && secondary
          ? <><span className={styles.secondary}>{secondary}</span><span className={styles.primary}>{primary}</span></>
          : <><span className={styles.primary}>{primary}</span>{secondary && <span className={styles.secondary}>{secondary}</span>}</>}
      </div>
    </main>
  );
}
