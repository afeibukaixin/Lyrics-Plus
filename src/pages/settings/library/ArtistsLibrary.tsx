import { useEffect, useRef, useState } from "react";
import { Plus, Save, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { lyricsApi } from "@/shared/api/lyrics";
import { messageOf } from "@/shared/api";
import type { LibraryArtistDetail, LibraryArtistSummary, LibraryPage } from "@/shared/types/lyrics";
import { LibraryDetailHeader, LibraryDetailSection, LibraryRelationItem, LibraryRelationList, LibraryState, LibraryToolbar, PageControls, TruncatedText, useLibraryNavigation } from "./shared";
import styles from "./library.module.scss";

export default function ArtistsLibrary({ detailId }: { detailId: number | null }) {
  const { t } = useTranslation();
  const navigation = useLibraryNavigation({
    section: "artists",
    sectionLabel: t("library.manager.tabs.artists"),
    detailsLabel: t("library.manager.details"),
    detailId: null,
  });
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [data, setData] = useState<LibraryPage<LibraryArtistSummary> | null>(null);
  const [error, setError] = useState("");
  const requestSequence = useRef(0);
  useEffect(() => {
    const sequence = ++requestSequence.current;
    const timer = window.setTimeout(() => lyricsApi.listLibraryArtists(query, page, pageSize).then((value) => {
      if (sequence !== requestSequence.current) return;
      setData(value); setError("");
    }).catch((reason) => {
      if (sequence === requestSequence.current) setError(messageOf(reason));
    }), 180);
    return () => window.clearTimeout(timer);
  }, [query, page, pageSize, detailId]);
  if (detailId) return <ArtistDetail artistId={detailId} />;
  return (
    <div className={styles.workspace}>
      <Card className={`${styles.panel} ${styles.listPanel}`}>
        <LibraryToolbar query={query} onQueryChange={(value) => { setQuery(value); setPage(1); }} />
        <CardContent className={styles.tableContent}>
        {error ? (
          <LibraryState state="error" message={error} />
        ) : data === null ? (
          <LibraryState state="loading" message={t("library.manager.loading")} />
        ) : (
          <>
            {data.items.length ? (
              <Table className={`${styles.adaptiveTable} ${styles.dataTable}`}>
                <colgroup>
                  <col className={styles.artistNameColumn} />
                  <col className={styles.artistAliasesColumn} />
                  <col className={styles.artistSongCountColumn} />
                  <col className={styles.artistRawCreditsColumn} />
                  <col className={styles.artistActionsColumn} />
                </colgroup>
                <TableHeader><TableRow><TableHead className={styles.growColumn}>{t("library.manager.artist")}</TableHead><TableHead className={styles.growColumn}>{t("library.manager.aliases")}</TableHead><TableHead className={`${styles.numericColumn} ${styles.compactColumn}`}>{t("library.manager.songCount")}</TableHead><TableHead className={styles.growColumn}>{t("library.manager.rawCredits")}</TableHead><TableHead className={`${styles.actionColumn} ${styles.compactColumn}`}>{t("library.manager.actions")}</TableHead></TableRow></TableHeader>
                <TableBody>{data.items.map((item) => <TableRow key={item.artistId}><TableCell className={`${styles.primaryCell} ${styles.growColumn}`}><TruncatedText variant="title">{item.canonicalName}</TruncatedText></TableCell><TableCell className={styles.growColumn}>{item.aliases.length ? <div className={styles.tableAliasBadges}>{item.aliases.map((value) => <Badge className={styles.tableAliasBadge} variant="secondary" key={value}><TruncatedText>{value}</TruncatedText></Badge>)}</div> : "—"}</TableCell><TableCell className={`${styles.numericColumn} ${styles.compactColumn}`}>{item.songCount}</TableCell><TableCell className={styles.growColumn}><TruncatedText>{item.rawNames.join(" / ") || "—"}</TruncatedText></TableCell><TableCell className={`${styles.actionColumn} ${styles.compactColumn}`}><Button size="sm" variant="outline" onClick={() => navigation.openDetail("artists", item.artistId, item.canonicalName)}>{t("library.manager.details")}</Button></TableCell></TableRow>)}</TableBody>
              </Table>
            ) : (
              <LibraryState state="empty" message={t("library.manager.emptyArtists")} />
            )}
          </>
        )}
        </CardContent>
        {data ? <PageControls page={data.page} pageSize={data.pageSize} total={data.total} onPageChange={setPage} onPageSizeChange={(value) => { setPageSize(value); setPage(1); }} /> : null}
      </Card>
    </div>
  );
}

function ArtistDetail({ artistId }: { artistId: number }) {
  const { t } = useTranslation();
  const [detail, setDetail] = useState<LibraryArtistDetail | null>(null);
  const [name, setName] = useState("");
  const [alias, setAlias] = useState("");
  const [error, setError] = useState("");
  const navigation = useLibraryNavigation({
    section: "artists",
    sectionLabel: t("library.manager.tabs.artists"),
    detailsLabel: t("library.manager.details"),
    detailId: artistId,
    detailLabel: detail?.summary.canonicalName,
  });
  const onBack = navigation.back;
  const apply = (value: LibraryArtistDetail) => { setDetail(value); setName(value.summary.canonicalName); setError(""); };
  useEffect(() => { lyricsApi.getLibraryArtist(artistId).then(apply).catch((reason) => setError(messageOf(reason))); }, [artistId]);
  const saveName = async () => { try { apply(await lyricsApi.updateLibraryArtistName(artistId, name)); toast.success(t("library.manager.artistNameSaved")); } catch (reason) { toast.error(messageOf(reason)); } };
  const addAlias = async () => { if (!alias.trim()) return; try { apply(await lyricsApi.setLibraryArtistAlias(artistId, alias, true)); setAlias(""); toast.success(t("library.manager.aliasAdded")); } catch (reason) { toast.error(messageOf(reason)); } };
  const removeAlias = async (value: string) => { try { apply(await lyricsApi.setLibraryArtistAlias(artistId, value, false)); toast.success(t("library.manager.aliasRemoved")); } catch (reason) { toast.error(messageOf(reason)); } };
  if (!detail) return <Card className={`${styles.panel} ${styles.detailPanel}`}><LibraryDetailHeader title={t("library.manager.details")} breadcrumbs={navigation.breadcrumbs} onBack={onBack} /><CardContent><LibraryState state={error ? "error" : "loading"} message={error || t("library.manager.loading")} /></CardContent></Card>;
  return <Card className={`${styles.panel} ${styles.detailPanel}`}><LibraryDetailHeader title={detail.summary.canonicalName} description={t("library.manager.artistSummary", { songs: detail.summary.songCount, aliases: detail.summary.aliases.length })} breadcrumbs={navigation.breadcrumbs} onBack={onBack} /><CardContent className={styles.detailBody}>{error ? <LibraryState state="error" message={error} /> : null}<LibraryDetailSection title={t("library.manager.canonicalName")}><Field orientation="horizontal" className={styles.formRow}><Input value={name} onChange={(event) => setName(event.target.value)} /><Button size="sm" disabled={!name.trim() || name.trim() === detail.summary.canonicalName} onClick={() => void saveName()}><Save data-icon="inline-start" />{t("common.actions.save")}</Button></Field></LibraryDetailSection><LibraryDetailSection title={t("library.manager.aliases")}><div className={styles.badges}>{detail.summary.aliases.map((value) => <Badge key={value} variant="secondary">{value}<button type="button" aria-label={t("library.manager.removeAlias", { alias: value })} onClick={() => void removeAlias(value)}><X data-icon="inline-end" /></button></Badge>)}</div><Field orientation="horizontal" className={styles.formRow}><Input value={alias} onChange={(event) => setAlias(event.target.value)} placeholder={t("library.manager.aliasPlaceholder")} onKeyDown={(event) => { if (event.key === "Enter") void addAlias(); }} /><Button size="sm" disabled={!alias.trim()} onClick={() => void addAlias()}><Plus data-icon="inline-start" />{t("library.manager.addAlias")}</Button></Field></LibraryDetailSection><LibraryDetailSection title={t("library.manager.rawCredits")}><div className={styles.badges}>{detail.summary.rawNames.map((value) => <Badge variant="outline" key={value}>{value}</Badge>)}</div></LibraryDetailSection><LibraryDetailSection title={t("library.manager.relatedSongs")}><LibraryRelationList>{detail.songs.map((song) => <LibraryRelationItem key={song.recordingId} title={song.title} description={song.album || "—"} actions={<Button size="sm" variant="outline" onClick={() => navigation.openDetail("songs", song.recordingId, song.title)}>{t("library.manager.openSong")}</Button>} />)}</LibraryRelationList></LibraryDetailSection></CardContent></Card>;
}
