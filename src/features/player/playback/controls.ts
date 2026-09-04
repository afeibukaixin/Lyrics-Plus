import { useCallback, useRef, useState, type Dispatch, type SetStateAction } from "react";

import { api, messageOf } from "../../../shared/api";
import type { PlaybackAction, PlayerSelection } from "../../../shared/types";

import { playerService } from "../playerService";

export function usePlaybackControls(
  selection: PlayerSelection,
  setSelectionState: Dispatch<SetStateAction<PlayerSelection>>,
  setConfigError: Dispatch<SetStateAction<string | null>>,
) {
  const [isControlling, setIsControlling] = useState(false);
  const [controlError, setControlError] = useState<string | null>(null);
  const controlPromiseRef = useRef<Promise<void> | null>(null);

  const setSelection = async (next: PlayerSelection) => {
    const previous = selection;
    setSelectionState(next);
    setConfigError(null);
    try {
      await api.setPlayerSelection(next);
    } catch (error) {
      setSelectionState(previous);
      setConfigError(messageOf(error));
      throw error;
    }
  };

  const runPlayerOperation = useCallback((task: () => Promise<void>) => {
    if (controlPromiseRef.current) return controlPromiseRef.current;
    setControlError(null);
    setIsControlling(true);
    const operation = Promise.resolve()
      .then(task)
      .catch((error) => {
        setControlError(messageOf(error));
        throw error;
      })
      .finally(() => {
        controlPromiseRef.current = null;
        setIsControlling(false);
      });
    controlPromiseRef.current = operation;
    return operation;
  }, []);

  const runControl = useCallback((action: PlaybackAction) => {
    return runPlayerOperation(() => playerService.control(action));
  }, [runPlayerOperation]);

  const seekTo = useCallback((positionMs: number) => {
    return runPlayerOperation(() => playerService.seek(positionMs));
  }, [runPlayerOperation]);

  const play = useCallback(() => runControl("play"), [runControl]);
  const pause = useCallback(() => runControl("pause"), [runControl]);
  const togglePlayPause = useCallback(
    () => runControl("toggle_play_pause"),
    [runControl],
  );
  const previousTrack = useCallback(() => runControl("previous"), [runControl]);
  const nextTrack = useCallback(() => runControl("next"), [runControl]);
  const clearControlError = useCallback(() => setControlError(null), []);

  return {
    setSelection,
    play,
    pause,
    togglePlayPause,
    previousTrack,
    nextTrack,
    seekTo,
    isControlling,
    controlError,
    clearControlError,
  };
}
