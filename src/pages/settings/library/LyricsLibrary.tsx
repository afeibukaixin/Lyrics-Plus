import { useEffect, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { Bubbles, Check, Link2, Search, Trash2 } from "lucide-react";
import { useNavigate } from "react-router";
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
import type { LibraryLyricDetail, LibraryLyricPage, LibraryLyricSource, LibraryLyricStatus, LibrarySongSummary, LibraryLyricSummary, LyricSimilarityGroup, UnboundCleanupPreview } from "@/shared/types/lyrics";
import { ConfirmAction, formatBytes, LibraryDetailHeader, LibraryDetailSection, LibraryRelationItem, LibraryRelationList, LibraryState, LibraryToolbar, PageControls, TruncatedText } from "./shared";
import styles from "./library.module.scss";

const lyricStatuses: Array<LibraryLyricStatus | "all"> = ["all", "inUse", "candidate", "unbound"];
const lyricSourceKinds = ["all", "managed", "cache", "local", "legacy"] as const;

export default function LyricsLibrary({ detailId, onStatusCountsChange }: {
  detailId: number | null;
  onStatusCountsChange: (counts: Record<LibraryLyricStatus, number>) => void;
}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<LibraryLyricStatus | "all">("all");
  const [sourceKind, setSourceKind] = useState<(typeof lyricSourceKinds)[number]>("all");
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [data, setData] = useState<LibraryLyricPage | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [cleanupSelection, setCleanupSelection] = useState<Set<number>>(new Set());
  const [cleanupExcluded, setCleanupExcluded] = useState<Set<number>>(new Set());
  const [cleanupSelectionMode, setCleanupSelectionMode] = useState<"selected" | "allExcept">("allExcept");
  const [cleanupPageSize, setCleanupPageSize] = useState(20);
  const [similarOpen, setSimilarOpen] = useState(false);
  const [cleanup, setCleanup] = useState<UnboundCleanupPreview | null>(null);
  const [cleanupOpen, setCleanupOpen] = useState(false);
  const [error, setError] = useState("");
  const requestSequence = useRef(0);

  const refresh = () => {
    const sequence = ++requestSequence.current;
    return lyricsApi.listLibraryLyrics(query, status === "all" ? null : status, sourceKind === "all" ? null : sourceKind, page, pageSize).then((value) => {
      if (sequence !== requestSequence.current) return;
      setData(value); setError("");
    }).catch((reason) => {
      if (sequence === requestSequence.current) setError(messageOf(reason));
    });
  };
  useEffect(() => {
    const timer = window.setTimeout(() => void refresh(), 180);
    return () => window.clearTimeout(timer);
  }, [query, status, sourceKind, page, pageSize]);
  useEffect(() => {
    if (detailId !== null) return;
    if (data) onStatusCountsChange(data.statusCounts);
  }, [data, detailId, onStatusCountsChange]);

  if (detailId) return <LyricDetail assetId={detailId} onBack={() => navigate("/settings/library/lyrics")} />;
  if (similarOpen) return <SimilarityQueue onBack={() => { setSimilarOpen(false); void refresh(); }} />;
  const visibleIds = data?.items.map((item) => item.assetId) ?? [];
  const allVisibleSelected = visibleIds.length > 0 && visibleIds.every((id) => selected.has(id));

  const openCleanup = async () => {
    try {
      const value = await lyricsApi.previewUnboundLyricsCleanup(1, cleanupPageSize);
      setCleanup(value);
      const selectedCleanupIds = [...selected];
      setCleanupSelectionMode(selectedCleanupIds.length ? "selected" : "allExcept");
      setCleanupSelection(new Set(selectedCleanupIds));
      setCleanupExcluded(new Set());
      setCleanupOpen(true);
    }
    catch (reason) { toast.error(messageOf(reason)); }
  };
  const runCleanup = async () => {
    if (!cleanup) return;
    try {
      const result = await lyricsApi.cleanupUnboundLyrics(
        cleanupSelectionMode,
        [...cleanupSelection],
        [...cleanupExcluded],
        cleanup.revision,
      );
      const failures = result.items.filter((item) => item.error);
      toast.success(t("library.manager.cleanupDone", { count: result.deletedFiles, size: formatBytes(result.releasedBytes) }));
      failures.forEach((item) => toast.error(`#${item.assetId}: ${t("errors.command")}`));
      setCleanupOpen(false); setSelected(new Set()); await refresh();
    } catch (reason) { toast.error(messageOf(reason)); }
  };
  const loadCleanupPage = async (nextPage: number, nextPageSize: number) => {
    try {
      const value = await lyricsApi.previewUnboundLyricsCleanup(nextPage, nextPageSize);
      setCleanup(value); setCleanupPageSize(value.pageSize);
    } catch (reason) { toast.error(messageOf(reason)); }
  };

  return (
    <div className={styles.workspace}>
      <Card className={`${styles.panel} ${styles.listPanel}`}>
        <LibraryToolbar
          query={query}
          onQueryChange={(value) => { setQuery(value); setPage(1); }}
          filters={<><Select value={status} onValueChange={(value) => { setStatus(value as typeof status); setPage(1); }}><SelectTrigger><SelectValue>{t(`library.manager.status.${status}`)}</SelectValue></SelectTrigger><SelectContent><SelectGroup>{lyricStatuses.map((value) => <SelectItem value={value} key={value}>{t(`library.manager.status.${value}`)}</SelectItem>)}</SelectGroup></SelectContent></Select><Select value={sourceKind} onValueChange={(value) => { setSourceKind(value as typeof sourceKind); setPage(1); }}><SelectTrigger><SelectValue>{t(`library.manager.source.${sourceKind}`)}</SelectValue></SelectTrigger><SelectContent><SelectGroup>{lyricSourceKinds.map((value) => <SelectItem value={value} key={value}>{t(`library.manager.source.${value}`)}</SelectItem>)}</SelectGroup></SelectContent></Select></>}
          actions={<><Button size="sm" variant="outline" onClick={() => setSimilarOpen(true)}><Bubbles data-icon="inline-start" />{t("library.manager.similarLyrics")}</Button><Button size="sm" variant="destructive" onClick={() => void openCleanup()}><Trash2 data-icon="inline-start" />{t("library.manager.cleanup")}</Button></>}
        />
        <CardContent className={styles.tableContent}>
        {error ? <LibraryState state="error" message={error} /> : data === null ? <LibraryState state="loading" message={t("library.manager.loading")} /> : data.items.length ? <Table className={`${styles.fixedTable} ${styles.lyricsTable} ${styles.dataTable}`}><TableHeader><TableRow><TableHead className={styles.selectionColumn}><Checkbox checked={allVisibleSelected} onCheckedChange={(checked) => setSelected(checked ? new Set(visibleIds) : new Set())} aria-label={t("library.manager.selectAll")} /></TableHead><TableHead>{t("library.manager.lyric")}</TableHead><TableHead>{t("library.manager.statusLabel")}</TableHead><TableHead>{t("library.manager.capabilities")}</TableHead><TableHead className={styles.numericColumn}>{t("library.manager.size")}</TableHead><TableHead className={styles.actionColumn}>{t("library.manager.actions")}</TableHead></TableRow></TableHeader><TableBody>{data.items.map((item) => <TableRow key={item.assetId} data-state={selected.has(item.assetId) ? "selected" : undefined}><TableCell className={styles.selectionColumn}><Checkbox checked={selected.has(item.assetId)} onCheckedChange={(checked) => setSelected((current) => { const next = new Set(current); if (checked) next.add(item.assetId); else next.delete(item.assetId); return next; })} aria-label={item.title} /></TableCell><TableCell className={styles.primaryCell}><TruncatedText variant="title">{item.title}</TruncatedText><TruncatedText variant="meta">{`${item.artist} · ${item.sourceName}`}</TruncatedText></TableCell><TableCell><StatusBadge item={item} t={t} /></TableCell><TableCell><div className={styles.badges}>{item.hasWordTiming ? <Badge variant="outline">{t("common.feature.wordTiming")}</Badge> : null}{item.hasTranslation ? <Badge variant="outline">{t("common.feature.translation")}</Badge> : null}{item.hasRomanization ? <Badge variant="outline">{t("common.feature.romanization")}</Badge> : null}</div></TableCell><TableCell className={styles.numericColumn}><TruncatedText>{formatBytes(item.fileSize)}</TruncatedText></TableCell><TableCell className={styles.actionColumn}><Button size="sm" variant="outline" onClick={() => navigate(`/settings/library/lyrics/${item.assetId}`)}>{t("library.manager.details")}</Button></TableCell></TableRow>)}</TableBody></Table> : <LibraryState state="empty" message={t("library.manager.emptyLyrics")} />}
        </CardContent>
        {data ? <PageControls page={data.page} pageSize={data.pageSize} total={data.total} onPageChange={setPage} onPageSizeChange={(value) => { setPageSize(value); setPage(1); }} /> : null}
      </Card>
      <Dialog open={cleanupOpen} onOpenChange={setCleanupOpen}><DialogContent><DialogHeader><DialogTitle>{t("library.manager.cleanupTitle")}</DialogTitle><DialogDescription>{cleanup ? t("library.manager.cleanupDescription", { count: cleanup.fileCount, size: formatBytes(cleanup.totalSize) }) : ""}</DialogDescription></DialogHeader><ItemGroup className={styles.dialogItemList}>{cleanup?.items.map((item) => { const checked = cleanupSelectionMode === "allExcept" ? !cleanupExcluded.has(item.assetId) : cleanupSelection.has(item.assetId); return <Item render={<label />} variant="outline" className={styles.checkboxItem} data-selected={checked} key={item.assetId}><Checkbox checked={checked} onCheckedChange={(value) => { const enabled = value === true; if (cleanupSelectionMode === "allExcept") setCleanupExcluded((current) => { const next = new Set(current); if (enabled) next.delete(item.assetId); else next.add(item.assetId); return next; }); else setCleanupSelection((current) => { const next = new Set(current); if (enabled) next.add(item.assetId); else next.delete(item.assetId); return next; }); }} /><ItemContent><ItemTitle>{item.title}</ItemTitle><ItemDescription>{formatBytes(item.fileSize)}</ItemDescription></ItemContent></Item>; })}</ItemGroup>{cleanup ? <PageControls compact page={cleanup.page} pageSize={cleanup.pageSize} total={cleanup.total} onPageChange={(value) => void loadCleanupPage(value, cleanupPageSize)} onPageSizeChange={(value) => void loadCleanupPage(1, value)} /> : null}<DialogFooter><Button variant="outline" onClick={() => setCleanupOpen(false)}>{t("common.actions.cancel")}</Button><Button variant="destructive" disabled={cleanupSelectionMode === "selected" ? !cleanupSelection.size : Boolean(cleanup && cleanupExcluded.size >= cleanup.total)} onClick={() => void runCleanup()}>{t("library.manager.confirmCleanup")}</Button></DialogFooter></DialogContent></Dialog>
    </div>
  );
}

function StatusBadge({ item, t }: { item: LibraryLyricSummary; t: TFunction }) {
  const variant = item.status === "inUse" ? "default" : item.status === "candidate" ? "secondary" : "outline";
  return <span className={styles.badges}><Badge variant={variant}>{t(`library.manager.status.${item.status}`)}</Badge>{!item.available ? <Badge variant="destructive">{t("library.manager.unavailable")}</Badge> : null}</span>;
}

function LyricDetail({ assetId, onBack }: { assetId: number; onBack: () => void }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [detail, setDetail] = useState<LibraryLyricDetail | null>(null);
  const [bindOpen, setBindOpen] = useState(false);
  const [formattedPreview, setFormattedPreview] = useState(true);
  const [error, setError] = useState("");
  const refresh = () => lyricsApi.getLibraryLyric(assetId).then((value) => { setDetail(value); setError(""); }).catch((reason) => setError(messageOf(reason)));
  useEffect(() => { void refresh(); }, [assetId]);
  const unbind = async (recordingId: number) => { try { await lyricsApi.unbindLibraryLyric(recordingId, assetId); await refresh(); toast.success(t("library.manager.unbound")); } catch (reason) { toast.error(messageOf(reason)); } };
  const removeSource = async (sourceId: number) => { try { setDetail(await lyricsApi.deleteLibraryLyricSource(assetId, sourceId)); toast.success(t("library.manager.sourceDeleted")); } catch (reason) { toast.error(messageOf(reason)); } };
  if (!detail) return <Card className={`${styles.panel} ${styles.detailPanel}`}><LibraryDetailHeader title={t("library.manager.details")} onBack={onBack} /><CardContent><LibraryState state={error ? "error" : "loading"} message={error || t("library.manager.loading")} /></CardContent></Card>;
  return <Card className={`${styles.panel} ${styles.detailPanel}`}><LibraryDetailHeader title={detail.summary.title} description={`${detail.summary.artist} · ${detail.summary.sourceName}`} status={<StatusBadge item={detail.summary} t={t} />} actions={<Button size="sm" variant="default" onClick={() => setBindOpen(true)}><Link2 data-icon="inline-start" />{t("library.manager.bindSong")}</Button>} onBack={onBack} /><CardContent className={styles.detailBody}>{error ? <LibraryState state="error" message={error} /> : null}<div className={`${styles.metadataGrid} ${styles.lyricMetadataGrid}`}><div><span>{t("library.manager.format")}</span><strong>{detail.summary.originalFormat} · {detail.summary.language}</strong></div><div><span>{t("library.manager.capabilities")}</span><strong>{[detail.summary.hasWordTiming && t("common.feature.wordTiming"), detail.summary.hasTranslation && t("common.feature.translation"), detail.summary.hasRomanization && t("common.feature.romanization")].filter(Boolean).join(" / ") || t("common.feature.plainText")}</strong></div><div><span>{t("library.manager.fingerprint")}</span><strong>{detail.summary.contentFingerprint.slice(0, 16)}…</strong></div></div><LibraryDetailSection title={t("library.manager.boundSongs")}>{detail.recordings.length ? <LibraryRelationList>{detail.recordings.map((song) => <LibraryRelationItem key={song.recordingId} title={song.title} description={`${song.artists.join(" / ")} · ${song.isDefault ? t("library.manager.defaultLyric") : t("library.manager.candidateLyric")}`} actions={<><Button size="sm" variant="outline" onClick={() => navigate(`/settings/library/songs/${song.recordingId}`)}>{t("library.manager.openSong")}</Button><ConfirmAction title={t("library.manager.unbindTitle")} description={t("library.manager.unbindDescription")} label={t("library.manager.unbind")} triggerVariant="destructive" onConfirm={() => unbind(song.recordingId)} /></>} />)}</LibraryRelationList> : <LibraryState state="empty" message={t("library.manager.noBoundSongs")} />}</LibraryDetailSection><LibraryDetailSection title={t("library.manager.physicalSources")}><LibraryRelationList>{detail.sources.map((source, index) => <LibraryRelationItem key={source.sourceId ?? index} title={source.sourceName} description={`${source.rootName || source.sourceKind} · ${source.relativePath || t("library.manager.memorySource")} · ${formatBytes(source.fileSize)}`} actions={<LyricSourceAction detail={detail} source={source} onRemove={removeSource} t={t} />} />)}</LibraryRelationList></LibraryDetailSection><LibraryDetailSection title={t("library.manager.lyricPreview")} actions={<PreviewModeSwitch formatted={formattedPreview} onCheckedChange={setFormattedPreview} t={t} />}><LyricsPreview document={detail.document} formatted={formattedPreview} t={t} /></LibraryDetailSection></CardContent><BindSongDialog open={bindOpen} onOpenChange={setBindOpen} assetId={assetId} onBound={() => void refresh()} /></Card>;
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
  const [selected, setSelected] = useState<number | null>(null);
  const [replace, setReplace] = useState(false);
  const requestSequence = useRef(0);
  useEffect(() => { if (!open) return; const sequence = ++requestSequence.current; const timer = window.setTimeout(() => lyricsApi.listLibrarySongs(query).then((value) => { if (requestSequence.current === sequence) setItems(value.items); }).catch((reason) => { if (requestSequence.current === sequence) toast.error(messageOf(reason)); }), 180); return () => window.clearTimeout(timer); }, [open, query]);
  const bind = async () => { if (!selected) return; try { await lyricsApi.bindLibraryLyric(selected, assetId, replace); toast.success(t("library.manager.bound")); onOpenChange(false); onBound(); } catch (reason) { toast.error(messageOf(reason)); } };
  return <Dialog open={open} onOpenChange={onOpenChange}><DialogContent><DialogHeader><DialogTitle>{t("library.manager.bindSong")}</DialogTitle><DialogDescription>{t("library.manager.bindRule")}</DialogDescription></DialogHeader><Field><FieldLabel htmlFor="bind-song-search" className="sr-only">{t("library.manager.search")}</FieldLabel><InputGroup><InputGroupAddon><Search data-icon="inline-start" /></InputGroupAddon><InputGroupInput id="bind-song-search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("library.manager.searchPlaceholder")} /></InputGroup></Field><ItemGroup className={styles.dialogItemList}>{items.map((item) => <Item render={<button type="button" />} variant="outline" className={styles.selectableItem} key={item.recordingId} data-selected={selected === item.recordingId} aria-pressed={selected === item.recordingId} onClick={() => setSelected(item.recordingId)}><ItemContent><ItemTitle>{item.title}</ItemTitle><ItemDescription>{item.artists.join(" / ")}</ItemDescription></ItemContent>{selected === item.recordingId ? <ItemActions><Check className={styles.checkRowSelectionIcon} aria-hidden="true" /></ItemActions> : null}</Item>)}</ItemGroup><Item render={<label />} variant="outline" className={styles.checkboxItem}><Checkbox checked={replace} onCheckedChange={(value) => setReplace(value === true)} /><ItemContent><ItemTitle>{t("library.manager.replaceDefault")}</ItemTitle></ItemContent></Item><DialogFooter><Button variant="outline" onClick={() => onOpenChange(false)}>{t("common.actions.cancel")}</Button><Button disabled={!selected} onClick={() => void bind()}><Link2 data-icon="inline-start" />{t("library.manager.bind")}</Button></DialogFooter></DialogContent></Dialog>;
}

function SimilarityQueue({ onBack }: { onBack: () => void }) {
  const { t } = useTranslation();
  const [groups, setGroups] = useState<LyricSimilarityGroup[] | null>(null);
  const [previews, setPreviews] = useState<Record<number, LyricsDocument | null>>({});
  const [keepers, setKeepers] = useState<Record<string, number>>({});
  const [formattedPreview, setFormattedPreview] = useState(true);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [total, setTotal] = useState(0);
  const [progress, setProgress] = useState("");
  useEffect(() => { lyricsApi.listLibraryLyricSimilarity(page, pageSize).then((value) => { setGroups(value.items); setTotal(value.total); setPreviews(value.previews); setKeepers(Object.fromEntries(value.items.map((group) => [group.groupId, group.recommendedAssetId]))); }).catch((reason) => toast.error(messageOf(reason))); }, [page, pageSize]);
  useEffect(() => { if (groups !== null) return; const update = () => { void lyricsApi.getLibraryIndexStatus().then((value) => { const index = value.indexes.find((item) => item.indexKind === "lyric_similarity"); setProgress(index && index.total > 0 ? ` ${index.processed}/${index.total}` : ""); }).catch(() => undefined); }; update(); const timer = window.setInterval(update, 500); return () => window.clearInterval(timer); }, [groups]);
  const dismiss = async (group: LyricSimilarityGroup) => { try { await lyricsApi.dismissLibraryLyricSimilarity(group.items.map((item) => item.assetId)); setGroups((current) => current?.filter((item) => item.groupId !== group.groupId) ?? []); } catch (reason) { toast.error(messageOf(reason)); } };
  const merge = async (group: LyricSimilarityGroup) => { const keeper = keepers[group.groupId]; if (!keeper) return; try { await lyricsApi.mergeLibraryLyrics(keeper, group.items.map((item) => item.assetId).filter((id) => id !== keeper)); setGroups((current) => current?.filter((item) => item.groupId !== group.groupId) ?? []); toast.success(t("library.manager.merged")); } catch (reason) { toast.error(messageOf(reason)); } };
  return <Card className={`${styles.panel} ${styles.queuePanel}`}><LibraryDetailHeader title={t("library.manager.similarTitle")} description={t("library.manager.similarDescription")} actions={<PreviewModeSwitch formatted={formattedPreview} onCheckedChange={setFormattedPreview} t={t} />} onBack={onBack} /><CardContent className={styles.similarity}>{groups === null ? <LibraryState state="loading" message={`${t("library.manager.analyzing")}${progress}`} /> : groups.length === 0 ? <LibraryState state="empty" message={t("library.manager.noSimilar")} /> : groups.map((group) => <section className={styles.section} key={group.groupId}><div className={styles.sectionHeader}><h3>{Math.round(group.score * 100)}% · {group.highSimilarity ? t("library.manager.highSimilarity") : t("library.manager.similar")}</h3><div className={styles.badges}>{group.durationWarning ? <Badge variant="destructive">{t("library.manager.durationConflict")}</Badge> : null}{group.versionWarning ? <Badge variant="destructive">{t("library.manager.versionConflict")}</Badge> : null}</div></div><ToggleGroup className={styles.similarityGrid} variant="outline" value={keepers[group.groupId] ? [String(keepers[group.groupId])] : []} onValueChange={(values) => { const value = Number(values[0]); if (Number.isSafeInteger(value)) setKeepers((current) => ({ ...current, [group.groupId]: value })); }} aria-label={t("library.manager.selectKeeper")}>{group.items.map((item) => <ToggleGroupItem className={styles.similarityItem} value={String(item.assetId)} data-recommended={item.assetId === group.recommendedAssetId} key={item.assetId}>{keepers[group.groupId] === item.assetId ? <Check className={styles.similaritySelectionIcon} aria-hidden="true" /> : null}<strong>{item.title}</strong><span>{item.artist} · {item.sourceName}</span>{item.assetId === group.recommendedAssetId ? <Badge>{t("library.manager.recommended")}</Badge> : null}<LyricsPreview compact document={previews[item.assetId]} formatted={formattedPreview} t={t} /></ToggleGroupItem>)}</ToggleGroup><div className={styles.relationActions}><Button size="sm" variant="outline" onClick={() => void dismiss(group)}>{t("library.manager.notDuplicate")}</Button><ConfirmAction title={t("library.manager.mergeTitle")} description={t("library.manager.mergeDescription")} label={t("library.manager.merge")} triggerVariant="default" confirmVariant="default" onConfirm={() => merge(group)} /></div></section>)}</CardContent>{groups ? <PageControls page={page} pageSize={pageSize} total={total} onPageChange={setPage} onPageSizeChange={(value) => { setPageSize(value); setPage(1); }} /> : null}</Card>;
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
