import { useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent, type FormEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { api, AppOperationError, isTauriRuntime, messageOf } from "@/shared/api";
import { createTauriListenerCleanup } from "@/shared/tauriEvent";
import type { ArtistCredit, LyricsContext, LyricsSearchProgress, LyricsSearchResult, LyricsSearchTrace, SongAssociationCandidate, TrackObservation } from "@/shared/types";
import { type QuickLyricsDetailsProps, type RecordingAction, type DetachLyricsMode, normalizeArtistName, resultTrace } from "./helpers";
import { type StageProgressSnapshot, historyFromTrace, progressFromEvent } from "./SearchTimeline";
export function useSongManager({
  open,
  playback,
  lyrics,
  onSearch,
  t,
}: QuickLyricsDetailsProps) {
  const [context, setContext] = useState<LyricsContext | null>(null);
  const [trace, setTrace] = useState<LyricsSearchTrace | null>(null);
  const [progress, setProgress] = useState<LyricsSearchProgress | null>(null);
  const [stageHistory, setStageHistory] = useState<Record<string, StageProgressSnapshot>>({});
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [aliasDrafts, setAliasDrafts] = useState<Record<string, string>>({});
  const [aliasErrors, setAliasErrors] = useState<Record<string, string>>({});
  const [recordingAction, setRecordingAction] = useState<RecordingAction>(null);
  const [splitLyricsMode, setDetachLyricsMode] = useState<DetachLyricsMode>("unbound");
  const [associationCandidates, setAssociationCandidates] = useState<SongAssociationCandidate[]>([]);
  const [associationCandidate, setAssociationCandidate] = useState<SongAssociationCandidate | null>(null);
  const importInput = useRef<HTMLInputElement>(null);
  const runIdRef = useRef<string | null>(null);
  const trackKeyRef = useRef<string | null>(lyrics.trackKey);
  const trackKey = lyrics.trackKey;

  const refresh = useCallback(async () => {
    if (!trackKey) {
      setContext(null);
      setTrace(null);
      setProgress(null);
      setStageHistory({});
      runIdRef.current = null;
      return;
    }
    try {
      const [nextContext, nextTrace] = await Promise.all([
        api.getCurrentLyricsContext(trackKey),
        api.getLyricsSearchTrace(trackKey),
      ]);
      if (trackKeyRef.current !== trackKey) return;
      setError(null);
      setContext(nextContext);
      if (runIdRef.current && nextTrace && nextTrace.runId !== runIdRef.current) return;
      setTrace(nextTrace);
      if (nextTrace) {
        runIdRef.current = nextTrace.runId;
        setStageHistory((previous) => {
          const nextHistory = historyFromTrace(nextTrace);
          if (nextTrace.status === "running") {
            // 面板在搜索过程中重新打开时，优先保留已经收到的毫秒级实时快照。
            for (const [stage, item] of Object.entries(previous)) {
              if (item.runId === nextTrace.runId) nextHistory[stage] = item;
            }
          }
          return nextHistory;
        });
        setProgress((previous) => previous?.runId === nextTrace.runId ? previous : null);
      } else if (!runIdRef.current) {
        setStageHistory({});
      }
    } catch (refreshError) {
      setError(messageOf(refreshError));
    }
  }, [trackKey]);

  useEffect(() => {
    if (!open) return;
    void refresh();
  }, [lyrics.document, open, refresh]);

  useEffect(() => {
    if (!open || !trackKey || !context) {
      setAssociationCandidates([]);
      return;
    }
    if (busyAction !== null) return;
    let active = true;
    void api.getSongAssociationCandidates(trackKey, context.platform)
      .then((candidates) => {
        if (active) setAssociationCandidates(candidates);
      })
      .catch((candidateError) => {
        if (active) {
          setAssociationCandidates([]);
          setError(messageOf(candidateError));
        }
      });
    return () => {
      active = false;
    };
  }, [open, trackKey, context, busyAction]);

  useEffect(() => {
    if (!open || !trackKey || !isTauriRuntime()) return;
    return createTauriListenerCleanup(
      listen<LyricsSearchProgress>("lyrics-search-progress", ({ payload }) => {
        if (runIdRef.current && runIdRef.current !== payload.runId) {
          // 只有新一轮的首个节点可以切换 runId；旧一轮晚到的完成事件直接丢弃。
          if (payload.stage !== "identify") return;
          setTrace(null);
          setProgress(null);
          setStageHistory({});
        }
        runIdRef.current = payload.runId;
        setProgress(payload);
        setStageHistory((previous) => {
          const receivedAtMs = Date.now();
          return {
            ...previous,
            [payload.stage]: progressFromEvent(payload, receivedAtMs, previous[payload.stage]),
          };
        });
        const isFinalStage = payload.stage === "auto_apply" || payload.stage === "await_selection";
        if (isFinalStage && (payload.status === "completed" || payload.status === "failed")) {
          const eventRunId = payload.runId;
          void api.getLyricsSearchTrace(trackKey).then((nextTrace) => {
            if (trackKeyRef.current !== trackKey) return;
            if (runIdRef.current !== eventRunId) return;
            if (nextTrace && nextTrace.runId !== eventRunId) return;
            setTrace(nextTrace);
            if (nextTrace && nextTrace.status !== "running") {
              setStageHistory(historyFromTrace(nextTrace));
            }
          }).catch((traceError) => setError(messageOf(traceError)));
        }
      }),
    );
  }, [open, trackKey]);

  useEffect(() => {
    if (!open) {
      // 关闭面板后不保留“本次实时动画”标记；重新打开时只从摘要恢复。
      setProgress(null);
    }
  }, [open]);

  useEffect(() => {
    trackKeyRef.current = trackKey;
    setContext(null);
    setTrace(null);
    setProgress(null);
    setStageHistory({});
    runIdRef.current = null;
    setError(null);
    setAliasDrafts({});
    setAliasErrors({});
    setRecordingAction(null);
    setDetachLyricsMode("unbound");
    setAssociationCandidates([]);
    setAssociationCandidate(null);
  }, [trackKey]);

  const currentBinding = useMemo(() => {
    if (!context) return null;
    return context.bindings.find((binding) => binding.bindingId === context.currentBindingId)
      ?? context.bindings.find((binding) => binding.isDefault)
      ?? null;
  }, [context]);

  const runAction = async (
    key: string,
    action: () => Promise<unknown>,
    handleError?: (error: unknown) => boolean,
  ) => {
    setBusyAction(key);
    setError(null);
    try {
      await action();
      await refresh();
      return true;
    } catch (actionError) {
      if (!handleError?.(actionError)) setError(messageOf(actionError));
      return false;
    } finally {
      setBusyAction(null);
    }
  };

  const selectCandidate = (result: LyricsSearchResult) => runAction(
    `${result.providerId}:${result.id}:default`,
    async () => {
      // 歌词搜索结果只设置歌曲共用歌词，不再创建平台专属歌词覆盖。
      await lyrics.applyResult(result, true);
    },
  );

  const handleImport = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.currentTarget.files?.[0];
    event.currentTarget.value = "";
    if (!file) return;
    await runAction("import", async () => {
      await lyrics.importRaw(await file.text());
    });
  };

  const handleSearch = () => runAction("search", async () => {
    await onSearch();
  });

  const openSplitDialog = (observation: TrackObservation) => {
    setDetachLyricsMode("unbound");
    setRecordingAction({ kind: "split", observation });
  };

  const handleRecordingAction = () => {
    if (!recordingAction) return;
    const action = recordingAction;
    setRecordingAction(null);
    void runAction(`split:${action.observation.platform}:${action.observation.trackKey}`, async () => {
      await api.detachPlatformTrack(action.observation.trackKey, action.observation.platform, splitLyricsMode);
    }).then((success) => {
      if (success) toast.success(t("quickLyrics.details.detachPlatformTrackDone"));
    });
  };

  const openAssociationDialog = (candidate: SongAssociationCandidate) => {
    setAssociationCandidate(candidate);
  };

  const handleAssociateCandidate = () => {
    const candidate = associationCandidate;
    if (!candidate || !trackKey || !context) return;
    setAssociationCandidate(null);
    void runAction(`associate:${candidate.recordingId}`, async () => {
      await api.associateSongCandidate(trackKey, context.platform, candidate.recordingId);
      // 只有事务成功才移除旧候选，失败时保持当前状态。
      setAssociationCandidates([]);
    }).then((success) => {
      if (success) {
        toast.success(t("quickLyrics.details.mergeRecordingDone"));
      }
    });
  };

  const handleAliasDraftChange = (artistId: number, value: string) => {
    const key = String(artistId);
    setAliasDrafts((previous) => ({ ...previous, [key]: value }));
    setAliasErrors((previous) => {
      if (!(key in previous)) return previous;
      const next = { ...previous };
      delete next[key];
      return next;
    });
  };

  const handleAliasAdd = (event: FormEvent<HTMLFormElement>, credit: ArtistCredit) => {
    event.preventDefault();
    if (!trackKey || credit.artistId === null) return;
    const artistId = credit.artistId;
    const key = String(artistId);
    const alias = (aliasDrafts[key] ?? "").trim();
    const normalizedAlias = normalizeArtistName(alias);
    const setValidationError = (message: string) => {
      setAliasErrors((previous) => ({ ...previous, [key]: message }));
    };
    if (!alias) {
      setValidationError(t("quickLyrics.details.artistAliasEditor.required"));
      return;
    }
    if (!normalizedAlias) {
      setValidationError(t("quickLyrics.details.artistAliasEditor.invalid"));
      return;
    }
    if (normalizedAlias === normalizeArtistName(credit.rawName)) {
      setValidationError(t("quickLyrics.details.artistAliasEditor.sameAsArtist"));
      return;
    }
    if (credit.confirmedAliases.some((existing) => normalizeArtistName(existing) === normalizedAlias)) {
      setValidationError(t("quickLyrics.details.artistAliasEditor.duplicate"));
      return;
    }
    void runAction(
      `artist-alias:add:${artistId}:${normalizedAlias}`,
      async () => {
        await api.setArtistAliasConfirmation(trackKey, artistId, alias, true);
        if (trackKeyRef.current !== trackKey) return;
        setAliasDrafts((previous) => ({ ...previous, [key]: "" }));
        setAliasErrors((previous) => {
          if (!(key in previous)) return previous;
          const next = { ...previous };
          delete next[key];
          return next;
        });
      },
      (actionError) => {
        if (!(actionError instanceof AppOperationError)) return false;
        const validationKeys: Partial<Record<string, "required" | "invalid" | "sameAsArtist" | "duplicate">> = {
          "歌手别名不能为空": "required",
          "歌手别名不包含有效字符": "invalid",
          "歌手别名与规范歌手名称相同": "sameAsArtist",
          "该歌手等价名称已存在": "duplicate",
        };
        const validationKey = validationKeys[actionError.message];
        if (!validationKey) return false;
        setValidationError(t(`quickLyrics.details.artistAliasEditor.${validationKey}`));
        return true;
      },
    );
  };

  // 别名编辑只刷新歌曲身份上下文；用户下次搜索时才会使用最新别名。
  const handleAliasRemove = (credit: ArtistCredit, alias: string) => {
    if (!trackKey || credit.artistId === null) return;
    const artistId = credit.artistId;
    void runAction(`artist-alias:remove:${artistId}:${normalizeArtistName(alias)}`, async () => {
      await api.setArtistAliasConfirmation(trackKey, artistId, alias, false);
    });
  };

  const observations = context?.recording.observations ?? [];
  const currentPlatformId = context?.recording.externalIdentifiers
    .filter((identifier) => identifier.namespace === context.platform)
    .sort((left, right) => Number(right.confirmed) - Number(left.confirmed))[0]?.value
    ?? playback.trackId
    ?? context?.observation.trackKey;
  const currentAsset = context?.currentAsset ?? null;
  // “沿用歌曲共用歌词”检查录音级默认绑定；当前平台可能有空覆盖，
  // 这不应阻止用户把歌曲默认歌词带到新建的独立歌曲。
  const canInheritCurrentLyrics = Boolean(context?.bindings.some((binding) => binding.isDefault && binding.asset?.available));
  const currentOffset = context?.platformOverride?.offsetMs ?? currentBinding?.offsetMs ?? lyrics.document?.offsetMs ?? 0;
  const assetCapabilities = currentAsset ? [
    t("quickLyrics.details.capabilities.plainText"),
    lyrics.document?.tracks.original.lines.length
      ? t("quickLyrics.details.capabilities.lineTiming")
      : null,
    currentAsset.hasWordTiming ? t("common.feature.wordTiming") : null,
    currentAsset.hasTranslation ? t("common.feature.hasTranslation") : null,
    currentAsset.hasRomanization ? t("common.feature.romanization") : null,
  ].filter(Boolean) as string[] : [];
  const candidateGroups = useMemo(() => {
    const groups = new Map<string, LyricsSearchResult[]>();
    for (const result of lyrics.results) {
      const candidate = resultTrace(result, trace);
      const version = candidate?.versionTags.length
        ? candidate.versionTags.join(" / ")
        : t("quickLyrics.details.unspecified");
      const existing = groups.get(version) ?? [];
      existing.push(result);
      groups.set(version, existing);
    }
    return [...groups.entries()];
  }, [lyrics.results, t, trace]);
  const latestTraceStage = trace ? trace.stages[trace.stages.length - 1] : undefined;
  const stageLabel = progress
    ? t(`quickLyrics.details.stages.${progress.stage}`, { defaultValue: progress.stage })
    : latestTraceStage
      ? t(`quickLyrics.details.stages.${latestTraceStage.stage}`, { defaultValue: latestTraceStage.stage })
      : trace
        ? t("quickLyrics.details.searchFinished")
        : t("quickLyrics.details.notSearched");
  const totalElapsedMs = trace?.totalElapsedMs ?? progress?.totalElapsedMs ?? null;
  const timingAvailable = progress !== null || trace?.totalElapsedMs != null;

  return {
    context, trace, progress, stageHistory, busyAction, error, setError,
    aliasDrafts, aliasErrors, recordingAction, setRecordingAction,
    splitLyricsMode, setDetachLyricsMode, associationCandidates, associationCandidate,
    setAssociationCandidate, importInput, trackKey, refresh, currentBinding, runAction,
    selectCandidate, handleImport, handleSearch, openSplitDialog, handleRecordingAction,
    openAssociationDialog, handleAssociateCandidate, handleAliasDraftChange, handleAliasAdd,
    handleAliasRemove, observations, currentPlatformId, currentAsset, canInheritCurrentLyrics,
    currentOffset, assetCapabilities, candidateGroups, stageLabel, totalElapsedMs, timingAvailable,
  };
}
export type SongManagerViewProps = QuickLyricsDetailsProps & ReturnType<typeof useSongManager>;
