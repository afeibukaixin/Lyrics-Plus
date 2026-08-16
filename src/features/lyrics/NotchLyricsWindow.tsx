import { useEffect, useState, type CSSProperties } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../../shared/api";
import { useAppConfig } from "../config/AppConfigProvider";
import { usePlayback } from "../player/usePlayback";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import { useLyricsPresentation } from "./useLyricsPresentation";
import styles from "./NotchLyricsWindow.module.scss";

export default function NotchLyricsWindow() {
  const { config } = useAppConfig();
  const playback = usePlayback();
  const lyrics = useLyricsPresentation(playback.snapshot, playback.positionMs);
  const [hovered, setHovered] = useState(false);
  const [hasNotch, setHasNotch] = useState(false);
  const notch = config.lyrics.displays.notch;
  const appearance = notch.appearance;
  const expanded = notch.expandedOnHover && hovered;
  const primary = lyrics.currentLine?.text || playback.snapshot.title || "Lyrics Plus";
  const secondary = lyrics.nextLine?.text || playback.snapshot.artist || "";

  useEffect(() => {
    void api.getNotchHasSafeArea().then(setHasNotch).catch(() => undefined);
    return createTauriListenerCleanup(
      listen<boolean>("notch://safe-area", ({ payload }) => setHasNotch(payload)),
    );
  }, []);

  return (
    <main
      className={styles.shell}
      data-expanded={expanded || undefined}
      data-has-notch={hasNotch || undefined}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        "--notch-font-family": appearance.fontFamily,
        "--notch-font-size": `${appearance.fontSize}px`,
        "--notch-font-weight": appearance.fontWeight,
        "--notch-opacity": appearance.backgroundOpacity,
        "--notch-active-color": appearance.activeColor,
        "--notch-secondary-color": appearance.secondaryColor,
        "--notch-background-color": appearance.backgroundColor,
        "--notch-background-blur": `${appearance.backgroundBlur}px`,
        "--notch-radius": `${appearance.borderRadius}px`,
        "--notch-max-width": `${appearance.maxWidth}px`,
      } as CSSProperties}
    >
      <section className={styles.island} aria-live="polite">
        <div className={styles.compactLine} key={`${lyrics.currentLine?.startMs ?? "fallback"}:${primary}`}>
          <span>{primary}</span>
        </div>
        <div className={styles.details} aria-hidden={!expanded}>
          <div className={styles.track}>
            <strong>{playback.snapshot.title ?? "Lyrics Plus"}</strong>
            <span>{playback.snapshot.artist ?? ""}</span>
          </div>
          <p>{secondary}</p>
        </div>
      </section>
    </main>
  );
}
