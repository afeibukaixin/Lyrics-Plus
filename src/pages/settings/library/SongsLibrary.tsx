import { useEffect, useState } from "react";
import type { TFunction } from "i18next";
import { Bubbles, Check, Link2, Plus, Search } from "lucide-react";
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
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { CandidateEvidence } from "@/features/lyrics/quickLyrics/details/CandidateEvidence";
import { associationReasonLabel } from "@/features/lyrics/quickLyrics/details/helpers";
import { lyricsApi } from "@/shared/api/lyrics";
import { messageOf } from "@/shared/api";
import type { LibraryLyricSummary, LibraryPage, LibrarySongDetail, LibrarySongSummary, SongAssociationCandidate, SongSimilarityPair } from "@/shared/types/lyrics";
import { ConfirmAction, formatDuration, LibraryDetailHeader, LibraryDetailSection, LibraryRelationItem, LibraryRelationList, LibraryState, LibraryToolbar, PageControls, TruncatedText } from "./shared";
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

export default function SongsLibrary({ detailId }: { detailId: number | null }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [data, setData] = useState<LibraryPage<LibrarySongSummary> | null>(null);
  const [loadingError, setLoadingError] = useState("");
  const [similarOpen, setSimilarOpen] = useState(false);

  useEffect(() => {
    let live = true;
    const timer = window.setTimeout(() => {
      lyricsApi.listLibrarySongs(query, page, pageSize).then((result) => {
        if (!live) return;
        setData(result);
        setLoadingError("");
      }).catch((error) => live && setLoadingError(messageOf(error)));
    }, 180);
    return () => { live = false; window.clearTimeout(timer); };
  }, [query, page, pageSize, detailId]);

  if (detailId) return <SongDetail recordingId={detailId} onBack={() => navigate("/settings/library/songs")} />;
  if (similarOpen) return <SongSimilarityQueue onBack={() => setSimilarOpen(false)} />;
  return (
    <div className={styles.workspace}>
      <Card className={`${styles.panel} ${styles.listPanel}`}>
        <LibraryToolbar
          query={query}
          onQueryChange={(value) => { setQuery(value); setPage(1); }}
          actions={<Button size="sm" variant="outline" onClick={() => setSimilarOpen(true)}><Bubbles data-icon="inline-start" />{t("library.manager.similarSongs")}</Button>}
        />
        <CardContent className={styles.tableContent}>
        {loadingError ? <LibraryState state="error" message={loadingError} /> : data === null ? <LibraryState state="loading" message={t("library.manager.loading")} /> : data.items.length ? (
          <Table className={`${styles.fixedTable} ${styles.songsTable} ${styles.dataTable}`}>
            <TableHeader><TableRow><TableHead>{t("library.manager.song")}</TableHead><TableHead className={styles.numericColumn}>{t("library.manager.sources")}</TableHead><TableHead className={styles.numericColumn}>{t("library.manager.lyricsCount")}</TableHead><TableHead className={styles.numericColumn}>{t("library.manager.duration")}</TableHead><TableHead className={styles.actionColumn}>{t("library.manager.actions")}</TableHead></TableRow></TableHeader>
            <TableBody>{data.items.map((song) => (
              <TableRow key={song.recordingId}>
                <TableCell className={styles.primaryCell}><TruncatedText variant="title">{song.title}</TruncatedText><TruncatedText variant="meta">{song.artists.join(" / ") || "—"}</TruncatedText></TableCell>
                <TableCell className={styles.numericColumn}><TruncatedText>{String(song.sourceCount)}</TruncatedText></TableCell><TableCell className={styles.numericColumn}><TruncatedText>{String(song.lyricCount)}</TruncatedText></TableCell><TableCell className={styles.numericColumn}><TruncatedText>{formatDuration(song.durationMs)}</TruncatedText></TableCell>
                <TableCell className={styles.actionColumn}><Button size="sm" variant="outline" onClick={() => navigate(`/settings/library/songs/${song.recordingId}`)}>{t("library.manager.details")}</Button></TableCell>
              </TableRow>
            ))}</TableBody>
          </Table>
        ) : <LibraryState state="empty" message={t("library.manager.emptySongs")} />}
        </CardContent>
        {data ? <PageControls page={data.page} pageSize={data.pageSize} total={data.total} onPageChange={setPage} onPageSizeChange={(value) => { setPageSize(value); setPage(1); }} /> : null}
      </Card>
    </div>
  );
}

function SongSimilarityQueue({ onBack }: { onBack: () => void }) {
  const { t } = useTranslation();
  const [pairs, setPairs] = useState<SongSimilarityPair[] | null>(null);
  const [keepers, setKeepers] = useState<Record<string, number>>({});
  const [busyPairId, setBusyPairId] = useState<string | null>(null);
  const [detailId, setDetailId] = useState<number | null>(null);
  const [error, setError] = useState("");
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [total, setTotal] = useState(0);
  const [progress, setProgress] = useState("");

  const applyPairs = (value: SongSimilarityPair[]) => {
    setPairs(value);
    setKeepers(Object.fromEntries(value.map((pair) => [pair.pairId, pair.recommendedRecordingId])));
    setError("");
  };
  const refresh = async () => {
    try { const value = await lyricsApi.listLibrarySongSimilarity(page, pageSize); applyPairs(value.items); setTotal(value.total); }
    catch (reason) { setError(messageOf(reason)); setPairs([]); }
  };
  useEffect(() => { void refresh(); }, [page, pageSize]);
  useEffect(() => {
    if (pairs !== null) return;
    const update = () => { void lyricsApi.getLibraryIndexStatus().then((value) => { const index = value.indexes.find((item) => item.indexKind === "song_similarity"); setProgress(index && index.total > 0 ? ` ${index.processed}/${index.total}` : ""); }).catch(() => undefined); };
    update();
    const timer = window.setInterval(update, 500);
    return () => window.clearInterval(timer);
  }, [pairs]);

  if (detailId !== null) {
    return <SongDetail recordingId={detailId} onBack={() => { setDetailId(null); void refresh(); }} />;
  }

  const dismiss = async (pair: SongSimilarityPair) => {
    setBusyPairId(pair.pairId);
    try {
      await lyricsApi.dismissLibrarySongSimilarity(pair.songs[0].recordingId, pair.songs[1].recordingId);
      setPairs((current) => current?.filter((item) => item.pairId !== pair.pairId) ?? []);
      toast.success(t("library.manager.similarSongsKeptSeparate"));
    } catch (reason) { toast.error(messageOf(reason)); }
    finally { setBusyPairId(null); }
  };
  const merge = async (pair: SongSimilarityPair) => {
    const keeper = keepers[pair.pairId];
    const redundant = pair.songs.find((song) => song.recordingId !== keeper);
    if (!keeper || !redundant) return;
    setBusyPairId(pair.pairId);
    try {
      await lyricsApi.mergeLibrarySong(keeper, redundant.recordingId);
      setPairs((current) => current?.filter((item) => item.pairId !== pair.pairId) ?? []);
      setTotal((current) => Math.max(0, current - 1));
      toast.success(t("library.manager.similarSongsMerged"));
    } catch (reason) { toast.error(messageOf(reason)); }
    finally { setBusyPairId(null); }
  };

  return (
    <Card className={`${styles.panel} ${styles.queuePanel}`}>
      <LibraryDetailHeader title={t("library.manager.similarSongsTitle")} description={t("library.manager.similarSongsDescription")} onBack={onBack} />
      <CardContent className={styles.similarity}>
        {error ? <LibraryState state="error" message={error} /> : null}
        {pairs === null ? <LibraryState state="loading" message={`${t("library.manager.analyzingSimilarSongs")}${progress}`} /> : pairs.length === 0 && !error ? <LibraryState state="empty" message={t("library.manager.noSimilarSongs")} /> : pairs?.map((pair) => {
          const keeper = keepers[pair.pairId];
          const keeperSong = pair.songs.find((song) => song.recordingId === keeper);
          const redundantSong = pair.songs.find((song) => song.recordingId !== keeper);
          const busy = busyPairId !== null;
          return (
            <section className={styles.section} key={pair.pairId}>
              <div className={styles.sectionHeader}>
                <h3>{t("library.manager.similarSongsScore", { value: Math.round(pair.evidence.score * 100) })}</h3>
                <div className={styles.badges}>
                  <Badge variant={pair.evidence.kind === "originalRelation" ? "default" : pair.evidence.kind === "sharedIdentifier" ? "secondary" : "outline"}>{t(`library.manager.similarSongKinds.${pair.evidence.kind}`)}</Badge>
                  {!pair.evidence.canAssociate ? <Badge variant="destructive">{t("library.manager.mergeBlocked")}</Badge> : null}
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
                {pair.songs.map((song) => <Button size="sm" variant="outline" key={song.recordingId} onClick={() => setDetailId(song.recordingId)}>{t("library.manager.openSongNamed", { title: song.title })}</Button>)}
              </div>
              <div className={styles.songSimilarityEvidence}>
                {pair.evidence.matchReasons.length ? <p>{pair.evidence.matchReasons.map((reason) => associationReasonLabel(reason, t)).join(" · ")}</p> : null}
                <CandidateEvidence candidate={pair.evidence} t={t} />
              </div>
              <div className={styles.relationActions}>
                <Button size="sm" variant="outline" disabled={busy} onClick={() => void dismiss(pair)}>{t("library.manager.notSameSong")}</Button>
                <ConfirmAction
                  title={t("library.manager.mergeSimilarSongsTitle")}
                  description={t("library.manager.mergeSimilarSongsDescription", { keeper: keeperSong?.title ?? "—", redundant: redundantSong?.title ?? "—" })}
                  label={t("library.manager.mergeSongs")}
                  triggerVariant="default"
                  confirmVariant="default"
                  disabled={busy || !pair.evidence.canAssociate || !keeperSong || !redundantSong}
                  onConfirm={() => merge(pair)}
                />
              </div>
            </section>
          );
        })}
      </CardContent>
      {pairs ? <PageControls page={page} pageSize={pageSize} total={total} onPageChange={setPage} onPageSizeChange={(value) => { setPageSize(value); setPage(1); }} /> : null}
    </Card>
  );
}

function SongDetail({ recordingId, onBack }: { recordingId: number; onBack: () => void }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [detail, setDetail] = useState<LibrarySongDetail | null>(null);
  const [candidates, setCandidates] = useState<SongAssociationCandidate[]>([]);
  const [error, setError] = useState("");
  const [bindOpen, setBindOpen] = useState(false);

  const refresh = async () => {
    try {
      const value = await lyricsApi.getLibrarySong(recordingId);
      setDetail(value); setError("");
      setCandidates(value.recording.observations.length ? await lyricsApi.getLibrarySongCandidates(recordingId) : []);
    } catch (reason) { setError(messageOf(reason)); }
  };
  useEffect(() => { void refresh(); }, [recordingId]);

  if (!detail) return <Card className={`${styles.panel} ${styles.detailPanel}`}><LibraryDetailHeader title={t("library.manager.details")} onBack={onBack} /><CardContent><LibraryState state={error ? "error" : "loading"} message={error || t("library.manager.loading")} /></CardContent></Card>;
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

  return (
    <Card className={`${styles.panel} ${styles.detailPanel}`}>
      <LibraryDetailHeader title={recording.title} description={recording.artistCredits.map((credit) => credit.canonicalName).join(" / ")} onBack={onBack} />
      <CardContent className={styles.detailBody}>
        {error ? <LibraryState state="error" message={error} /> : null}
        <div className={styles.metadataGrid}>
          <div><span>{t("library.manager.album")}</span><strong>{recording.album || "—"}</strong></div>
          <div><span>{t("library.manager.duration")}</span><strong>{formatDuration(recording.durationMs)}</strong></div>
          <div><span>{t("library.manager.version")}</span><strong>{recording.versionTags.join(" / ") || "—"}</strong></div>
          <div><span>{t("library.manager.externalIds")}</span><strong>{recording.externalIdentifiers.map((item) => `${item.namespace}: ${item.value}`).join(" · ") || "—"}</strong></div>
        </div>
        <LibraryDetailSection title={t("library.manager.artistIdentity")}>
          <LibraryRelationList>{recording.artistCredits.map((credit, index) => <LibraryRelationItem key={`${credit.rawName}-${index}`} title={credit.canonicalName} description={<>{t("library.manager.rawCredit")}: {credit.rawName}{credit.confirmedAliases.length ? ` · ${t("library.manager.aliases")}: ${credit.confirmedAliases.join(" / ")}` : ""}</>} actions={credit.artistId ? <Button size="sm" variant="outline" onClick={() => navigate(`/settings/library/artists/${credit.artistId}`)}>{t("library.manager.manageArtist")}</Button> : null} />)}</LibraryRelationList>
        </LibraryDetailSection>
        <LibraryDetailSection title={t("library.manager.platformTracks")}>
          <LibraryRelationList>{recording.observations.map((item) => <LibraryRelationItem key={item.observationId} title={`${songSourceLabel(item, t)} · ${item.rawTitle}`} description={`${item.trackKey} · ${item.rawArtists.join(" / ")}`} actions={recording.observations.length > 1 ? <ConfirmAction title={t("library.manager.detachTitle")} description={t("library.manager.detachDescription")} label={t("library.manager.detach")} triggerVariant="destructive" onConfirm={() => detach(item.observationId)} /> : null} />)}</LibraryRelationList>
          {candidates.length ? <LibraryRelationList>{candidates.map((candidate) => <LibraryRelationItem key={candidate.recordingId} title={candidate.title} description={`${candidate.observations.map((item) => songSourceLabel(item, t)).join(" / ")} · ${Math.round(candidate.score * 100)}%`} actions={<ConfirmAction title={t("library.manager.associateTitle")} description={t("library.manager.associateDescription")} label={t("library.manager.associate")} triggerVariant="default" confirmVariant="default" disabled={!candidate.canAssociate} onConfirm={() => associate(candidate)} />} />)}</LibraryRelationList> : null}
        </LibraryDetailSection>
        <LibraryDetailSection title={t("library.manager.lyricResources")} actions={<Button size="sm" variant="default" onClick={() => setBindOpen(true)}><Plus data-icon="inline-start" />{t("library.manager.bindLyric")}</Button>}>
          {detail.bindings.length ? <LibraryRelationList>{detail.bindings.map((binding) => <LibraryRelationItem key={binding.bindingId} title={binding.asset?.sourceName ?? `#${binding.assetId}`} description={`${binding.isDefault ? t("library.manager.defaultLyric") : t("library.manager.candidateLyric")} · ${binding.offsetMs} ms`} actions={<>{!binding.isDefault ? <Button size="sm" variant="secondary" onClick={() => void setDefault(binding.assetId)}><Link2 data-icon="inline-start" />{t("library.manager.setDefault")}</Button> : null}<Button size="sm" variant="outline" onClick={() => navigate(`/settings/library/lyrics/${binding.assetId}`)}>{t("library.manager.preview")}</Button><ConfirmAction title={t("library.manager.unbindTitle")} description={t("library.manager.unbindDescription")} label={t("library.manager.unbind")} triggerVariant="destructive" onConfirm={() => unbind(binding.assetId)} /></>} />)}</LibraryRelationList> : <LibraryState state="empty" message={t("library.manager.noBoundLyrics")} />}
          {detail.platformOverrides.length ? <div className={styles.badges}>{detail.platformOverrides.map((item) => <Badge key={item.overrideId} variant="outline">{item.platform}: {item.asset?.sourceName ?? t("library.manager.none")}</Badge>)}</div> : null}
        </LibraryDetailSection>
      </CardContent>
      <BindLyricDialog open={bindOpen} onOpenChange={setBindOpen} recordingId={recordingId} onBound={(value) => setDetail(value)} />
    </Card>
  );
}

function BindLyricDialog({ open, onOpenChange, recordingId, onBound }: { open: boolean; onOpenChange: (open: boolean) => void; recordingId: number; onBound: (detail: LibrarySongDetail) => void }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<LibraryLyricSummary[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [replace, setReplace] = useState(false);
  useEffect(() => {
    if (!open) return;
    const timer = window.setTimeout(() => lyricsApi.listLibraryLyrics(query).then((value) => setItems(value.items)).catch((error) => toast.error(messageOf(error))), 180);
    return () => window.clearTimeout(timer);
  }, [open, query]);
  const bind = async () => {
    if (!selected) return;
    try { onBound(await lyricsApi.bindLibraryLyric(recordingId, selected, replace)); onOpenChange(false); toast.success(t("library.manager.bound")); }
    catch (reason) { toast.error(messageOf(reason)); }
  };
  return <Dialog open={open} onOpenChange={onOpenChange}><DialogContent><DialogHeader><DialogTitle>{t("library.manager.bindLyric")}</DialogTitle><DialogDescription>{t("library.manager.bindRule")}</DialogDescription></DialogHeader><Field><FieldLabel htmlFor="bind-lyric-search" className="sr-only">{t("library.manager.search")}</FieldLabel><InputGroup><InputGroupAddon><Search data-icon="inline-start" /></InputGroupAddon><InputGroupInput id="bind-lyric-search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("library.manager.searchPlaceholder")} /></InputGroup></Field><ItemGroup className={styles.dialogItemList}>{items.map((item) => <Item render={<button type="button" />} variant="outline" className={styles.selectableItem} data-selected={selected === item.assetId} aria-pressed={selected === item.assetId} key={item.assetId} onClick={() => setSelected(item.assetId)}><ItemContent><ItemTitle>{item.title}</ItemTitle><ItemDescription>{item.artist}</ItemDescription></ItemContent>{selected === item.assetId ? <ItemActions><Check className={styles.checkRowSelectionIcon} aria-hidden="true" /></ItemActions> : null}</Item>)}</ItemGroup><Item render={<label />} variant="outline" className={styles.checkboxItem}><Checkbox checked={replace} onCheckedChange={(value) => setReplace(value === true)} /><ItemContent><ItemTitle>{t("library.manager.replaceDefault")}</ItemTitle></ItemContent></Item><DialogFooter><Button variant="outline" onClick={() => onOpenChange(false)}>{t("common.actions.cancel")}</Button><Button disabled={!selected} onClick={() => void bind()}><Link2 data-icon="inline-start" />{t("library.manager.bind")}</Button></DialogFooter></DialogContent></Dialog>;
}
