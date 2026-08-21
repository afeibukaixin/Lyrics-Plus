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

  getArtwork(artworkId: string): Promise<PlaybackArtwork | null> {
    return api.getPlaybackArtwork(artworkId);
  },

  subscribeSpectrum(
    onFrame: SpectrumFrameListener,
    onState: SpectrumStateListener,
  ): () => void {
    if (!isTauriRuntime()) return () => undefined;

    let disposed = false;
    spectrumSubscriberCount += 1;
    const cleanupFrameListener = createTauriListenerCleanup(
      listen<PlaybackSpectrumFrame>("playback://spectrum-frame", ({ payload }) => onFrame(payload)),
    );
    const cleanupStateListener = createTauriListenerCleanup(
      listen<PlaybackSpectrumState>("playback://spectrum-state", ({ payload }) => onState(payload)),
    );
    void queueSpectrumCommand(() => api.startPlaybackSpectrum())
      .then((state) => {
        if (disposed) {
          // 组件可能在 start 命令返回前卸载，补一次 stop 避免留下孤立订阅。
          if (spectrumSubscriberCount === 0) {
            void queueSpectrumCommand(() => api.stopPlaybackSpectrum()).catch(() => undefined);
          }
          return;
        }
        onState(state);
      })
      .catch((error) => {
        if (!disposed) {
          onState({
            status: "unavailable",
            sourceAppBundleId: null,
            error: messageOf(error),
          });
        } else if (spectrumSubscriberCount === 0) {
          void queueSpectrumCommand(() => api.stopPlaybackSpectrum()).catch(() => undefined);
        }
      });

    return () => {
      if (disposed) return;
      disposed = true;
      cleanupFrameListener();
      cleanupStateListener();
      spectrumSubscriberCount = Math.max(0, spectrumSubscriberCount - 1);
      if (spectrumSubscriberCount === 0) {
        void queueSpectrumCommand(() => api.stopPlaybackSpectrum()).catch(() => undefined);
      }
    };
  },
};
