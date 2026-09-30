import { useState } from "react";
import { Navigate, useLocation, useNavigate, useParams } from "react-router";
import { useTranslation } from "react-i18next";
import { Bubbles, Mic2, Music2, ScrollText } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { LibraryLyricStatus } from "@/shared/types/lyrics";
import { PageHeader } from "../shared/components";
import ArtistsLibrary from "./ArtistsLibrary";
import LyricsLibrary from "./LyricsLibrary";
import SongsLibrary from "./SongsLibrary";
import { librarySections, type LibrarySection } from "./shared";
import { useLibraryViewState } from "./viewState";
import styles from "./library.module.scss";

export default function LibraryPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const location = useLocation();
  const { section, id } = useParams();
  const [lyricStatusCounts, setLyricStatusCounts] = useState<Record<LibraryLyricStatus, number>>({
    inUse: 0,
    candidate: 0,
    unbound: 0,
  });
  const songsView = useLibraryViewState("songs");
  const lyricsView = useLibraryViewState("lyrics");

  if (!librarySections.includes(section as LibrarySection)) {
    return <Navigate to="/settings/library/songs" replace />;
  }

  const activeSection = section as LibrarySection;
  const objectId = id ? Number(id) : null;
  const validId = objectId !== null && Number.isSafeInteger(objectId) && objectId > 0 ? objectId : null;

  return (
    <main className={styles.page} data-detail={validId !== null ? "true" : "false"}>
      <PageHeader
        title={t("library.manager.title")}
        description={t("library.manager.description")}
      />
      <Tabs
        className={styles.libraryTabs}
        value={activeSection}
        onValueChange={(value) => {
          navigate(`/settings/library/${String(value)}${location.search}`);
        }}
      >
        <div className={styles.sectionBar}>
          <TabsList className={styles.sectionTabs} aria-label={t("library.manager.tabsLabel")}>
            <TabsTrigger value="songs"><Music2 data-icon="inline-start" />{t("library.manager.tabs.songs")}</TabsTrigger>
            <TabsTrigger value="lyrics"><ScrollText data-icon="inline-start" />{t("library.manager.tabs.lyrics")}</TabsTrigger>
            <TabsTrigger value="artists"><Mic2 data-icon="inline-start" />{t("library.manager.tabs.artists")}</TabsTrigger>
          </TabsList>
          {validId === null && (activeSection === "songs" || activeSection === "lyrics") ? (
            <div className={styles.sectionActions}>
              {activeSection === "lyrics" ? (
                <div className={styles.sectionSummary}>
                  {(["inUse", "candidate", "unbound"] as const).map((status) => (
                    <Button
                      key={status}
                      type="button"
                      size="sm"
                      variant={lyricsView.status === status ? "secondary" : "outline"}
                      aria-pressed={lyricsView.status === status}
                      onClick={() => lyricsView.update({ status: lyricsView.status === status ? "all" : status, page: 1, view: "list" })}
                    >{t(`library.manager.status.${status}`)} {lyricStatusCounts[status]}</Button>
                  ))}
                </div>
              ) : null}
              <Button
                size="sm"
                variant="outline"
                onClick={() => (activeSection === "songs" ? songsView : lyricsView).update({ view: "similar" })}
              >
                <Bubbles data-icon="inline-start" />
                {t(`library.manager.similar${activeSection === "songs" ? "Songs" : "Lyrics"}`)}
              </Button>
            </div>
          ) : null}
        </div>
        <TabsContent value="songs" className={styles.libraryContent}>
          <SongsLibrary
            detailId={validId}
            similarityOpen={songsView.view === "similar"}
            onSimilarityClose={() => songsView.update({ view: "list" })}
          />
        </TabsContent>
        <TabsContent value="lyrics" className={styles.libraryContent}>
          <LyricsLibrary
            detailId={validId}
            onStatusCountsChange={setLyricStatusCounts}
            similarityOpen={lyricsView.view === "similar"}
            onSimilarityClose={() => lyricsView.update({ view: "list" })}
          />
        </TabsContent>
        <TabsContent value="artists" className={styles.libraryContent}>
          <ArtistsLibrary detailId={validId} />
        </TabsContent>
      </Tabs>
    </main>
  );
}
