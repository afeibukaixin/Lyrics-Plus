import { useMemo } from "react";

import { trackKeyOf } from "../../shared/api";
import { useLyricsDocument } from "./useLyrics/document";
import { useLyricsLifecycle } from "./useLyrics/lifecycle";
import { createLyricsOffsetActions } from "./useLyrics/offset";
import { useLyricsSearch } from "./useLyrics/search";
import { useLyricsState } from "./useLyrics/state";
import { useLyricsDisplay } from "./useLyrics/display";
import type { PlaybackSnapshot } from "../../shared/types";

export { findAlignedAuxiliaryLine } from "./useLyrics/display";

export function useLyrics(snapshot: PlaybackSnapshot, positionMs: number, active = true) {
  const trackKey = useMemo(() => trackKeyOf(snapshot), [snapshot]);
  const state = useLyricsState(trackKey, active);
  const document = useLyricsDocument(snapshot, trackKey, state);
  const search = useLyricsSearch(snapshot, trackKey, state, document.updateDocument);
  useLyricsLifecycle(
    active,
    trackKey,
    state,
    document.loadTrack,
    document.load,
    search.restoreCompletedSearch,
    document.updateDocument,
  );
  const display = useLyricsDisplay(state.document, positionMs);
  const offset = createLyricsOffsetActions(
    trackKey,
    state,
    document.updateDocument,
    document.loadTrack,
  );

  return {
    trackKey,
    document: state.document,
    results: state.results,
    providerStatuses: state.providerStatuses,
    searching: state.searching,
    loadState: state.loadState,
    error: state.error,
    activeIndex: display.activeIndex,
    currentLine: display.currentLine,
    nextLine: display.nextLine,
    currentTranslation: display.currentTranslation,
    currentRomanization: display.currentRomanization,
    adjustedPositionMs: display.adjustedPositionMs,
    search: search.search,
    searchWith: search.searchWith,
    applyResult: search.applyResult,
    importRaw: document.importRaw,
    changeOffset: offset.changeOffset,
    setOffset: offset.setOffset,
    remove: document.remove,
  };
}
