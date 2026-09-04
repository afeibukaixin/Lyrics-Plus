import { usePlaybackArtwork } from "./playback/artwork";
import { usePlaybackControls } from "./playback/controls";
import { usePlaybackEvents } from "./playback/events";
import { usePlaybackPosition } from "./playback/position";
import { useSurfaceActivity } from "../window/useSurfaceActivity";

type UsePlaybackOptions = {
  loadArtwork?: boolean;
  trackPosition?: boolean;
};

export function usePlayback({
  loadArtwork = false,
  trackPosition = true,
}: UsePlaybackOptions = {}) {
  const active = useSurfaceActivity();
  const playbackEvents = usePlaybackEvents(active);
  const artwork = usePlaybackArtwork(active, loadArtwork, playbackEvents.snapshot);
  const positionMs = usePlaybackPosition(active, trackPosition, playbackEvents.snapshot);
  const controls = usePlaybackControls(
    playbackEvents.selection,
    playbackEvents.syncSelection,
    playbackEvents.setConfigError,
  );

  return {
    active,
    snapshot: playbackEvents.snapshot,
    positionMs,
    selection: playbackEvents.selection,
    setSelection: controls.setSelection,
    syncSelection: playbackEvents.syncSelection,
    configError: playbackEvents.configError,
    snapshotLoadError: playbackEvents.snapshotLoadError,
    refreshSnapshot: playbackEvents.refreshSnapshot,
    play: controls.play,
    pause: controls.pause,
    togglePlayPause: controls.togglePlayPause,
    previousTrack: controls.previousTrack,
    nextTrack: controls.nextTrack,
    seekTo: controls.seekTo,
    isControlling: controls.isControlling,
    controlError: controls.controlError,
    clearControlError: controls.clearControlError,
    artworkUrl: artwork.artworkUrl,
    artworkAccentColor: artwork.artworkAccentColor,
    artworkSpectrumColors: artwork.artworkSpectrumColors,
    artworkLoading: artwork.artworkLoading,
    artworkError: artwork.artworkError,
  };
}
