import { useState } from "react";
import { Navigate, useNavigate, useParams } from "react-router";
import { useTranslation } from "react-i18next";
import { Bubbles, Mic2, Music2, ScrollText } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { LibraryLyricStatus } from "@/shared/types/lyrics";
import { PageHeader } from "../shared/components";
import ArtistsLibrary from "./ArtistsLibrary";
import LyricsLibrary from "./LyricsLibrary";
import SongsLibrary from "./SongsLibrary";
import { librarySections, type LibrarySection } from "./shared";
import styles from "./library.module.scss";

type SimilaritySection = Exclude<LibrarySection, "artists">;

export default function LibraryPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { section, id } = useParams();
  const [lyricStatusCounts, setLyricStatusCounts] = useState<Record<LibraryLyricStatus, number>>({
    inUse: 0,
    candidate: 0,
    unbound: 0,
  });
  const [similaritySection, setSimilaritySection] = useState<SimilaritySection | null>(null);

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
      <div className={styles.sectionBar}>
        <Tabs
          value={activeSection}
          onValueChange={(value) => {
            setSimilaritySection(null);
            navigate(`/settings/library/${String(value)}`);
          }}
        >
          <TabsList aria-label={t("library.manager.tabsLabel")}>
            <TabsTrigger value="songs"><Music2 data-icon="inline-start" />{t("library.manager.tabs.songs")}</TabsTrigger>
            <TabsTrigger value="lyrics"><ScrollText data-icon="inline-start" />{t("library.manager.tabs.lyrics")}</TabsTrigger>
            <TabsTrigger value="artists"><Mic2 data-icon="inline-start" />{t("library.manager.tabs.artists")}</TabsTrigger>
          </TabsList>
        </Tabs>
        {validId === null && (activeSection === "songs" || activeSection === "lyrics") ? (
          <div className={styles.sectionActions}>
            {activeSection === "lyrics" ? (
              <div className={styles.sectionSummary}>
                <Badge>{t("library.manager.status.inUse")} {lyricStatusCounts.inUse}</Badge>
                <Badge variant="secondary">{t("library.manager.status.candidate")} {lyricStatusCounts.candidate}</Badge>
                <Badge variant="outline">{t("library.manager.status.unbound")} {lyricStatusCounts.unbound}</Badge>
              </div>
            ) : null}
            <Button
              size="sm"
              variant="outline"
              onClick={() => setSimilaritySection(activeSection)}
            >
              <Bubbles data-icon="inline-start" />
              {t(`library.manager.similar${activeSection === "songs" ? "Songs" : "Lyrics"}`)}
            </Button>
          </div>
        ) : null}
      </div>
      <div className={styles.libraryContent}>
        {activeSection === "songs" ? (
          <SongsLibrary
            detailId={validId}
            similarityOpen={similaritySection === "songs"}
            onSimilarityClose={() => setSimilaritySection(null)}
          />
        ) : null}
        {activeSection === "lyrics" ? (
          <LyricsLibrary
            detailId={validId}
            onStatusCountsChange={setLyricStatusCounts}
            similarityOpen={similaritySection === "lyrics"}
            onSimilarityClose={() => setSimilaritySection(null)}
          />
        ) : null}
        {activeSection === "artists" ? <ArtistsLibrary detailId={validId} /> : null}
      </div>
    </main>
  );
}
