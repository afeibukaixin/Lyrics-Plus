import { api, messageOf } from "../../../shared/api";

import type { LoadLyricsTrack, UpdateLyricsDocument } from "./document";
import type { LyricsState } from "./state";

type ResolveNextOffset = (currentOffsetMs: number) => number;

export function createLyricsOffsetActions(
  trackKey: string | null,
  state: LyricsState,
  updateDocument: UpdateLyricsDocument,
  loadTrack: LoadLyricsTrack,
) {
  const enqueueOffsetWrite = (resolveNext: ResolveNextOffset) => {
    const key = trackKey;
    const current = state.documentRef.current;
    if (!key || !current || state.documentTrackKey.current !== key) return Promise.resolve();

    const existing = state.pendingOffsetWrites.current.get(key);
    const next = Math.trunc(resolveNext(existing?.desiredOffsetMs ?? current.offsetMs));
    state.pendingOffsetWrites.current.set(key, {
      desiredOffsetMs: next,
      count: (existing?.count ?? 0) + 1,
    });
    updateDocument({ ...current, offsetMs: next }, key);
    state.setError(null);

    let writeError: unknown = null;
    const write = state.offsetWriteQueue.current
      .then(() => api.setLyricsOffset(key, next))
      .catch((offsetError: unknown) => {
        writeError = offsetError;
      })
      .then(async () => {
        const pending = state.pendingOffsetWrites.current.get(key);
        if (!pending) return;
        if (pending.count > 1) {
          state.pendingOffsetWrites.current.set(key, { ...pending, count: pending.count - 1 });
          return;
        }
        state.pendingOffsetWrites.current.delete(key);
        await loadTrack(key);
        if (writeError && state.activeTrackKey.current === key) state.setError(messageOf(writeError));
      });
    state.offsetWriteQueue.current = write;
    return write;
  };

  const changeOffset = (delta: number) => enqueueOffsetWrite((current) => current + delta);
  const setOffset = (offsetMs: number) => enqueueOffsetWrite(() => offsetMs);

  return { changeOffset, setOffset };
}
