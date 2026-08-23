import { invoke } from "./core";
import type {
  PlaybackAction,
  PlaybackArtwork,
  PlaybackSnapshot,
  PlaybackSpectrumState,
  PlayerSelection,
 } from "./types";

export const playbackApi = {
  getPlayback: () => invoke<PlaybackSnapshot>("get_playback_snapshot"),
  controlPlayback: (action: PlaybackAction) =>
    invoke<void>("control_playback", { action }),
  seekPlayback: (positionMs: number) =>
    invoke<void>("seek_playback", { positionMs }),
  getPlaybackArtwork: (artworkId: string) =>
    invoke<PlaybackArtwork | null>("get_playback_artwork", { artworkId }),
  startPlaybackSpectrum: () =>
    invoke<PlaybackSpectrumState>("start_playback_spectrum"),
  stopPlaybackSpectrum: () => invoke<void>("stop_playback_spectrum"),
  getPlaybackSpectrumState: () =>
    invoke<PlaybackSpectrumState>("get_playback_spectrum_state"),
  getPlayerSelection: () => invoke<PlayerSelection>("get_player_selection"),
  setPlayerSelection: (selection: PlayerSelection) =>
    invoke<void>("set_player_selection", { selection }),
};
