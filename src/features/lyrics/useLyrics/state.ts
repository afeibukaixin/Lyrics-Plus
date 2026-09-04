import { useRef, useState } from "react";

import type {
  LyricsDocument,
  LyricsLoadStatus,
  LyricsSearchResult,
  ProviderStatus,
} from "../../../shared/types";

export type PendingOffsetWrite = {
  desiredOffsetMs: number;
  count: number;
};

export type LyricsLoadState = "idle" | "loading" | LyricsLoadStatus;

export function useLyricsState(trackKey: string | null, active: boolean) {
  const [document, setDocument] = useState<LyricsDocument | null>(null);
  const [results, setResults] = useState<LyricsSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [providerStatuses, setProviderStatuses] = useState<ProviderStatus[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loadState, setLoadState] = useState<LyricsLoadState>("idle");
  const searchGeneration = useRef(0);
  const activeTrackKey = useRef(trackKey);
  const documentRef = useRef<LyricsDocument | null>(null);
  const documentTrackKey = useRef<string | null>(null);
  const pendingOffsetWrites = useRef(new Map<string, PendingOffsetWrite>());
  const offsetWriteQueue = useRef<Promise<void>>(Promise.resolve());
  activeTrackKey.current = active ? trackKey : null;

  return {
    document,
    setDocument,
    results,
    setResults,
    searching,
    setSearching,
    providerStatuses,
    setProviderStatuses,
    error,
    setError,
    loadState,
    setLoadState,
    searchGeneration,
    activeTrackKey,
    documentRef,
    documentTrackKey,
    pendingOffsetWrites,
    offsetWriteQueue,
  };
}

export type LyricsState = ReturnType<typeof useLyricsState>;
