import { useEffect, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { Check, Link2, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemTitle } from "@/components/ui/item";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { lyricsApi } from "@/shared/api/lyrics";
import { messageOf } from "@/shared/api";
import { findAlignedAuxiliaryLine } from "@/features/lyrics/useLyrics/display";
import type { LyricsDocument } from "@/shared/types/lyrics";
import type { LibraryLyricDetail, LibraryLyricPage, LibraryLyricSource, LibraryLyricStatus, LibrarySongSummary, LibraryLyricSummary, LyricSimilarityGroup } from "@/shared/types/lyrics";
import { ConfirmAction, formatBytes, LibraryDetailHeader, LibraryDetailSection, LibraryRefreshing, LibraryRelationItem, LibraryRelationList, LibraryState, LibraryToolbar, PageControls, TruncatedText, useLibraryNavigation } from "./shared";
import { useLibraryViewState } from "./viewState";
import styles from "./library.module.scss";

const lyricSourceKinds = ["all", "managed", "cache", "local", "legacy"] as const;

export default function LyricsLibrary({ detailId, onStatusCountsChange, similarityOpen, onSimilarityClose }: {
  detailId: number | null;
  onStatusCountsChange: (counts: Record<LibraryLyricStatus, number>) => void;
  similarityOpen: boolean;
  onSimilarityClose: () => void;
}) {
  const { t } = useTranslation();
  const navigation = useLibraryNavigation({
    section: "lyrics",
    sectionLabel: t("library.manager.tabs.lyrics"),
    detailsLabel: t("library.manager.details"),
    detailId: null,
  });
  const view = useLibraryViewState("lyrics");
  const { query, status, sourceKind, page, pageSize } = view;
  const [data, setData] = useState<LibraryLyricPage | null>(null);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [batchBusy, setBatchBusy] = useState(false);
  const [error, setError] = useState("");
  const [refreshRevision, setRefreshRevision] = useState(0);
  const [countsRevision, setCountsRevision] = useState(0);
  const requestSequence = useRef(0);
  const batchBusyRef = useRef(false);

  const refresh = () => {
    const sequence = ++requestSequence.current;
    let correctingPage = false;
    setLoading(true);
    setError("");
    return lyricsApi.listLibraryLyrics(query, status === "all" ? null : status, sourceKind === "all" ? null : sourceKind, page, pageSize).then((value) => {
      if (sequence !== requestSequence.current) return;
      const lastPage = Math.max(1, Math.ceil(value.total / pageSize));
      if (page > lastPage) { correctingPage = true; view.update({ page: lastPage }, true); return; }
      setData(value); setError("");
      onStatusCountsChange(value.statusCounts);
    }).catch((reason) => {
      if (sequence === requestSequence.current) setError(messageOf(reason));
    }).finally(() => { if (sequence === requestSequence.current && !correctingPage) setLoading(false); });
  };
  useEffect(() => {
    if (detailId !== null || similarityOpen) return;
    requestSequence.current++;
    setLoading(true);
    const timer = window.setTimeout(() => void refresh(), 180);
    return () => { requestSequence.current++; window.clearTimeout(timer); };
  }, [query, status, sourceKind, page, pageSize, detailId, similarityOpen, refreshRevision]);
  useEffect(() => {
    setSelected(new Set());
  }, [query, status, sourceKind, page, pageSize, detailId]);
  useEffect(() => {
    if (!similarityOpen || detailId !== null) return;
    let live = true;
    void lyricsApi.listLibraryLyrics(query, status === "all" ? null : status, sourceKind === "all" ? null : sourceKind, 1, 1)
      .then((value) => { if (live) onStatusCountsChange(value.statusCounts); })
      .catch((reason) => { if (live) toast.error(messageOf(reason)); });
    return () => { live = false; };
  }, [similarityOpen, detailId, query, status, sourceKind, countsRevision, onStatusCountsChange]);
  if (detailId) return <LyricDetail assetId={detailId} />;
  if (similarityOpen) return <SimilarityQueue onBack={onSimilarityClose} onChanged={() => setCountsRevision((value) => value + 1)} />;
  const selectionEnabled = status === "candidate" || status === "unbound";
  const visibleIds = data?.items
    .filter((item) => status === "candidate" || (status === "unbound" && item.canCleanup))
    .map((item) => item.assetId) ?? [];
  const allVisibleSelected = visibleIds.length > 0 && visibleIds.every((id) => selected.has(id));

  const clearCandidates = async () => {
    if (batchBusy || batchBusyRef.current || !selected.size) return;
    batchBusyRef.current = true;
    setBatchBusy(true);
    try {
      const result = await lyricsApi.clearLibraryLyricCandidates([...selected]);
      toast.success(t("library.manager.candidatesCleared", { count: result.processedAssets, bindings: result.removedBindings }));
      result.failures.forEach((item) => toast.error(`#${item.assetId}: ${item.error}`));
      setSelected(new Set());
      setRefreshRevision((value) => value + 1);
    } catch (reason) { toast.error(messageOf(reason)); }
    finally { batchBusyRef.current = false; setBatchBusy(false); }
  };
  const cleanupSelected = async () => {
    if (batchBusy || batchBusyRef.current || !selected.size) return;
    batchBusyRef.current = true;
    setBatchBusy(true);
    try {
      const result = await lyricsApi.cleanupSelectedUnboundLyrics([...selected]);
      toast.success(t("library.manager.cleanupDone", { count: result.deletedFiles, size: formatBytes(result.releasedBytes) }));
      result.items.forEach((item) => toast.error(`#${item.assetId}: ${item.error ?? t("errors.command")}`));
      setSelected(new Set());
      setRefreshRevision((value) => value + 1);
    } catch (reason) { toast.error(messageOf(reason)); }
    finally { batchBusyRef.current = false; setBatchBusy(false); }
  };
  const batchActions = selected.size > 0 && status === "candidate" ? <ConfirmAction
    title={t("library.manager.clearCandidatesTitle")}
    description={t("library.manager.clearCandidatesDescription", { count: selected.size })}
    label={t("library.manager.clearCandidates", { count: selected.size })}
    triggerVariant="destructive"
    disabled={batchBusy}
    onConfirm={clearCandidates}
  /> : selected.size > 0 && status === "unbound" ? <ConfirmAction
    title={t("library.manager.cleanupSelectedTitle")}
    description={t("library.manager.cleanupSelectedDescription", { count: selected.size })}
    label={t("library.manager.cleanupSelected", { count: selected.size })}
    triggerVariant="destructive"
    disabled={batchBusy}
    onConfirm={cleanupSelected}
  /> : null;

  return (
    <div className={styles.workspace}>
      <Card className={`${styles.panel} ${styles.listPanel}`}>
        <LibraryToolbar
          query={query}
          onQueryChange={(value) => view.update({ query: value, page: 1 }, true)}
          filters={<Select value={sourceKind} onValueChange={(value) => { if (value) view.update({ sourceKind: value as typeof sourceKind, page: 1 }); }}><SelectTrigger><SelectValue>{t(`library.manager.source.${sourceKind}`)}</SelectValue></SelectTrigger><SelectContent><SelectGroup>{lyricSourceKinds.map((value) => <SelectItem value={value} key={value}>{t(`library.manager.source.${value}`)}</SelectItem>)}</SelectGroup></SelectContent></Select>}
        />
        <CardContent className={styles.tableContent} aria-busy={loading}>
        {loading && data ? <LibraryRefreshing /> : null}
        {error ? <LibraryState state="error" message={error} /> : data === null ? <LibraryState state="loading" message={t("library.manager.loading")} /> : data.items.length ? <Table inert={loading} data-selection={selectionEnabled ? "true" : "false"} className={`${styles.adaptiveTable} ${styles.dataTable}`}><colgroup>{selectionEnabled ? <col className={styles.lyricSelectionColumn} /> : null}<col className={styles.lyricTitleColumn} /><col className={styles.lyricStatusColumn} /><col className={styles.lyricCapabilitiesColumn} /><col className={styles.lyricSizeColumn} /><col className={styles.lyricActionsColumn} /></colgroup><TableHeader><TableRow>{selectionEnabled ? <TableHead className={`${styles.selectionColumn} ${styles.compactColumn}`}>{visibleIds.length ? <Checkbox checked={allVisibleSelected} onCheckedChange={(checked) => setSelected(checked ? new Set(visibleIds) : new Set())} aria-label={t("library.manager.selectAll")} /> : null}</TableHead> : null}<TableHead className={styles.growColumn}>{t("library.manager.lyric")}</TableHead><TableHead className={styles.compactColumn}>{t("library.manager.statusLabel")}</TableHead><TableHead className={styles.compactColumn}>{t("library.manager.capabilities")}</TableHead><TableHead className={`${styles.numericColumn} ${styles.fileSizeColumn} ${styles.compactColumn}`}>{t("library.manager.size")}</TableHead><TableHead className={`${styles.actionColumn} ${styles.compactColumn}`}>{t("library.manager.actions")}</TableHead></TableRow></TableHeader><TableBody>{data.items.map((item) => {
          const selectable = status === "candidate" || (status === "unbound" && item.canCleanup);
          return <TableRow key={item.assetId} data-state={selected.has(item.assetId) ? "selected" : undefined}>{selectionEnabled ? <TableCell className={`${styles.selectionColumn} ${styles.compactColumn}`}>{selectable ? <Checkbox checked={selected.has(item.assetId)} onCheckedChange={(checked) => setSelected((current) => { const next = new Set(current); if (checked) next.add(item.assetId); else next.delete(item.assetId); return next; })} aria-label={item.title} /> : null}</TableCell> : null}<TableCell className={`${styles.primaryCell} ${styles.growColumn}`}><TruncatedText variant="title">{item.title}</TruncatedText><TruncatedText variant="meta">{`${item.artist} · ${item.sourceName}`}</TruncatedText></TableCell><TableCell className={styles.compactColumn}><StatusBadge item={item} t={t} /></TableCell><TableCell className={styles.compactColumn}><div className={styles.badges}>{item.hasWordTiming ? <Badge variant="outline">{t("common.feature.wordTiming")}</Badge> : null}{item.hasTranslation ? <Badge variant="outline">{t("common.feature.translation")}</Badge> : null}{item.hasRomanization ? <Badge variant="outline">{t("common.feature.romanization")}</Badge> : null}</div></TableCell><TableCell className={`${styles.numericColumn} ${styles.fileSizeColumn} ${styles.compactColumn}`}>{formatBytes(item.fileSize)}</TableCell><TableCell className={`${styles.actionColumn} ${styles.compactColumn}`}><Button size="sm" variant="outline" onClick={() => navigation.openDetail("lyrics", item.assetId, item.title)}>{t("library.manager.details")}</Button></TableCell></TableRow>;
        })}</TableBody></Table> : <LibraryState state="empty" message={t("library.manager.emptyLyrics")} />}
        </CardContent>
        {data ? <PageControls disabled={loading || !!error} page={page} pageSize={pageSize} total={data.total} actions={batchActions} onPageChange={(value) => view.update({ page: value })} onPageSizeChange={(value) => view.update({ pageSize: value, page: 1 })} /> : null}
      </Card>
    </div>
  );
}

function StatusBadge({ item, t }: { item: LibraryLyricSummary; t: TFunction }) {
  const variant = item.status === "inUse" ? "default" : item.status === "candidate" ? "secondary" : "outline";
  return <span className={styles.badges}><Badge variant={variant}>{t(`library.manager.status.${item.status}`)}</Badge>{!item.available ? <Badge variant="destructive">{t("library.manager.unavailable")}</Badge> : null}</span>;
}

function LyricDetail({ assetId }: { assetId: number }) {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<LibraryLyricDetail | null>(null);
  const [bindOpen, setBindOpen] = useState(false);
  const [formattedPreview, setFormattedPreview] = useState(true);
  const [error, setError] = useState("");
  const navigation = useLibraryNavigation({
    section: "lyrics",
    sectionLabel: t("library.manager.tabs.lyrics"),
    detailsLabel: t("library.manager.details"),
    detailId: assetId,
    detailLabel: detail?.summary.title,
  });
  const onBack = navigation.back;
  const refresh = () => lyricsApi.getLibraryLyric(assetId).then((value) => { setDetail(value); setError(""); }).catch((reason) => setError(messageOf(reason)));
  useEffect(() => { void refresh(); }, [assetId]);
  const unbind = async (recordingId: number) => { try { await lyricsApi.unbindLibraryLyric(recordingId, assetId); await refresh(); toast.success(t("library.manager.unbound")); } catch (reason) { toast.error(messageOf(reason)); } };
  const removeSource = async (sourceId: number) => { try { setDetail(await lyricsApi.deleteLibraryLyricSource(assetId, sourceId)); toast.success(t("library.manager.sourceDeleted")); } catch (reason) { toast.error(messageOf(reason)); } };
  if (!detail) return <Card className={`${styles.panel} ${styles.detailPanel}`}><LibraryDetailHeader title={t("library.manager.details")} breadcrumbs={navigation.breadcrumbs} onBack={onBack} /><CardContent><LibraryState state={error ? "error" : "loading"} message={error || t("library.manager.loading")} /></CardContent></Card>;
  return <Card className={`${styles.panel} ${styles.detailPanel}`}><LibraryDetailHeader title={detail.summary.title} description={`${detail.summary.artist} · ${detail.summary.sourceName}`} status={<StatusBadge item={detail.summary} t={t} />} actions={<Button size="sm" variant="default" onClick={() => setBindOpen(true)}><Link2 data-icon="inline-start" />{t("library.manager.bindSong")}</Button>} breadcrumbs={navigation.breadcrumbs} onBack={onBack} /><CardContent className={styles.detailBody}>{error ? <LibraryState state="error" message={error} /> : null}<div className={`${styles.metadataGrid} ${styles.lyricMetadataGrid}`}><div><span>{t("library.manager.format")}</span><strong>{detail.summary.originalFormat} · {detail.summary.language}</strong></div><div><span>{t("library.manager.capabilities")}</span><strong>{[detail.summary.hasWordTiming && t("common.feature.wordTiming"), detail.summary.hasTranslation && t("common.feature.translation"), detail.summary.hasRomanization && t("common.feature.romanization")].filter(Boolean).join(" / ") || t("common.feature.plainText")}</strong></div><div><span>{t("library.manager.fingerprint")}</span><strong>{detail.summary.contentFingerprint.slice(0, 16)}…</strong></div></div><LibraryDetailSection title={t("library.manager.boundSongs")}>{detail.recordings.length ? <LibraryRelationList>{detail.recordings.map((song) => <LibraryRelationItem key={song.recordingId} title={song.title} description={`${song.artists.join(" / ")} · ${song.isDefault ? t("library.manager.defaultLyric") : t("library.manager.candidateLyric")}`} actions={<><Button size="sm" variant="outline" onClick={() => navigation.openDetail("songs", song.recordingId, song.title)}>{t("library.manager.openSong")}</Button><ConfirmAction title={t("library.manager.unbindTitle")} description={t("library.manager.unbindDescription")} label={t("library.manager.unbind")} triggerVariant="destructive" onConfirm={() => unbind(song.recordingId)} /></>} />)}</LibraryRelationList> : <LibraryState state="empty" message={t("library.manager.noBoundSongs")} />}</LibraryDetailSection><LibraryDetailSection title={t("library.manager.physicalSources")}><LibraryRelationList>{detail.sources.map((source, index) => <LibraryRelationItem key={source.sourceId ?? index} title={source.sourceName} description={`${source.rootName || source.sourceKind} · ${source.relativePath || t("library.manager.memorySource")} · ${formatBytes(source.fileSize)}`} actions={<LyricSourceAction detail={detail} source={source} onRemove={removeSource} t={t} />} />)}</LibraryRelationList></LibraryDetailSection><LibraryDetailSection title={t("library.manager.lyricPreview")} actions={<PreviewModeSwitch formatted={formattedPreview} onCheckedChange={setFormattedPreview} t={t} />}><LyricsPreview document={detail.document} formatted={formattedPreview} t={t} /></LibraryDetailSection></CardContent><BindSongDialog open={bindOpen} onOpenChange={setBindOpen} assetId={assetId} onBound={() => void refresh()} /></Card>;
}

function LyricSourceAction({ detail, source, onRemove, t }: {
  detail: LibraryLyricDetail;
  source: LibraryLyricSource;
  onRemove: (sourceId: number) => Promise<void>;
  t: TFunction;
}) {
  if (!source.writable || source.sourceId === null) {
    return <Badge variant="outline">{t("library.manager.readOnly")}</Badge>;
  }

  const isPrimarySource = detail.asset.rootId === source.rootId
    && detail.asset.relativePath === source.relativePath;
  const hasReadableReplacement = detail.sources.some((candidate) => (
    candidate.sourceId !== source.sourceId && candidate.available
  ));
  if ((source.available || isPrimarySource) && !hasReadableReplacement) {
    return <Badge variant="outline">{t("library.manager.requiredSource")}</Badge>;
  }

  return <ConfirmAction title={t("library.manager.deleteSourceTitle")} description={t("library.manager.deleteSourceDescription")} label={t("common.actions.remove")} triggerVariant="destructive" onConfirm={() => onRemove(source.sourceId!)} />;
}

function BindSongDialog({ open, onOpenChange, assetId, onBound }: { open: boolean; onOpenChange: (open: boolean) => void; assetId: number; onBound: () => void }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<LibrarySongSummary[]>([]);
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
    const timer = window.setTimeout(() => lyricsApi.listLibrarySongs(query).then((value) => {
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
    try { await lyricsApi.bindLibraryLyric(selected, assetId, replace); toast.success(t("library.manager.bound")); onOpenChange(false); onBound(); }
    catch (reason) { toast.error(messageOf(reason)); }
    finally { busyRef.current = false; setBusy(false); }
  };
  return <Dialog open={open} onOpenChange={(value) => { if (!busyRef.current) onOpenChange(value); }}><DialogContent><DialogHeader><DialogTitle>{t("library.manager.bindSong")}</DialogTitle><DialogDescription>{t("library.manager.bindRule")}</DialogDescription></DialogHeader><Field><FieldLabel htmlFor="bind-song-search" className="sr-only">{t("library.manager.search")}</FieldLabel><InputGroup><InputGroupAddon><Search data-icon="inline-start" /></InputGroupAddon><InputGroupInput id="bind-song-search" disabled={busy} value={query} onChange={(event) => { setQuery(event.target.value); setSelected(null); }} placeholder={t("library.manager.searchPlaceholder")} /></InputGroup></Field>{error ? <LibraryState state="error" message={error} /> : null}{loading ? <LibraryState state="loading" message={t("library.manager.loading")} /> : !error && !items.length ? <LibraryState state="empty" message={t("library.manager.noBindMatches")} /> : !error ? <ItemGroup className={styles.dialogItemList} inert={busy}>{items.map((item) => <Item render={<button type="button" />} variant="outline" className={styles.selectableItem} key={item.recordingId} data-selected={selected === item.recordingId} aria-pressed={selected === item.recordingId} onClick={() => setSelected(item.recordingId)}><ItemContent><ItemTitle>{item.title}</ItemTitle><ItemDescription>{item.artists.join(" / ")}</ItemDescription></ItemContent>{selected === item.recordingId ? <ItemActions><Check className={styles.checkRowSelectionIcon} aria-hidden="true" /></ItemActions> : null}</Item>)}</ItemGroup> : null}{!loading && !error && total > items.length ? <p className={styles.dialogHint}>{t("library.manager.bindResultsLimited", { count: items.length, total })}</p> : null}<Item render={<label />} variant="outline" className={styles.checkboxItem}><Checkbox disabled={busy} checked={replace} onCheckedChange={(value) => setReplace(value === true)} /><ItemContent><ItemTitle>{t("library.manager.replaceDefault")}</ItemTitle></ItemContent></Item><DialogFooter><Button variant="outline" disabled={busy} onClick={() => onOpenChange(false)}>{t("common.actions.cancel")}</Button><Button disabled={!selected || loading || busy} onClick={() => void bind()}><Link2 data-icon="inline-start" />{t(busy ? "library.manager.binding" : "library.manager.bind")}</Button></DialogFooter></DialogContent></Dialog>;
}

function SimilarityQueue({ onBack, onChanged }: { onBack: () => void; onChanged: () => void }) {
  const { t } = useTranslation();
  const view = useLibraryViewState("lyrics");
  const page = view.similarPage;
  const pageSize = view.similarPageSize;
  const [groups, setGroups] = useState<LyricSimilarityGroup[] | null>(null);
  const [previews, setPreviews] = useState<Record<number, LyricsDocument | null>>({});
  const [keepers, setKeepers] = useState<Record<string, number>>({});
  const [formattedPreview, setFormattedPreview] = useState(true);
  const [total, setTotal] = useState(0);
  const [progress, setProgress] = useState("");
  const [refreshRevision, setRefreshRevision] = useState(0);
  const requestSequence = useRef(0);
  const refresh = async () => {
    const sequence = ++requestSequence.current;
    try {
      const value = await lyricsApi.listLibraryLyricSimilarity(page, pageSize);
      if (sequence !== requestSequence.current) return;
      const lastPage = Math.max(1, Math.ceil(value.total / pageSize));
      if (page > lastPage) { view.update({ similarPage: lastPage }, true); return; }
      setGroups(value.items);
      setTotal(value.total);
      setPreviews(value.previews);
      setKeepers(Object.fromEntries(value.items.map((group) => [group.groupId, group.recommendedAssetId])));
    } catch (reason) { if (sequence === requestSequence.current) toast.error(messageOf(reason)); }
  };
  useEffect(() => { setGroups(null); void refresh(); return () => { requestSequence.current++; }; }, [page, pageSize, refreshRevision]);
  useEffect(() => { if (groups !== null) return; const update = () => { void lyricsApi.getLibraryIndexStatus().then((value) => { const index = value.indexes.find((item) => item.indexKind === "lyric_similarity"); setProgress(index && index.total > 0 ? ` ${index.processed}/${index.total}` : ""); }).catch(() => undefined); }; update(); const timer = window.setInterval(update, 500); return () => window.clearInterval(timer); }, [groups]);
  const dismiss = async (group: LyricSimilarityGroup) => { try { await lyricsApi.dismissLibraryLyricSimilarity(group.items.map((item) => item.assetId)); setRefreshRevision((value) => value + 1); onChanged(); } catch (reason) { toast.error(messageOf(reason)); } };
  const merge = async (group: LyricSimilarityGroup) => { const keeper = keepers[group.groupId]; if (!keeper) return; try { await lyricsApi.mergeLibraryLyrics(keeper, group.items.map((item) => item.assetId).filter((id) => id !== keeper)); setRefreshRevision((value) => value + 1); onChanged(); toast.success(t("library.manager.merged")); } catch (reason) { toast.error(messageOf(reason)); } };
  return <Card className={`${styles.panel} ${styles.queuePanel}`}><LibraryDetailHeader title={t("library.manager.similarTitle")} description={t("library.manager.similarDescription")} actions={<PreviewModeSwitch formatted={formattedPreview} onCheckedChange={setFormattedPreview} t={t} />} onBack={onBack} /><CardContent className={styles.similarity}>{groups === null ? <LibraryState state="loading" message={`${t("library.manager.analyzing")}${progress}`} /> : groups.length === 0 ? <LibraryState state="empty" message={t("library.manager.noSimilar")} /> : groups.map((group) => <section className={styles.section} key={group.groupId}><div className={styles.sectionHeader}><h3>{Math.round(group.score * 100)}% · {group.highSimilarity ? t("library.manager.highSimilarity") : t("library.manager.similar")}</h3><div className={styles.badges}>{group.durationWarning ? <Badge variant="destructive">{t("library.manager.durationConflict")}</Badge> : null}{group.versionWarning ? <Badge variant="destructive">{t("library.manager.versionConflict")}</Badge> : null}</div></div><ToggleGroup className={styles.similarityGrid} variant="outline" value={keepers[group.groupId] ? [String(keepers[group.groupId])] : []} onValueChange={(values) => { const value = Number(values[0]); if (Number.isSafeInteger(value)) setKeepers((current) => ({ ...current, [group.groupId]: value })); }} aria-label={t("library.manager.selectKeeper")}>{group.items.map((item) => <ToggleGroupItem className={styles.similarityItem} value={String(item.assetId)} data-recommended={item.assetId === group.recommendedAssetId} key={item.assetId}>{keepers[group.groupId] === item.assetId ? <Check className={styles.similaritySelectionIcon} aria-hidden="true" /> : null}<strong>{item.title}</strong><span>{item.artist} · {item.sourceName}</span>{item.assetId === group.recommendedAssetId ? <Badge>{t("library.manager.recommended")}</Badge> : null}<LyricsPreview compact document={previews[item.assetId]} formatted={formattedPreview} t={t} /></ToggleGroupItem>)}</ToggleGroup><div className={styles.relationActions}><Button size="sm" variant="outline" onClick={() => void dismiss(group)}>{t("library.manager.notDuplicate")}</Button><ConfirmAction title={t("library.manager.mergeTitle")} description={t("library.manager.mergeDescription")} label={t("library.manager.merge")} triggerVariant="default" confirmVariant="default" onConfirm={() => merge(group)} /></div></section>)}</CardContent>{groups ? <PageControls page={page} pageSize={pageSize} total={total} onPageChange={(value) => view.update({ similarPage: value })} onPageSizeChange={(value) => view.update({ similarPageSize: value, similarPage: 1 })} /> : null}</Card>;
}

function PreviewModeSwitch({ formatted, onCheckedChange, t }: { formatted: boolean; onCheckedChange: (checked: boolean) => void; t: TFunction }) {
  return <label className={styles.previewModeSwitch}><span>{t(`library.manager.${formatted ? "formattedLyrics" : "rawLyrics"}`)}</span><Switch checked={formatted} onCheckedChange={onCheckedChange} aria-label={t("library.manager.formattedLyrics")} /></label>;
}

function LyricsPreview({ document, formatted, compact = false, t }: { document: LyricsDocument | null | undefined; formatted: boolean; compact?: boolean; t: TFunction }) {
  if (!document) return <div className={`${styles.lyricsText} ${compact ? styles.compactLyricsText : ""}`}>{t("library.manager.previewUnavailable")}</div>;
  if (!formatted) {
    const raw = document.raw.slice(0, compact ? 1200 : undefined);
    return <div className={`${styles.lyricsText} ${compact ? styles.compactLyricsText : ""}`}>{raw.trim() ? raw : t("library.manager.previewUnavailable")}</div>;
  }

  let remaining = compact ? 1200 : Number.POSITIVE_INFINITY;
  let hasVisibleText = false;
  const rows = document.tracks.original.lines.flatMap((original, index) => {
    if (!original.text.trim() || remaining <= 0) return [];
    const romanization = document.tracks.romanization && findAlignedAuxiliaryLine(document.tracks.romanization.lines, original);
    const translation = document.tracks.translation && findAlignedAuxiliaryLine(document.tracks.translation.lines, original);
    const tracks = [
      ["romanization", romanization?.text] as const,
      ["original", original.text] as const,
      ["translation", translation?.text] as const,
    ].filter(([, text]) => text?.trim());
    return <div className={styles.formattedLyricsRow} key={`${original.startMs}-${index}`}>{tracks.map(([kind, text]) => {
      const visibleText = text!.slice(0, remaining);
      remaining -= visibleText.length;
      if (visibleText.trim()) hasVisibleText = true;
      return visibleText ? <span className={styles[`${kind}LyricsLine`]} key={kind}>{visibleText}</span> : null;
    })}</div>;
  });
  return <div className={`${styles.lyricsText} ${styles.formattedLyrics} ${compact ? styles.compactLyricsText : ""}`}>{hasVisibleText ? rows : t("library.manager.previewUnavailable")}</div>;
}
