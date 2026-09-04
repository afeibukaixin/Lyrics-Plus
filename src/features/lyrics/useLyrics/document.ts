import { useCallback } from "react";

import { api, messageOf } from "../../../shared/api";
import type { LyricsDocument, PlaybackSnapshot } from "../../../shared/types";

import type { LyricsState } from "./state";

export type UpdateLyricsDocument = (
  next: LyricsDocument | null,
  key?: string | null,
) => void;

export type LoadLyricsTrack = (key: string) => Promise<LyricsDocument | null>;

export function useLyricsDocument(
  snapshot: PlaybackSnapshot,
  trackKey: string | null,
  state: LyricsState,
) {
  const updateDocument = useCallback<UpdateLyricsDocument>((
    next,
    key = state.activeTrackKey.current,
  ) => {
    state.documentRef.current = next;
    state.documentTrackKey.current = next ? key : null;
    state.setDocument(next);
  }, []);

  const loadTrack = useCallback(async (key: string) => {
    if (state.activeTrackKey.current === key) state.setLoadState("loading");
    try {
      const cached = await api.getCachedLyrics(key);
      const pending = state.pendingOffsetWrites.current.get(key);
      const next = cached.document && pending
        ? { ...cached.document, offsetMs: pending.desiredOffsetMs }
        : cached.document;
      if (state.activeTrackKey.current === key) {
        updateDocument(cached.status === "ready" ? next : null, key);
        state.setLoadState(cached.status);
        if (cached.error) state.setError(cached.error);
      }
      return next;
    } catch (loadError) {
      if (state.activeTrackKey.current === key) {
        updateDocument(null, key);
        state.setLoadState("error");
        state.setError(messageOf(loadError));
      }
      return null;
    }
  }, [updateDocument]);

  const load = useCallback(async () => {
    if (!trackKey) {
      updateDocument(null);
      state.setResults([]);
      state.setLoadState("idle");
      return null;
    }
    return loadTrack(trackKey);
  }, [loadTrack, trackKey, updateDocument]);

  const importRaw = async (raw: string) => {
    if (!trackKey || !snapshot.title || !snapshot.artist) return;
    state.setError(null);
    try {
      const imported = await api.importLyrics(
        trackKey,
        snapshot.title,
        snapshot.artist,
        snapshot.album,
        snapshot.durationMs,
        raw,
      );
      if (state.activeTrackKey.current === trackKey) {
        updateDocument(imported, trackKey);
        state.setLoadState("ready");
      }
    } catch (importError) {
      state.setError(messageOf(importError));
    }
  };

  const remove = async () => {
    if (!trackKey) return;
    try {
      await api.removeLyricsAssociation(trackKey);
      if (state.activeTrackKey.current === trackKey) {
        updateDocument(null);
        state.setLoadState("missing");
      }
    } catch (removeError) {
      state.setError(messageOf(removeError));
    }
  };

  return {
    updateDocument,
    loadTrack,
    load,
    importRaw,
    remove,
  };
}
