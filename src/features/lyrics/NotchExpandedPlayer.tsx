import { useEffect, useState, type ReactNode } from "react";
import type { TFunction } from "i18next";
import { Pause, Play, SkipBack, SkipForward } from "lucide-react";
import appIconUrl from "../../../src-tauri/icons/128x128@2x.png";
import { IconButton } from "@/components/ui/icon-button";
import { Slider } from "@/components/ui/slider";
import { usePlayback } from "../player/usePlayback";
import type { CompactKaraokeStyle, LyricsLine } from "../../shared/types";
import { ArtworkTransitionImage } from "./NotchArtwork";
import { KaraokeLine } from "./NotchKaraokeLine";
import { OverflowText } from "./NotchMarquee";
import styles from "./NotchLyricsWindow.module.scss";

const SEEK_SYNC_TOLERANCE_MS = 1_500;
const SEEK_SYNC_TIMEOUT_MS = 5_000;

function formatPlaybackTime(valueMs: number | null) {
  const totalSeconds = Math.max(0, Math.floor((valueMs ?? 0) / 1_000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return `${minutes}:${seconds}`;
}

type PlaybackController = ReturnType<typeof usePlayback>;

export function ExpandedPlayer({
  karaokeStyle,
  playback,
  previewLine,
  previewMaxDurationMs,
  previewPositionMs,
  quickControls,
  marqueePaused,
  t,
}: {
  karaokeStyle: CompactKaraokeStyle;
  playback: PlaybackController;
  previewLine: LyricsLine | null;
  previewMaxDurationMs: number | null;
  previewPositionMs: number;
  quickControls: ReactNode;
  marqueePaused: boolean;
  t: TFunction;
}) {
  const trackKey = playback.snapshot.trackId ?? "fallback";
  const title = playback.snapshot.title ?? "Lyrics Plus";
  const artist = playback.snapshot.artist ?? "";
  const durationMs = playback.snapshot.durationMs ?? 0;
  const canSeek = durationMs > 0 && Boolean(playback.snapshot.player);
  const [draftPositionMs, setDraftPositionMs] = useState<number | null>(null);
  const [pendingSeek, setPendingSeek] = useState<{ trackKey: string; positionMs: number } | null>(null);
  const pendingPositionMs = pendingSeek?.trackKey === trackKey ? pendingSeek.positionMs : null;
  const positionMs = durationMs > 0
    ? Math.min(durationMs, Math.max(0, draftPositionMs ?? pendingPositionMs ?? playback.positionMs))
    : 0;

  useEffect(() => {
    setDraftPositionMs(null);
    setPendingSeek(null);
  }, [trackKey]);

  useEffect(() => {
    if (
      pendingPositionMs === null
      || playback.isControlling
      || Math.abs(playback.positionMs - pendingPositionMs) > SEEK_SYNC_TOLERANCE_MS
    ) {
      return;
    }
    setPendingSeek((current) => (
      current?.trackKey === trackKey && current.positionMs === pendingPositionMs
        ? null
        : current
    ));
  }, [pendingPositionMs, playback.isControlling, playback.positionMs, trackKey]);

  useEffect(() => {
    if (pendingPositionMs === null) return;
    const timeout = setTimeout(() => {
      setPendingSeek((current) => (
        current?.trackKey === trackKey && current.positionMs === pendingPositionMs
          ? null
          : current
      ));
    }, SEEK_SYNC_TIMEOUT_MS);
    return () => clearTimeout(timeout);
  }, [pendingPositionMs, trackKey]);

  const commitPosition = (value: number) => {
    const nextPosition = Math.min(durationMs, Math.max(0, Math.round(value)));
    setDraftPositionMs(null);
    setPendingSeek({ trackKey, positionMs: nextPosition });
    void playback.seekTo(nextPosition)
      .catch(() => {
        setPendingSeek((current) => (
          current?.trackKey === trackKey && current.positionMs === nextPosition
            ? null
            : current
        ));
      });
  };

  return (
    <div className={styles.player}>
      <div className={styles.playerTop}>
        <div className={styles.playerMetadata}>
          <div className={styles.playerArtwork}>
            <ArtworkTransitionImage
              alt=""
              artworkLoading={playback.artworkLoading}
              artworkUrl={playback.artworkUrl}
              draggable={false}
              fallbackSrc={appIconUrl}
            />
          </div>
          <div className={styles.playerTrack}>
            <strong className={styles.playerTitle} title={playback.snapshot.title ?? undefined}>
              <OverflowText contentKey={`${trackKey}:title:${title}`} paused={marqueePaused}>{title}</OverflowText>
            </strong>
            <span className={styles.playerArtist} title={playback.snapshot.artist ?? undefined}>
              <OverflowText contentKey={`${trackKey}:artist:${artist}`} paused={marqueePaused}>{artist}</OverflowText>
            </span>
          </div>
        </div>
        {quickControls}
      </div>
      {previewLine && (
        <div className={styles.playerLyricsPreview}>
          <OverflowText
            align="center"
            behavior="once"
            contentKey={`${trackKey}:preview:${previewLine.startMs}:${previewLine.text}`}
            maxDurationMs={previewMaxDurationMs}
            paused={marqueePaused}
          >
            <KaraokeLine line={previewLine} positionMs={previewPositionMs} karaokeStyle={karaokeStyle} />
          </OverflowText>
        </div>
      )}
      <div className={styles.playerProgress}>
        <span className={styles.playerTime}>{formatPlaybackTime(positionMs)}</span>
        <Slider
          aria-label={t("notchLyrics.player.seek")}
          className={styles.playerSlider}
          disabled={!canSeek || playback.isControlling}
          max={Math.max(1, durationMs)}
          min={0}
          onValueChange={(value) => setDraftPositionMs(Number(value))}
          onValueCommitted={(value) => commitPosition(Number(value))}
          step={1_000}
          value={positionMs}
        />
        <span className={styles.playerTime}>−{formatPlaybackTime(Math.max(0, durationMs - positionMs))}</span>
      </div>
      <div className={styles.playerControls} role="group" aria-label={t("notchLyrics.player.label")}>
        <IconButton className={styles.playerControl} label={t("notchLyrics.player.previous")} variant="ghost" size="icon" onClick={() => void playback.previousTrack().catch(() => undefined)}><SkipBack fill="currentColor" strokeWidth={1.75} /></IconButton>
        <IconButton className={styles.playerPrimaryControl} label={playback.snapshot.isPlaying ? t("notchLyrics.player.pause") : t("notchLyrics.player.play")} variant="ghost" size="icon" onClick={() => void playback.togglePlayPause().catch(() => undefined)}>{playback.snapshot.isPlaying ? <Pause fill="currentColor" strokeWidth={1.5} /> : <Play fill="currentColor" strokeWidth={1.5} />}</IconButton>
        <IconButton className={styles.playerControl} label={t("notchLyrics.player.next")} variant="ghost" size="icon" onClick={() => void playback.nextTrack().catch(() => undefined)}><SkipForward fill="currentColor" strokeWidth={1.75} /></IconButton>
      </div>
    </div>
  );
}
