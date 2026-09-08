import { useMemo, useRef } from "react";
import type { TFunction } from "i18next";
import { useGSAP } from "@gsap/react";
import { gsap } from "gsap";
import type { LyricsSearchProgress, LyricsSearchTrace } from "@/shared/types";
import { resultTrace } from "./helpers";
import styles from "../../QuickLyricsWindow.module.scss";
const SEARCH_STAGES = [
  "identify",
  "existing_binding",
  "local_search",
  "online_search",
  "compare_versions",
  "auto_apply",
  "await_selection",
] as const;

gsap.registerPlugin(useGSAP);

type SearchStageId = typeof SEARCH_STAGES[number];

const SEARCH_TIMELINE = {
  shared: ["identify", "existing_binding"] as const satisfies readonly SearchStageId[],
  parallel: ["local_search", "online_search"] as const satisfies readonly SearchStageId[],
  compare: "compare_versions" as const,
};

const OUTCOME_STAGES = ["auto_apply", "await_selection"] as const satisfies readonly SearchStageId[];

type TimelineStatus = "pending" | "running" | "completed" | "failed" | "waiting" | "skipped";

export type StageProgressSnapshot = LyricsSearchProgress & {
  receivedAtMs: number;
  startedAtMs: number | null;
  timingAvailable: boolean;
};

export function formatSeconds(elapsedMs: number | null | undefined) {
  if (elapsedMs === null || elapsedMs === undefined || !Number.isFinite(elapsedMs)) return "—";
  return `${(Math.max(0, elapsedMs) / 1000).toFixed(1)}s`;
}

function progressFromTrace(
  trace: LyricsSearchTrace,
  stage: LyricsSearchTrace["stages"][number],
): StageProgressSnapshot {
  return {
    runId: trace.runId,
    stage: stage.stage,
    providerId: stage.providerId,
    status: stage.status,
    elapsedMs: stage.elapsedMs,
    totalElapsedMs: trace.totalElapsedMs ?? 0,
    candidateCount: stage.candidateCount,
    receivedAtMs: Date.now(),
    startedAtMs: Number.isFinite(stage.startedAt) ? stage.startedAt * 1000 : null,
    // 没有累计耗时时不回推旧记录的时间，避免把秒级时间戳当成精确耗时。
    timingAvailable: trace.totalElapsedMs != null,
  };
}

export function historyFromTrace(trace: LyricsSearchTrace | null) {
  if (!trace) return {} as Record<string, StageProgressSnapshot>;
  return trace.stages.reduce<Record<string, StageProgressSnapshot>>((history, stage) => {
    history[stage.stage] = progressFromTrace(trace, stage);
    return history;
  }, {});
}

export function progressFromEvent(
  payload: LyricsSearchProgress,
  receivedAtMs = Date.now(),
  previous: StageProgressSnapshot | undefined,
): StageProgressSnapshot {
  return {
    ...payload,
    receivedAtMs,
    startedAtMs: previous?.runId === payload.runId && previous.startedAtMs !== null
      ? previous.startedAtMs
      : receivedAtMs - payload.elapsedMs,
    timingAvailable: true,
  };
}

const REJECTION_REASON_KEYS: Record<string, string> = {
  score_below_threshold: "quickLyrics.details.rejectionReasons.scoreBelowThreshold",
  not_selected: "quickLyrics.details.rejectionReasons.notSelected",
};

const SELECTION_REASON_KEYS: Record<string, string> = {
  automatic_score_and_quality_gate: "quickLyrics.details.selectionReasons.automaticScoreAndQualityGate",
  user_selected_default: "quickLyrics.details.selectionReasons.userSelectedDefault",
  user_selected_platform_override: "quickLyrics.details.selectionReasons.userSelectedPlatformOverride",
  user_selected: "quickLyrics.details.selectionReasons.userSelected",
};

export function localizedCandidateReason(
  reason: string,
  kind: "rejection" | "selection",
  t: TFunction,
) {
  const key = kind === "rejection"
    ? REJECTION_REASON_KEYS[reason]
    : SELECTION_REASON_KEYS[reason];
  if (key) return t(key, { defaultValue: reason });
  return t(
    kind === "rejection"
      ? "quickLyrics.details.rejectionReasons.fallback"
      : "quickLyrics.details.selectionReasons.fallback",
  );
}

export function scoreReasons(candidate: ReturnType<typeof resultTrace>, t: TFunction) {
  const evidence = candidate?.scoreEvidence;
  if (!evidence) return null;
  const capabilities = [
    evidence.synced ? t("quickLyrics.details.scoreReasons.lineTiming") : t("quickLyrics.details.scoreReasons.plainText"),
    evidence.wordTiming ? t("quickLyrics.details.scoreReasons.wordTiming") : null,
    evidence.translation ? t("quickLyrics.details.scoreReasons.translation") : null,
    evidence.romanization ? t("quickLyrics.details.scoreReasons.romanization") : null,
  ].filter(Boolean).join("、");
  return [
    t("quickLyrics.details.scoreReasons.titleMatch", { value: Math.round(evidence.titleSimilarity * 100) }),
    t("quickLyrics.details.scoreReasons.artistMatch", {
      value: Math.round(evidence.artistSimilarity * 100),
      state: evidence.artistMainConflict
        ? t("quickLyrics.details.scoreReasons.artistConflict")
        : evidence.spotifyArtistSubset
          ? t("quickLyrics.details.scoreReasons.artistSubset")
        : evidence.artistFeaturedComplete
          ? t("quickLyrics.details.scoreReasons.artistComplete")
          : t("quickLyrics.details.scoreReasons.artistMissing"),
    }),
    t("quickLyrics.details.scoreReasons.albumMatch", { value: Math.round(evidence.albumSimilarity * 100) }),
    t("quickLyrics.details.scoreReasons.duration", {
      value: evidence.durationDeltaMs === null
        ? t("quickLyrics.details.scoreReasons.durationUnknown")
        : `${(evidence.durationDeltaMs / 1000).toFixed(1)}s`,
    }),
    t("quickLyrics.details.scoreReasons.version", {
      value: evidence.versionConflict
        ? t("quickLyrics.details.scoreReasons.versionConflict")
        : evidence.versionSimilarity > 0
          ? t("quickLyrics.details.scoreReasons.versionMatch")
          : t("quickLyrics.details.scoreReasons.versionUnknown"),
    }),
    t("quickLyrics.details.scoreReasons.capabilities", { value: capabilities || t("quickLyrics.details.scoreReasons.none") }),
  ].join(" · ");
}

type SearchStageMetricProps = {
  item: StageProgressSnapshot;
  t: TFunction;
};

function SearchStageMetric({ item, t }: SearchStageMetricProps) {
  const rootRef = useRef<HTMLSpanElement>(null);
  const metricRef = useRef<HTMLSpanElement>(null);
  const reducedMotionRef = useRef(false);
  const displayStateRef = useRef<{ elapsedMs: number; candidateCount: number } | null>(null);
  const displayRunIdRef = useRef<string | null>(null);

  useGSAP(() => {
    const media = gsap.matchMedia();
    media.add({
      reduceMotion: "(prefers-reduced-motion: reduce)",
      allowMotion: "(prefers-reduced-motion: no-preference)",
    }, (context) => {
      reducedMotionRef.current = Boolean(context.conditions?.reduceMotion);
    });
    return () => media.revert();
  }, { scope: rootRef });

  useGSAP(() => {
    const metric = metricRef.current;
    if (!metric) return;

    const isRunning = item.status === "running";
    const startedAtMs = item.startedAtMs ?? item.receivedAtMs - item.elapsedMs;
    if (displayRunIdRef.current !== item.runId) {
      displayRunIdRef.current = item.runId;
      displayStateRef.current = null;
    }
    const previousState = displayStateRef.current;
    const state = {
      elapsedMs: isRunning
        ? Math.max(item.elapsedMs, Date.now() - startedAtMs)
        : previousState?.elapsedMs ?? item.elapsedMs,
      candidateCount: isRunning
        ? item.candidateCount
        : previousState?.candidateCount ?? item.candidateCount,
    };
    let lastRunningPaintAt = 0;
    const render = (force = false) => {
      const now = Date.now();
      if (isRunning && !force && now - lastRunningPaintAt < 100) return;
      if (isRunning) lastRunningPaintAt = now;
      const elapsed = isRunning
        ? Math.max(item.elapsedMs, now - startedAtMs)
        : state.elapsedMs;
      displayStateRef.current = { elapsedMs: elapsed, candidateCount: state.candidateCount };
      metric.textContent = `${Math.max(0, Math.round(state.candidateCount))} ${t("quickLyrics.details.candidates")} · ${formatSeconds(elapsed)}`;
    };

    render(true);
    if (isRunning) {
      const tick = () => render();
      gsap.ticker.add(tick);
      return () => gsap.ticker.remove(tick);
    }
    if (reducedMotionRef.current) {
      state.elapsedMs = item.elapsedMs;
      state.candidateCount = item.candidateCount;
      render();
      return;
    }

    const tween = gsap.to(state, {
      elapsedMs: item.elapsedMs,
      candidateCount: item.candidateCount,
      duration: 0.28,
      ease: "power1.out",
      overwrite: "auto",
      onUpdate: render,
    });
    return () => tween.kill();
  }, {
    dependencies: [
      item.runId,
      item.stage,
      item.status,
      item.elapsedMs,
      item.candidateCount,
      item.startedAtMs,
      item.receivedAtMs,
      t,
    ],
    scope: rootRef,
    revertOnUpdate: true,
  });

  return <span ref={rootRef} className={styles.detailsStageMetric}><span ref={metricRef} /></span>;
}

type TimelineStageProps = {
  stage: SearchStageId;
  item: StageProgressSnapshot | null;
  status: TimelineStatus;
  connectorStatus?: TimelineStatus;
  t: TFunction;
  showConnector?: boolean;
};

function TimelineStage({
  stage,
  item,
  status,
  connectorStatus = "pending",
  t,
  showConnector = true,
}: TimelineStageProps) {
  const label = t(`quickLyrics.details.stages.${stage}`, { defaultValue: stage });
  const hasMetric = Boolean(item?.timingAvailable && !["pending", "skipped"].includes(status));
  const connectorVisible = showConnector && status !== "skipped";
  return (
    <div
      className={styles.detailsStage}
      data-timeline-stage="true"
      data-stage={stage}
      data-status={status}
      role="listitem"
      aria-label={label}
    >
      <span className={styles.detailsStageRail} aria-hidden="true">
        <i />
        {connectorVisible && <span className={styles.detailsStageConnector} data-timeline-connector={connectorStatus}>
          <span className={styles.detailsStageConnectorFill} data-timeline-connector-fill={connectorStatus} />
          <span className={styles.detailsStageConnectorGlow} data-timeline-glow="true" />
        </span>}
      </span>
      <span className={styles.detailsStageLabel}>{label}</span>
      <span className={styles.detailsStageMeta}>
        {hasMetric && item ? <SearchStageMetric item={item} t={t} /> : status === "skipped" ? t("quickLyrics.details.skipped") : null}
      </span>
    </div>
  );
}

type SearchTimelineProps = {
  trace: LyricsSearchTrace | null;
  progress: LyricsSearchProgress | null;
  stageHistory: Record<string, StageProgressSnapshot>;
  t: TFunction;
};

export function SearchTimeline({ trace, progress, stageHistory, t }: SearchTimelineProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const reducedMotionRef = useRef(false);
  const traceHistory = useMemo(() => historyFromTrace(trace), [trace]);
  const activeRunId = progress?.runId ?? trace?.runId ?? null;
  const itemForStage = (stage: SearchStageId) => {
    const liveItem = stageHistory[stage];
    const traceItem = traceHistory[stage];
    const preferLive = Boolean(progress?.runId && liveItem?.runId === progress.runId);
    const item = preferLive || trace?.status === "running"
      ? liveItem ?? traceItem ?? null
      : traceItem ?? liveItem ?? null;
    return item && (!activeRunId || item.runId === activeRunId) ? item : null;
  };
  const items = useMemo(() => SEARCH_STAGES.reduce<Record<string, StageProgressSnapshot | null>>((result, stage) => {
    result[stage] = itemForStage(stage);
    return result;
  }, {}), [activeRunId, progress, stageHistory, trace, traceHistory]);
  const rawStatus = (stage: SearchStageId): TimelineStatus => {
    const status = items[stage]?.status;
    return status === "running" || status === "completed" || status === "failed" ? status : "pending";
  };
  const statusForStage = (stage: SearchStageId): TimelineStatus => {
    if (stage === "await_selection" && rawStatus(stage) === "completed") return "waiting";
    return rawStatus(stage);
  };
  const statuses = SEARCH_STAGES.reduce<Record<string, TimelineStatus>>((result, stage) => {
    result[stage] = statusForStage(stage);
    return result;
  }, {});

  const aggregateStatus = (stages: readonly SearchStageId[]): TimelineStatus => {
    const values = stages.map((stage) => statuses[stage]);
    if (values.includes("running")) return "running";
    if (values.includes("failed") && values.every((value) => value === "failed" || value === "skipped")) return "failed";
    if (values.includes("waiting")) return "waiting";
    if (values.includes("completed")) return "completed";
    return "pending";
  };
  const parallelStatus = aggregateStatus(SEARCH_TIMELINE.parallel);
  const outcomeStage = OUTCOME_STAGES.find((stage) => items[stage] !== null) ?? null;
  const outcomeStatus = outcomeStage ? statuses[outcomeStage] : "pending";
  const statusSignature = SEARCH_STAGES.map((stage) => `${stage}:${statuses[stage]}`).join("|");
  const animate = Boolean(progress) || trace?.status === "running";

  useGSAP(() => {
    const media = gsap.matchMedia();
    media.add({
      reduceMotion: "(prefers-reduced-motion: reduce)",
      allowMotion: "(prefers-reduced-motion: no-preference)",
    }, (context) => {
      reducedMotionRef.current = Boolean(context.conditions?.reduceMotion);
    });
    return () => media.revert();
  }, { scope: rootRef });

  useGSAP(() => {
    const root = rootRef.current;
    if (!root) return;
    const stageNodes = gsap.utils.toArray<HTMLElement>("[data-timeline-stage]", root);
    const connectorNodes = gsap.utils.toArray<HTMLElement>("[data-timeline-connector-fill]", root);
    const groupRailFills = gsap.utils.toArray<HTMLElement>("[data-timeline-group-fill]", root);
    const connectorGlows = gsap.utils.toArray<HTMLElement>("[data-timeline-glow]", root);
    const groupRailGlows = gsap.utils.toArray<HTMLElement>("[data-timeline-group-glow]", root);
    const runningDots = stageNodes
      .filter((node) => node.dataset.status === "running")
      .map((node) => node.querySelector("i"))
      .filter((node): node is HTMLElement => node instanceof HTMLElement);
    const visibleConnectors = connectorNodes.filter((node) => node.dataset.timelineConnectorFill !== "pending");
    const flowingGlows = connectorGlows.filter((node) => node.parentElement?.dataset.timelineConnector === "running");
    const flowingGroupGlows = groupRailGlows.filter((node) => node.parentElement?.parentElement?.dataset.groupStatus === "running");
    const visibleGroupRails = groupRailFills.filter((node) => node.dataset.timelineGroupFill !== "pending");

    const connectorTarget = (node: HTMLElement) => node.dataset.timelineConnectorFill === "completed"
      || node.dataset.timelineConnectorFill === "running"
      || node.dataset.timelineConnectorFill === "waiting"
      || node.dataset.timelineConnectorFill === "failed";
    gsap.set(connectorNodes, { scaleY: 0 });
    gsap.set(groupRailFills, { scaleY: 0 });
    gsap.set(connectorGlows, { y: "-120%", autoAlpha: 0 });
    gsap.set(groupRailGlows, { y: "-120%", autoAlpha: 0 });
    if (!animate || reducedMotionRef.current) {
      gsap.set(connectorNodes.filter(connectorTarget), { scaleY: 1 });
      gsap.set(visibleGroupRails, { scaleY: 1 });
      gsap.set(stageNodes, { clearProps: "transform,opacity,visibility" });
      return;
    }

    gsap.fromTo(stageNodes, { autoAlpha: 0.72, y: 4, scale: 0.92 }, {
      autoAlpha: 1,
      y: 0,
      scale: 1,
      duration: 0.28,
      ease: "power2.out",
      stagger: 0.035,
      overwrite: "auto",
    });
    gsap.to(visibleGroupRails, {
      scaleY: 1,
      duration: 0.42,
      ease: "power2.out",
      stagger: 0.08,
      overwrite: "auto",
    });
    gsap.to(visibleConnectors.filter(connectorTarget), {
      scaleY: 1,
      duration: 0.42,
      ease: "power2.out",
      stagger: 0.045,
      overwrite: "auto",
    });
    if (runningDots.length > 0) {
      gsap.to(runningDots, {
        scale: 1.3,
        repeat: -1,
        yoyo: true,
        duration: 0.78,
        ease: "sine.inOut",
        stagger: 0.06,
      });
    }
    if (flowingGlows.length > 0) {
      gsap.set(flowingGlows, { autoAlpha: 0.9, y: "-120%" });
      gsap.to(flowingGlows, {
        y: "120%",
        repeat: -1,
        duration: 1.05,
        ease: "none",
        stagger: 0.12,
      });
    }
    if (flowingGroupGlows.length > 0) {
      gsap.set(flowingGroupGlows, { autoAlpha: 0.9, y: "-120%" });
      gsap.to(flowingGroupGlows, {
        y: "120%",
        repeat: -1,
        duration: 1.05,
        ease: "none",
      });
    }
  }, {
    dependencies: [activeRunId, statusSignature, animate],
    scope: rootRef,
    revertOnUpdate: true,
  });

  return (
    <div ref={rootRef} className={styles.detailsTimeline} role="list" aria-label={t("quickLyrics.details.searchTrace")}>
      <div className={styles.detailsTimelineShared}>
        {SEARCH_TIMELINE.shared.map((stage, index) => (
          <TimelineStage
            key={stage}
            stage={stage}
            item={items[stage]}
            status={statuses[stage]}
            connectorStatus={statuses[stage] === "failed"
              ? "failed"
              : index === SEARCH_TIMELINE.shared.length - 1 ? parallelStatus : statuses[stage]}
            t={t}
          />
        ))}
      </div>
      <div className={styles.detailsTimelineGroup} data-group-status={parallelStatus}>
        <span className={styles.detailsTimelineGroupRail} aria-hidden="true"><span className={styles.detailsTimelineGroupRailFill} data-timeline-group-fill={parallelStatus} /><span className={styles.detailsTimelineGroupRailGlow} data-timeline-group-glow="true" /></span>
        <div className={styles.detailsTimelineGroupRows}>
          {SEARCH_TIMELINE.parallel.map((stage) => (
            <TimelineStage
              key={stage}
              stage={stage}
              item={items[stage]}
              status={statuses[stage]}
              connectorStatus={parallelStatus === "failed" ? "failed" : statuses[stage] === "running" ? "running" : statuses[stage] === "completed" ? "completed" : "pending"}
              t={t}
              showConnector={false}
            />
          ))}
        </div>
      </div>
      <TimelineStage
        stage={SEARCH_TIMELINE.compare}
        item={items[SEARCH_TIMELINE.compare]}
        status={statuses[SEARCH_TIMELINE.compare]}
        connectorStatus={statuses[SEARCH_TIMELINE.compare] === "failed" ? "failed" : outcomeStatus}
        t={t}
        showConnector={Boolean(outcomeStage)}
      />
      {outcomeStage ? (
        <TimelineStage
          stage={outcomeStage}
          item={items[outcomeStage]}
          status={outcomeStatus}
          showConnector={false}
          t={t}
        />
      ) : null}
    </div>
  );
}
