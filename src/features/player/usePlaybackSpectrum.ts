import { useEffect, useState } from "react";
import { isTauriRuntime } from "../../shared/api";
import type {
  PlaybackSpectrumBands,
  PlaybackSpectrumState,
} from "../../shared/types";
import { playerService } from "./playerService";

const silentBands: PlaybackSpectrumBands = [0, 0, 0, 0, 0, 0];

const initialState: PlaybackSpectrumState = {
  status: "idle",
  sourceAppBundleId: null,
  error: null,
};

/**
 * 频谱是独立的懒加载能力；只有真正使用这个 hook 的界面才会申请系统音频权限。
 */
export function usePlaybackSpectrum(
  enabled = true,
  onBands?: (bands: PlaybackSpectrumBands) => void,
) {
  const [bands, setBands] = useState<PlaybackSpectrumBands>(silentBands);
  const [state, setState] = useState<PlaybackSpectrumState>(initialState);

  useEffect(() => {
    if (!enabled || !isTauriRuntime()) {
      setBands(silentBands);
      setState(initialState);
      onBands?.(silentBands);
      return;
    }
    return playerService.subscribeSpectrum(
      (frame) => {
        if (onBands) onBands(frame.bands);
        else setBands(frame.bands);
        setState((previous) => previous.sourceAppBundleId === frame.sourceAppBundleId
          ? previous
          : { ...previous, sourceAppBundleId: frame.sourceAppBundleId });
      },
      (next) => {
        setState(next);
        if (next.status !== "running") {
          setBands(silentBands);
          onBands?.(silentBands);
        }
      },
    );
  }, [enabled, onBands]);

  return {
    bands,
    status: state.status,
    error: state.error,
    sourceAppBundleId: state.sourceAppBundleId,
  };
}
