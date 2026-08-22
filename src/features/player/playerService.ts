import { api, isTauriRuntime, messageOf } from "../../shared/api";
import { listen } from "@tauri-apps/api/event";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import type {
  PlaybackAction,
  PlaybackArtwork,
  PlaybackSpectrumFrame,
  PlaybackSpectrumState,
} from "../../shared/types";

type SpectrumFrameListener = (frame: PlaybackSpectrumFrame) => void;
type SpectrumStateListener = (state: PlaybackSpectrumState) => void;

let spectrumSubscriberCount = 0;
let spectrumCommandQueue: Promise<void> = Promise.resolve();

function queueSpectrumCommand<T>(operation: () => Promise<T>) {
  const result = spectrumCommandQueue.then(operation, operation);
  spectrumCommandQueue = result.then(() => undefined, () => undefined);
  return result;
}

/**
 * 播放器能力的前端入口；界面只依赖这里，不直接拼接 Tauri 命令。
 */
export const playerService = {
  control(action: PlaybackAction) {
    return api.controlPlayback(action);
  },

  play() {
    return api.controlPlayback("play");
  },

  pause() {
    return api.controlPlayback("pause");
  },

  togglePlayPause() {
    return api.controlPlayback("toggle_play_pause");
  },

  previousTrack() {
    return api.controlPlayback("previous");
  },

  nextTrack() {
    return api.controlPlayback("next");
  },

  seek(positionMs: number) {
    return api.seekPlayback(positionMs);
  },

  getArtwork(artworkId: string): Promise<PlaybackArtwork | null> {
    return api.getPlaybackArtwork(artworkId);
  },

  subscribeSpectrum(
    onFrame: SpectrumFrameListener,
    onState: SpectrumStateListener,
  ): () => void {
    if (!isTauriRuntime()) return () => undefined;

    let disposed = false;
    let startRequested = false;
    spectrumSubscriberCount += 1;
    const frameListener = listen<PlaybackSpectrumFrame>(
      "playback://spectrum-frame",
      ({ payload }) => onFrame(payload),
    );
    const stateListener = listen<PlaybackSpectrumState>(
      "playback://spectrum-state",
      ({ payload }) => onState(payload),
    );
    const cleanupFrameListener = createTauriListenerCleanup(frameListener);
    const cleanupStateListener = createTauriListenerCleanup(stateListener);
    void Promise.all([frameListener, stateListener])
      .then(() => {
        if (disposed) return;
        startRequested = true;
        return queueSpectrumCommand(() => api.startPlaybackSpectrum())
          .then((state) => {
            if (!disposed) onState(state);
          });
      })
      .catch((error) => {
        if (!disposed) {
          onState({
            status: "unavailable",
            sourceAppBundleId: null,
            error: messageOf(error),
          });
        }
      });

    return () => {
      if (disposed) return;
      disposed = true;
      cleanupFrameListener();
      cleanupStateListener();
      spectrumSubscriberCount = Math.max(0, spectrumSubscriberCount - 1);
      if (startRequested && spectrumSubscriberCount === 0) {
        void queueSpectrumCommand(() => api.stopPlaybackSpectrum()).catch(() => undefined);
      }
    };
  },
};
