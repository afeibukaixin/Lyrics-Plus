import { useEffect, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { AlertTriangle, Check, GitMerge, Link2, Plus, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemTitle } from "@/components/ui/item";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { CandidateEvidence } from "@/features/lyrics/quickLyrics/details/CandidateEvidence";
import { associationReasonLabel } from "@/features/lyrics/quickLyrics/details/helpers";
import { lyricsApi } from "@/shared/api/lyrics";
import { messageOf } from "@/shared/api";
import type { LibraryLyricSummary, LibraryPage, LibrarySongDetail, LibrarySongSummary, SongAssociationCandidate, SongSimilarityBatchPreview, SongSimilarityPair } from "@/shared/types/lyrics";
import { ConfirmAction, formatDuration, LibraryDetailHeader, LibraryDetailSection, LibraryRefreshing, LibraryRelationItem, LibraryRelationList, LibraryState, LibraryToolbar, PageControls, TruncatedText, useLibraryNavigation } from "./shared";
import { useLibraryViewState } from "./viewState";
import styles from "./library.module.scss";

function songSourceLabel(
  source: { platform: string; sourceAppName: string | null; sourceAppBundleId: string | null },
  t: TFunction,
) {
  if (source.platform.toLowerCase() !== "system") return source.platform;
  const app = source.sourceAppName?.trim()
    || source.sourceAppBundleId?.trim()
    || t("library.manager.unknownSourceApp");
  return t("library.manager.systemSource", { app });
}

function normalizedPlatform(platform: string) {
  return platform.trim().toLowerCase();
}

function conflictingPlatforms(pair: SongSimilarityPair) {
  const counts = new Map<string, { label: string; count: number }>();
  pair.songs.flatMap((song) => song.sources).forEach((source) => {
    const platform = normalizedPlatform(source.platform);
    if (!platform || platform === "system") return;
    const current = counts.get(platform);
    counts.set(platform, {
      label: current?.label ?? source.platform.trim(),
      count: (current?.count ?? 0) + source.observationCount,
    });
  });
  return [...counts.values()].filter((item) => item.count > 1).map((item) => item.label);
}

function observationCount(song: LibrarySongSummary) {
  return song.sources.reduce((count, source) => count + source.observationCount, 0);
}

export default function SongsLibrary({ detailId, similarityOpen, onSimilarityClose }: {
  detailId: number | null;
  similarityOpen: boolean;
  onSimilarityClose: () => void;
}) {
  const { t } = useTranslation();
  const navigation = useLibraryNavigation({
    section: "songs",
    sectionLabel: t("library.manager.tabs.songs"),
    detailsLabel: t("library.manager.details"),
    detailId: null,
  });
  const view = useLibraryViewState("songs");
  const { query, page, pageSize } = view;
  const [data, setData] = useState<LibraryPage<LibrarySongSummary> | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingError, setLoadingError] = useState("");

  useEffect(() => {
    if (detailId !== null || similarityOpen) return;
    let live = true;
    let correctingPage = false;
    setLoading(true);
    setLoadingError("");
    const timer = window.setTimeout(() => {
      lyricsApi.listLibrarySongs(query, page, pageSize).then((result) => {
        if (!live) return;
        const lastPage = Math.max(1, Math.ceil(result.total / pageSize));
        if (page > lastPage) { correctingPage = true; view.update({ page: lastPage }, true); return; }
        setData(result);
        setLoadingError("");
      }).catch((error) => { if (live) setLoadingError(messageOf(error)); })
        .finally(() => { if (live && !correctingPage) setLoading(false); });
    }, 180);
    return () => { live = false; window.clearTimeout(timer); };
  }, [query, page, pageSize, detailId, similarityOpen]);

  if (detailId) return <SongDetail recordingId={detailId} />;
  if (similarityOpen) return <SongSimilarityQueue onBack={onSimilarityClose} />;
  return (
    <div className={styles.workspace}>
      <Card className={`${styles.panel} ${styles.listPanel}`}>
        <LibraryToolbar
          query={query}
          onQueryChange={(value) => view.update({ query: value, page: 1 }, true)}
        />
        <CardContent className={styles.tableContent} aria-busy={loading}>
        {loading && data ? <LibraryRefreshing /> : null}
        {loadingError ? <LibraryState state="error" message={loadingError} /> : data === null ? <LibraryState state="loading" message={t("library.manager.loading")} /> : data.items.length ? (
          <Table inert={loading} className={`${styles.adaptiveTable} ${styles.dataTable}`}>
            <colgroup>
              <col className={styles.songTitleColumn} />
              <col className={styles.songSourcesColumn} />
              <col className={styles.songLyricsColumn} />
              <col className={styles.songDurationColumn} />
              <col className={styles.songActionsColumn} />
            </colgroup>
            <TableHeader><TableRow><TableHead className={styles.growColumn}>{t("library.manager.song")}</TableHead><TableHead className={`${styles.numericColumn} ${styles.compactColumn}`}>{t("library.manager.sources")}</TableHead><TableHead className={`${styles.numericColumn} ${styles.compactColumn}`}>{t("library.manager.lyricsCount")}</TableHead><TableHead className={`${styles.numericColumn} ${styles.compactColumn}`}>{t("library.manager.duration")}</TableHead><TableHead className={`${styles.actionColumn} ${styles.compactColumn}`}>{t("library.manager.actions")}</TableHead></TableRow></TableHeader>
            <TableBody>{data.items.map((song) => (
              <TableRow key={song.recordingId}>
                <TableCell className={`${styles.primaryCell} ${styles.growColumn}`}><TruncatedText variant="title">{song.title}</TruncatedText><TruncatedText variant="meta">{song.artists.join(" / ") || "—"}</TruncatedText></TableCell>
                <TableCell className={`${styles.numericColumn} ${styles.compactColumn}`}>{song.sourceCount}</TableCell><TableCell className={`${styles.numericColumn} ${styles.compactColumn}`}>{song.lyricCount}</TableCell><TableCell className={`${styles.numericColumn} ${styles.compactColumn}`}>{formatDuration(song.durationMs)}</TableCell>
                <TableCell className={`${styles.actionColumn} ${styles.compactColumn}`}><Button size="sm" variant="outline" onClick={() => navigation.openDetail("songs", song.recordingId, song.title)}>{t("library.manager.details")}</Button></TableCell>
              </TableRow>
            ))}</TableBody>
          </Table>
        ) : <LibraryState state="empty" message={t("library.manager.emptySongs")} />}
        </CardContent>
        {data ? <PageControls disabled={loading || !!loadingError} page={page} pageSize={pageSize} total={data.total} onPageChange={(value) => view.update({ page: value })} onPageSizeChange={(value) => view.update({ pageSize: value, page: 1 })} /> : null}
      </Card>
    </div>
  );
}

function SongSimilarityQueue({ onBack }: { onBack: () => void }) {
  const { t } = useTranslation();
  const view = useLibraryViewState("songs");
  const page = view.similarPage;
  const pageSize = view.similarPageSize;
  const [pairs, setPairs] = useState<SongSimilarityPair[] | null>(null);
  const [keepers, setKeepers] = useState<Record<string, number>>({});
  const [busyPairId, setBusyPairId] = useState<string | null>(null);
  const [detailId, setDetailId] = useState<number | null>(null);
  const [detailConflictPlatforms, setDetailConflictPlatforms] = useState<string[]>([]);
  const [error, setError] = useState("");
  const [total, setTotal] = useState(0);
  const [progress, setProgress] = useState("");
  const [batchPreview, setBatchPreview] = useState<SongSimilarityBatchPreview | null>(null);
  const [batchOpen, setBatchOpen] = useState(false);
  const [batchPreviewBusy, setBatchPreviewBusy] = useState(false);
  const [batchApplying, setBatchApplying] = useState(false);
  const [refreshRevision, setRefreshRevision] = useState(0);
  const busyPairRef = useRef<string | null>(null);
  const requestSequence = useRef(0);

  const applyPairs = (value: SongSimilarityPair[]) => {
    setPairs(value);
    setKeepers(Object.fromEntries(value.map((pair) => [pair.pairId, pair.recommendedRecordingId])));
    setError("");
  };
  const refresh = async () => {
    const sequence = ++requestSequence.current;
    try {
      const value = await lyricsApi.listLibrarySongSimilarity(page, pageSize);
      if (sequence !== requestSequence.current) return;
      const lastPage = Math.max(1, Math.ceil(value.total / pageSize));
      if (page > lastPage) { view.update({ similarPage: lastPage }, true); return; }
      applyPairs(value.items);
      setTotal(value.total);
    }
    catch (reason) { if (sequence === requestSequence.current) { setError(messageOf(reason)); setPairs([]); } }
  };
  useEffect(() => { setPairs(null); void refresh(); return () => { requestSequence.current++; }; }, [page, pageSize, refreshRevision]);
  useEffect(() => {
    if (pairs !== null) return;
    const update = () => { void lyricsApi.getLibraryIndexStatus().then((value) => { const index = value.indexes.find((item) => item.indexKind === "song_similarity"); setProgress(index && index.total > 0 ? ` ${index.processed}/${index.total}` : ""); }).catch(() => undefined); };
    update();
    const timer = window.setInterval(update, 500);
    return () => window.clearInterval(timer);
  }, [pairs]);

  if (detailId !== null) {
    return <SongDetail recordingId={detailId} conflictPlatforms={detailConflictPlatforms} detailPath="/settings/library/songs" onBack={() => { setDetailId(null); setDetailConflictPlatforms([]); setRefreshRevision((value) => value + 1); }} />;
  }

  const dismiss = async (pair: SongSimilarityPair) => {
    if (busyPairRef.current !== null) return;
    busyPairRef.current = pair.pairId;
    setBusyPairId(pair.pairId);
    try {
      await lyricsApi.dismissLibrarySongSimilarity(pair.songs[0].recordingId, pair.songs[1].recordingId);
      setRefreshRevision((value) => value + 1);
      toast.success(t("library.manager.similarSongsKeptSeparate"));
    } catch (reason) { toast.error(messageOf(reason)); }
    finally { busyPairRef.current = null; setBusyPairId(null); }
  };
  const merge = async (pair: SongSimilarityPair) => {
    if (busyPairRef.current !== null) return;
    const keeper = keepers[pair.pairId];
    const redundant = pair.songs.find((song) => song.recordingId !== keeper);
    if (!keeper || !redundant) return;
    busyPairRef.current = pair.pairId;
    setBusyPairId(pair.pairId);
    try {
      await lyricsApi.mergeLibrarySong(keeper, redundant.recordingId);
      setRefreshRevision((value) => value + 1);
      toast.success(t("library.manager.similarSongsMerged"));
    } catch (reason) { toast.error(messageOf(reason)); }
    finally { busyPairRef.current = null; setBusyPairId(null); }
  };

  const previewBatch = async () => {
    if (batchPreviewBusy || batchApplying || busyPairRef.current !== null) return;
    setBatchPreviewBusy(true);
    try {
      setBatchPreview(await lyricsApi.previewLibrarySongSimilarityBatch());
      setBatchOpen(true);
    } catch (reason) { toast.error(messageOf(reason)); }
    finally { setBatchPreviewBusy(false); }
  };

  const applyBatch = async () => {
    if (!batchPreview || batchApplying) return;
    setBatchApplying(true);
    try {
      const result = await lyricsApi.applyLibrarySongSimilarityBatch(batchPreview.candidates);
      setBatchOpen(false);
      setBatchPreview(null);
      setRefreshRevision((value) => value + 1);
      const summary = t("library.manager.batchSimilarSongsDone", {
        merged: result.mergedCount,
        skipped: batchPreview.skippedCount + result.skippedCount,
        failed: result.failedCount,
      });
      if (result.failedCount) toast.error(summary);
      else toast.success(summary);
    } catch (reason) { toast.error(messageOf(reason)); }
    finally { setBatchApplying(false); }
  };

  return (
    <Card className={`${styles.panel} ${styles.queuePanel}`}>
      <LibraryDetailHeader title={t("library.manager.similarSongsTitle")} description={t("library.manager.similarSongsDescription")} onBack={onBack} />
      <CardContent className={styles.similarity}>
        <div className={styles.relationActions}>
          <Button size="sm" variant="outline" disabled={pairs === null || total === 0 || batchPreviewBusy || batchApplying || busyPairId !== null} onClick={() => void previewBatch()}>
            <GitMerge data-icon="inline-start" />{t("library.manager.batchSimilarSongs")}
          </Button>
        </div>
        {error ? <LibraryState state="error" message={error} /> : null}
        {pairs === null ? <LibraryState state="loading" message={`${t("library.manager.analyzingSimilarSongs")}${progress}`} /> : pairs.length === 0 && !error ? <LibraryState state="empty" message={t("library.manager.noSimilarSongs")} /> : pairs?.map((pair) => {
          const keeper = keepers[pair.pairId];
          const keeperSong = pair.songs.find((song) => song.recordingId === keeper);
          const redundantSong = pair.songs.find((song) => song.recordingId !== keeper);
          const conflictPlatforms = pair.evidence.canAssociate ? [] : conflictingPlatforms(pair);
          const conflictTarget = pair.songs
            .filter((song) => observationCount(song) > 1 && song.sources.some((source) => conflictPlatforms.some((platform) => normalizedPlatform(platform) === normalizedPlatform(source.platform))))
            .sort((left, right) => observationCount(right) - observationCount(left))[0];
          const busy = busyPairId !== null || batchApplying;
          return (
            <section className={styles.section} key={pair.pairId}>
              <div className={styles.sectionHeader}>
                <h3>{t("library.manager.similarSongsScore", { value: Math.round(pair.evidence.score * 100) })}</h3>
                <div className={styles.badges}>
                  <Badge variant={pair.evidence.kind === "originalRelation" ? "default" : pair.evidence.kind === "sharedIdentifier" ? "secondary" : "outline"}>{t(`library.manager.similarSongKinds.${pair.evidence.kind}`)}</Badge>
                  {!pair.evidence.canAssociate ? <Badge variant="destructive">{t("library.manager.mergeBlockedPlatforms", { platforms: conflictPlatforms.join(" / ") || "—" })}</Badge> : null}
                </div>
              </div>
              <ToggleGroup
                className={styles.songSimilarityChoices}
                variant="outline"
                value={keeper ? [String(keeper)] : []}
                onValueChange={(values) => { const selected = values[0]; if (!selected) return; const value = Number(selected); if (Number.isSafeInteger(value)) setKeepers((current) => ({ ...current, [pair.pairId]: value })); }}
                aria-label={t("library.manager.selectKeeper")}
              >
                {pair.songs.map((song) => (
                  <ToggleGroupItem className={styles.songSimilarityChoice} value={String(song.recordingId)} key={song.recordingId} aria-label={t("library.manager.keepSong", { title: song.title })}>
                    {song.recordingId === keeper ? <Check className={styles.similaritySelectionIcon} aria-hidden="true" /> : null}
                    <span className={styles.songSimilarityTitle}>{song.title}</span>
                    <span>{song.artists.join(" / ") || "—"}</span>
                    <span>{song.album || "—"} · {formatDuration(song.durationMs)}</span>
                    <span>{t("library.manager.songResourceSummary", { sources: song.sourceCount, lyrics: song.lyricCount })}</span>
                    <span>{t("library.manager.songSources")}: {song.sources.map((source) => songSourceLabel(source, t)).join(" / ") || "—"}</span>
                    {song.defaultLyric ? <span>{t("library.manager.defaultLyric")}: {song.defaultLyric}</span> : null}
                    {song.recordingId === pair.recommendedRecordingId ? <Badge>{t("library.manager.recommended")}</Badge> : null}
                  </ToggleGroupItem>
                ))}
              </ToggleGroup>
              <div className={styles.songSimilarityDetailsActions}>
                {pair.songs.map((song) => <Button size="sm" variant="outline" key={song.recordingId} onClick={() => { setDetailConflictPlatforms([]); setDetailId(song.recordingId); }}>{t("library.manager.openSongNamed", { title: song.title })}</Button>)}
              </div>
              <div className={styles.songSimilarityEvidence}>
                {pair.evidence.matchReasons.length ? <p>{pair.evidence.matchReasons.map((reason) => associationReasonLabel(reason, t)).join(" · ")}</p> : null}
                <CandidateEvidence candidate={pair.evidence} t={t} />
              </div>
              <div className={styles.relationActions}>
                <Button size="sm" variant="outline" disabled={busy} onClick={() => void dismiss(pair)}>{t("library.manager.notSameSong")}</Button>
                {!pair.evidence.canAssociate && conflictTarget ? <Button size="sm" variant="outline" disabled={busy} onClick={() => { setDetailConflictPlatforms(conflictPlatforms); setDetailId(conflictTarget.recordingId); }}><AlertTriangle data-icon="inline-start" />{t("library.manager.resolvePlatformConflict")}</Button> : null}
                {pair.evidence.canAssociate ? <ConfirmAction
                  title={t("library.manager.mergeSimilarSongsTitle")}
                  description={t("library.manager.mergeSimilarSongsDescription", { keeper: keeperSong?.title ?? "—", redundant: redundantSong?.title ?? "—" })}
                  label={t("library.manager.mergeSongs")}
                  triggerVariant="default"
                  confirmVariant="default"
                  disabled={busy || !keeperSong || !redundantSong}
                  onConfirm={() => merge(pair)}
                /> : null}
              </div>
            </section>
          );
        })}
      </CardContent>
      {pairs ? <PageControls page={page} pageSize={pageSize} total={total} onPageChange={(value) => view.update({ similarPage: value })} onPageSizeChange={(value) => view.update({ similarPageSize: value, similarPage: 1 })} /> : null}
      <Dialog open={batchOpen} onOpenChange={(open) => { if (!batchApplying) setBatchOpen(open); }}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("library.manager.batchSimilarSongsTitle")}</DialogTitle>
            <DialogDescription>{t("library.manager.batchSimilarSongsDescription", {
              merge: batchPreview?.candidates.length ?? 0,
              skip: batchPreview?.skippedCount ?? 0,
            })}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" disabled={batchApplying} onClick={() => setBatchOpen(false)}>{t("common.actions.cancel")}</Button>
            <Button disabled={batchApplying || !batchPreview?.candidates.length} onClick={() => void applyBatch()}>{t("library.manager.batchSimilarSongsConfirm")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}

function SongDetail({ recordingId, onBack: onBackOverride, conflictPlatforms = [], detailPath }: { recordingId: number; onBack?: () => void; conflictPlatforms?: string[]; detailPath?: string }) {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<LibrarySongDetail | null>(null);
  const [candidates, setCandidates] = useState<SongAssociationCandidate[]>([]);
  const [error, setError] = useState("");
  const [bindOpen, setBindOpen] = useState(false);
  const [mergeOpen, setMergeOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const navigation = useLibraryNavigation({
    section: "songs",
    sectionLabel: t("library.manager.tabs.songs"),
    detailsLabel: t("library.manager.details"),
    detailId: recordingId,
    detailLabel: detail?.recording.title,
    detailPath,
  });
  const onBack = onBackOverride ?? navigation.back;
  const breadcrumbs = onBackOverride ? undefined : navigation.breadcrumbs;

  const refresh = async () => {
    try {
      const value = await lyricsApi.getLibrarySong(recordingId);
      setDetail(value); setError("");
      setCandidates(value.recording.observations.length ? await lyricsApi.getLibrarySongCandidates(recordingId) : []);
    } catch (reason) { setError(messageOf(reason)); }
  };
  useEffect(() => { void refresh(); }, [recordingId]);

  if (!detail) return <Card className={`${styles.panel} ${styles.detailPanel}`}><LibraryDetailHeader title={t("library.manager.details")} breadcrumbs={breadcrumbs} onBack={onBack} /><CardContent><LibraryState state={error ? "error" : "loading"} message={error || t("library.manager.loading")} /></CardContent></Card>;
  const { recording } = detail;

  const unbind = async (assetId: number) => {
    try { setDetail(await lyricsApi.unbindLibraryLyric(recordingId, assetId)); toast.success(t("library.manager.unbound")); }
    catch (reason) { toast.error(messageOf(reason)); }
  };
  const setDefault = async (assetId: number) => {
    try { setDetail(await lyricsApi.bindLibraryLyric(recordingId, assetId, true)); toast.success(t("library.manager.defaultChanged")); }
    catch (reason) { toast.error(messageOf(reason)); }
  };
  const detach = async (observationId: number) => {
    try { setDetail(await lyricsApi.splitLibrarySong(recordingId, observationId)); toast.success(t("library.manager.detached")); await refresh(); }
    catch (reason) { toast.error(messageOf(reason)); }
  };
  const associate = async (candidate: SongAssociationCandidate) => {
    try { setDetail(await lyricsApi.mergeLibrarySong(recordingId, candidate.recordingId)); toast.success(t("library.manager.associated")); await refresh(); }
    catch (reason) { toast.error(messageOf(reason)); }
  };
  const remove = async () => {
    if (busy) return;
    setBusy(true);
    try {
      await lyricsApi.deleteLibrarySong(recordingId);
      toast.success(t("library.manager.songDeleted"));
      onBack();
    } catch (reason) { toast.error(messageOf(reason)); }
    finally { setBusy(false); }
  };

  return (
    <Card className={`${styles.panel} ${styles.detailPanel}`}>
      <LibraryDetailHeader
        title={recording.title}
        description={recording.artistCredits.map((credit) => credit.canonicalName).join(" / ")}
        actions={<div className={styles.relationActions}>
          <Button size="sm" variant="outline" disabled={busy} onClick={() => setMergeOpen(true)}><GitMerge data-icon="inline-start" />{t("library.manager.mergeIntoSong")}</Button>
          <ConfirmAction title={t("library.manager.deleteSongTitle")} description={t("library.manager.deleteSongDescription")} label={t("library.manager.deleteSong")} triggerVariant="destructive" disabled={busy} onConfirm={remove} />
        </div>}
        breadcrumbs={breadcrumbs}
        onBack={onBack}
      />
      <CardContent className={styles.detailBody}>
        {error ? <LibraryState state="error" message={error} /> : null}
        {conflictPlatforms.length ? <Alert variant="destructive"><AlertDescription>{t("library.manager.platformConflictResolution", { platforms: conflictPlatforms.join(" / ") })}</AlertDescription></Alert> : null}
        <div className={styles.metadataGrid}>
          <div><span>{t("library.manager.album")}</span><strong>{recording.album || "—"}</strong></div>
          <div><span>{t("library.manager.duration")}</span><strong>{formatDuration(recording.durationMs)}</strong></div>
          <div><span>{t("library.manager.version")}</span><strong>{recording.versionTags.join(" / ") || "—"}</strong></div>
          <div><span>{t("library.manager.externalIds")}</span><strong className={styles.externalIdentifiers}>{recording.externalIdentifiers.length ? recording.externalIdentifiers.map((item) => <span key={`${item.namespace}-${item.idKind}-${item.value}`}>{item.namespace}: {item.value}</span>) : "—"}</strong></div>
        </div>
        <LibraryDetailSection title={t("library.manager.artistIdentity")}>
          <LibraryRelationList>{recording.artistCredits.map((credit, index) => <LibraryRelationItem key={`${credit.rawName}-${index}`} title={credit.canonicalName} description={<>{t("library.manager.rawCredit")}: {credit.rawName}{credit.confirmedAliases.length ? ` · ${t("library.manager.aliases")}: ${credit.confirmedAliases.join(" / ")}` : ""}</>} actions={credit.artistId ? <Button size="sm" variant="outline" onClick={() => navigation.openDetail("artists", credit.artistId!, credit.canonicalName)}>{t("library.manager.manageArtist")}</Button> : null} />)}</LibraryRelationList>
        </LibraryDetailSection>
        <LibraryDetailSection title={t("library.manager.platformTracks")}>
          <LibraryRelationList>{recording.observations.map((item) => {
            const platformConflict = conflictPlatforms.some((platform) => normalizedPlatform(platform) === normalizedPlatform(item.platform));
            return <LibraryRelationItem key={item.observationId} title={<span className={styles.badges}>{songSourceLabel(item, t)} · {item.rawTitle}{platformConflict ? <Badge variant="destructive">{t("library.manager.platformConflictItem")}</Badge> : null}</span>} description={`${item.trackKey} · ${item.rawArtists.join(" / ")}`} actions={recording.observations.length > 1 ? <ConfirmAction title={t("library.manager.detachTitle")} description={t("library.manager.detachDescription")} label={t(platformConflict ? "library.manager.detachConflict" : "library.manager.detach")} triggerVariant="destructive" onConfirm={() => detach(item.observationId)} /> : null} />;
          })}</LibraryRelationList>
          {candidates.length ? <LibraryRelationList>{candidates.map((candidate) => <LibraryRelationItem key={candidate.recordingId} title={candidate.title} description={`${candidate.observations.map((item) => songSourceLabel(item, t)).join(" / ")} · ${Math.round(candidate.score * 100)}%`} actions={<ConfirmAction title={t("library.manager.associateTitle")} description={t("library.manager.associateDescription")} label={t("library.manager.associate")} triggerVariant="default" confirmVariant="default" disabled={!candidate.canAssociate} onConfirm={() => associate(candidate)} />} />)}</LibraryRelationList> : null}
        </LibraryDetailSection>
        <LibraryDetailSection title={t("library.manager.lyricResources")} actions={<Button size="sm" variant="default" onClick={() => setBindOpen(true)}><Plus data-icon="inline-start" />{t("library.manager.bindLyric")}</Button>}>
          {detail.bindings.length ? <LibraryRelationList>{detail.bindings.map((binding) => <LibraryRelationItem key={binding.bindingId} title={binding.asset?.sourceName ?? `#${binding.assetId}`} description={`${binding.isDefault ? t("library.manager.defaultLyric") : t("library.manager.candidateLyric")} · ${binding.offsetMs} ms`} actions={<>{!binding.isDefault ? <Button size="sm" variant="secondary" onClick={() => void setDefault(binding.assetId)}><Link2 data-icon="inline-start" />{t("library.manager.setDefault")}</Button> : null}<Button size="sm" variant="outline" onClick={() => navigation.openDetail("lyrics", binding.assetId, binding.asset?.sourceName ?? `#${binding.assetId}`)}>{t("library.manager.preview")}</Button><ConfirmAction title={t("library.manager.unbindTitle")} description={t("library.manager.unbindDescription")} label={t("library.manager.unbind")} triggerVariant="destructive" onConfirm={() => unbind(binding.assetId)} /></>} />)}</LibraryRelationList> : <LibraryState state="empty" message={t("library.manager.noBoundLyrics")} />}
          {detail.platformOverrides.length ? <div className={styles.badges}>{detail.platformOverrides.map((item) => <Badge key={item.overrideId} variant="outline">{item.platform}: {item.asset?.sourceName ?? t("library.manager.none")}</Badge>)}</div> : null}
        </LibraryDetailSection>
      </CardContent>
      <MergeSongDialog
        open={mergeOpen}
        onOpenChange={setMergeOpen}
        source={recording}
        onMerged={(targetRecordingId, targetTitle) => {
          setMergeOpen(false);
          navigation.openDetail("songs", targetRecordingId, targetTitle, { replace: true, replaceCurrent: true });
        }}
      />
      <BindLyricDialog open={bindOpen} onOpenChange={setBindOpen} recordingId={recordingId} onBound={(value) => setDetail(value)} />
    </Card>
  );
}

function MergeSongDialog({ open, onOpenChange, source, onMerged }: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  source: LibrarySongDetail["recording"];
  onMerged: (targetRecordingId: number, targetTitle: string) => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState(source.title);
  const [items, setItems] = useState<LibrarySongSummary[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    setQuery(source.title);
    setSelected(null);
  }, [open, source.recordingId, source.title]);
  useEffect(() => {
    if (!open) return;
    let live = true;
    setLoading(true);
    const timer = window.setTimeout(() => {
      lyricsApi.listLibrarySongs(query, 1, 20).then((value) => {
        if (!live) return;
        setItems(value.items.filter((item) => item.recordingId !== source.recordingId));
        setError("");
      }).catch((reason) => {
        if (!live) return;
        setItems([]);
        setError(messageOf(reason));
      }).finally(() => { if (live) setLoading(false); });
    }, 180);
    return () => { live = false; window.clearTimeout(timer); };
  }, [open, query, source.recordingId]);

  const target = items.find((item) => item.recordingId === selected);
  const merge = async () => {
    if (!target || busy) return;
    setBusy(true);
    try {
      await lyricsApi.mergeLibrarySong(target.recordingId, source.recordingId, true);
      toast.success(t("library.manager.songMergedInto", { title: target.title }));
      onMerged(target.recordingId, target.title);
    } catch (reason) { toast.error(messageOf(reason)); }
    finally { setBusy(false); }
  };

  return <Dialog open={open} onOpenChange={(value) => { if (!busy) onOpenChange(value); }}>
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{t("library.manager.mergeIntoSongTitle")}</DialogTitle>
        <DialogDescription>{t("library.manager.mergeIntoSongDescription")}</DialogDescription>
      </DialogHeader>
      <Field>
        <FieldLabel htmlFor="merge-song-search" className="sr-only">{t("library.manager.search")}</FieldLabel>
        <InputGroup><InputGroupAddon><Search data-icon="inline-start" /></InputGroupAddon><InputGroupInput id="merge-song-search" value={query} onChange={(event) => { setQuery(event.target.value); setSelected(null); }} placeholder={t("library.manager.searchPlaceholder")} /></InputGroup>
      </Field>
      {error ? <LibraryState state="error" message={error} /> : loading ? <LibraryState state="loading" message={t("library.manager.loading")} /> : items.length ? <ItemGroup className={styles.dialogItemList}>{items.map((item) => <Item render={<button type="button" />} variant="outline" className={styles.selectableItem} data-selected={selected === item.recordingId} aria-pressed={selected === item.recordingId} key={item.recordingId} onClick={() => setSelected(item.recordingId)}>
        <ItemContent><ItemTitle>{item.title}</ItemTitle><ItemDescription>{item.artists.join(" / ") || "—"} · {item.album || "—"} · {formatDuration(item.durationMs)} · {t("library.manager.songResourceSummary", { sources: item.sourceCount, lyrics: item.lyricCount })}</ItemDescription></ItemContent>
        {selected === item.recordingId ? <ItemActions><Check className={styles.checkRowSelectionIcon} aria-hidden="true" /></ItemActions> : null}
      </Item>)}</ItemGroup> : <LibraryState state="empty" message={t("library.manager.noMergeTargets")} />}
      <DialogFooter>
        <Button variant="outline" disabled={busy} onClick={() => onOpenChange(false)}>{t("common.actions.cancel")}</Button>
        <ConfirmAction
          title={t("library.manager.confirmManualMergeTitle")}
          description={t("library.manager.confirmManualMergeDescription", { source: source.title, target: target?.title ?? "—" })}
          label={t("library.manager.confirmManualMerge")}
          triggerVariant="default"
          confirmVariant="default"
          disabled={!target || loading || busy}
          onConfirm={merge}
        />
      </DialogFooter>
    </DialogContent>
  </Dialog>;
}

function BindLyricDialog({ open, onOpenChange, recordingId, onBound }: { open: boolean; onOpenChange: (open: boolean) => void; recordingId: number; onBound: (detail: LibrarySongDetail) => void }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<LibraryLyricSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [selected, setSelected] = useState<number | null>(null);
  const [replace, setReplace] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  useEffect(() => {
    if (!open) return;
    setQuery(""); setItems([]); setTotal(0); setSelected(null); setReplace(false); setError("");
  }, [open]);
  useEffect(() => {
    if (!open) return;
    let live = true;
    setLoading(true);
    setError("");
    const timer = window.setTimeout(() => lyricsApi.listLibraryLyrics(query).then((value) => {
      if (!live) return;
      setItems(value.items); setTotal(value.total);
    }).catch((reason) => {
      if (!live) return;
      setItems([]); setTotal(0); setError(messageOf(reason));
    }).finally(() => { if (live) setLoading(false); }), 180);
    return () => { live = false; window.clearTimeout(timer); };
  }, [open, query]);
  const bind = async () => {
    if (!selected || busyRef.current) return;
    busyRef.current = true; setBusy(true);
    try { onBound(await lyricsApi.bindLibraryLyric(recordingId, selected, replace)); onOpenChange(false); toast.success(t("library.manager.bound")); }
    catch (reason) { toast.error(messageOf(reason)); }
    finally { busyRef.current = false; setBusy(false); }
  };
  return <Dialog open={open} onOpenChange={(value) => { if (!busyRef.current) onOpenChange(value); }}><DialogContent><DialogHeader><DialogTitle>{t("library.manager.bindLyric")}</DialogTitle><DialogDescription>{t("library.manager.bindRule")}</DialogDescription></DialogHeader><Field><FieldLabel htmlFor="bind-lyric-search" className="sr-only">{t("library.manager.search")}</FieldLabel><InputGroup><InputGroupAddon><Search data-icon="inline-start" /></InputGroupAddon><InputGroupInput id="bind-lyric-search" disabled={busy} value={query} onChange={(event) => { setQuery(event.target.value); setSelected(null); }} placeholder={t("library.manager.searchPlaceholder")} /></InputGroup></Field>{error ? <LibraryState state="error" message={error} /> : null}{loading ? <LibraryState state="loading" message={t("library.manager.loading")} /> : !error && !items.length ? <LibraryState state="empty" message={t("library.manager.noBindMatches")} /> : !error ? <ItemGroup className={styles.dialogItemList} inert={busy}>{items.map((item) => <Item render={<button type="button" />} variant="outline" className={styles.selectableItem} data-selected={selected === item.assetId} aria-pressed={selected === item.assetId} key={item.assetId} onClick={() => setSelected(item.assetId)}><ItemContent><ItemTitle>{item.title}</ItemTitle><ItemDescription>{item.artist}</ItemDescription></ItemContent>{selected === item.assetId ? <ItemActions><Check className={styles.checkRowSelectionIcon} aria-hidden="true" /></ItemActions> : null}</Item>)}</ItemGroup> : null}{!loading && !error && total > items.length ? <p className={styles.dialogHint}>{t("library.manager.bindResultsLimited", { count: items.length, total })}</p> : null}<Item render={<label />} variant="outline" className={styles.checkboxItem}><Checkbox disabled={busy} checked={replace} onCheckedChange={(value) => setReplace(value === true)} /><ItemContent><ItemTitle>{t("library.manager.replaceDefault")}</ItemTitle></ItemContent></Item><DialogFooter><Button variant="outline" disabled={busy} onClick={() => onOpenChange(false)}>{t("common.actions.cancel")}</Button><Button disabled={!selected || loading || busy} onClick={() => void bind()}><Link2 data-icon="inline-start" />{t(busy ? "library.manager.binding" : "library.manager.bind")}</Button></DialogFooter></DialogContent></Dialog>;
}
