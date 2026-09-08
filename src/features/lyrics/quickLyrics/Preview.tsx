import { useEffect, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { FileText } from "lucide-react";
import { api } from "../../../shared/api";
import type { LyricsDocument, LyricsSearchResult } from "../../../shared/types";
import { findAlignedAuxiliaryLine } from "../useLyrics";
import { localizedSource } from "../../i18n/userText";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import { resultKey } from "./utils";
import styles from "../QuickLyricsWindow.module.scss";

type QuickLyricsPreviewProps = {
  selected: LyricsSearchResult | null;
  activeDocument: LyricsDocument | null;
  trackKey: string | null;
  t: TFunction;
};

type PreviewState = {
  selectionKey: string | null;
  document: LyricsDocument | null;
  loading: boolean;
  parseFailed: boolean;
};

type CachedPreview = {
  source: string;
  raw: string;
  document: LyricsDocument;
};

const EMPTY_PREVIEW: PreviewState = {
  selectionKey: null,
  document: null,
  loading: false,
  parseFailed: false,
};

function previewSelectionKey(trackKey: string | null, result: LyricsSearchResult) {
  return `${trackKey ?? "none"}:${resultKey(result)}:${result.source}:${result.lyrics}`;
}

function sameLyrics(left: LyricsDocument, result: LyricsSearchResult) {
  return left.metadata.source === result.source && left.raw.trim() === result.lyrics.trim();
}

function useLyricsPreview(
  selected: LyricsSearchResult | null,
  activeDocument: LyricsDocument | null,
  trackKey: string | null,
) {
  const cache = useRef(new Map<string, CachedPreview>());
  const requestVersion = useRef(0);
  const [state, setState] = useState<PreviewState>(EMPTY_PREVIEW);

  useEffect(() => {
    cache.current.clear();
  }, [trackKey]);

  useEffect(() => {
    const version = ++requestVersion.current;
    const selectionKey = selected ? previewSelectionKey(trackKey, selected) : null;
    if (!selected) {
      setState(EMPTY_PREVIEW);
      return () => undefined;
    }

    if (!selected.lyrics.trim()) {
      setState({ selectionKey, document: null, loading: false, parseFailed: true });
      return () => undefined;
    }

    if (activeDocument && sameLyrics(activeDocument, selected)) {
      setState({ selectionKey, document: activeDocument, loading: false, parseFailed: false });
      return () => undefined;
    }

    const key = `${trackKey ?? "none"}:${resultKey(selected)}`;
    const cached = cache.current.get(key);
    if (cached && cached.source === selected.source && cached.raw === selected.lyrics) {
      setState({ selectionKey, document: cached.document, loading: false, parseFailed: false });
      return () => undefined;
    }

    setState({ selectionKey, document: null, loading: true, parseFailed: false });
    void api.parseLyricsPreview(selected.source, selected.lyrics)
      .then((document) => {
        if (requestVersion.current !== version) return;
        cache.current.set(key, { source: selected.source, raw: selected.lyrics, document });
        setState({ selectionKey, document, loading: false, parseFailed: false });
      })
      .catch(() => {
        if (requestVersion.current !== version) return;
        setState({ selectionKey, document: null, loading: false, parseFailed: true });
      });

    return () => {
      if (requestVersion.current === version) requestVersion.current += 1;
    };
  }, [activeDocument, selected, trackKey]);

  return state;
}

function PreviewLyricsLines({ document }: { document: LyricsDocument }) {
  return (
    <div className={styles.previewLines} role="list">
      {document.tracks.original.lines.map((line, index) => {
        if (!line.text.trim()) {
          return <div className={styles.previewSectionBreak} aria-hidden="true" key={`${line.startMs}:${index}`} />;
        }

        const romanization = document.tracks.romanization
          ? findAlignedAuxiliaryLine(document.tracks.romanization.lines, line)
          : null;
        const translation = document.tracks.translation
          ? findAlignedAuxiliaryLine(document.tracks.translation.lines, line)
          : null;

        return (
          <div className={styles.previewLine} role="listitem" key={`${line.startMs}:${index}`}>
            {romanization && <p className={styles.previewAuxiliary} data-kind="romanization">{romanization.text}</p>}
            <p className={styles.previewOriginal}>{line.text}</p>
            {translation && <p className={styles.previewAuxiliary} data-kind="translation">{translation.text}</p>}
          </div>
        );
      })}
    </div>
  );
}

export function QuickLyricsPreview({ selected, activeDocument, trackKey, t }: QuickLyricsPreviewProps) {
  const preview = useLyricsPreview(selected, activeDocument, trackKey);
  const selectionKey = selected ? previewSelectionKey(trackKey, selected) : null;
  const previewMatchesSelection = preview.selectionKey === selectionKey;

  return (
    <Card className={cn(styles.previewPanel, "gap-0 py-0")} role="complementary">
      <CardHeader className={styles.panelTitle}>
        <CardTitle>{t("quickLyrics.preview")}</CardTitle>
        {selected && <CardAction><Badge variant="secondary">{localizedSource(selected.source, t)}</Badge></CardAction>}
      </CardHeader>
      <CardContent className="min-h-0 px-0">
        {selected ? (
          <ScrollArea className="h-full min-h-0">
            {!previewMatchesSelection || preview.loading
              ? <div className={styles.previewLoading} aria-busy="true" aria-label={t("quickLyrics.previewLoading")} aria-live="polite" role="status">
                {["one", "two", "three", "four", "five", "six"].map((key) => <Skeleton className={styles.previewSkeletonLine} key={key} />)}
              </div>
              : preview.document
                ? <PreviewLyricsLines document={preview.document} />
                : <div className={styles.previewFallback}>
                  {preview.parseFailed && <Alert className={styles.previewNotice} variant="warning"><AlertDescription>{t("quickLyrics.previewParseFailed")}</AlertDescription></Alert>}
                  <pre>{selected.lyrics}</pre>
                </div>}
          </ScrollArea>
        ) : (
          <Empty className={styles.empty}>
            <EmptyHeader>
              <EmptyMedia variant="icon"><FileText /></EmptyMedia>
              <EmptyTitle>{t("quickLyrics.selectCandidate")}</EmptyTitle>
              <EmptyDescription>{t("quickLyrics.rawHint")}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        )}
      </CardContent>
    </Card>
  );
}
