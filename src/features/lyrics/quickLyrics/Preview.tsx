import type { TFunction } from "i18next";
import { FileText } from "lucide-react";
import { localizedSource } from "../../i18n/userText";
import type { LyricsSearchResult } from "../../../shared/types";
import { Badge } from "@/components/ui/badge";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import styles from "../QuickLyricsWindow.module.scss";

type QuickLyricsPreviewProps = {
  selected: LyricsSearchResult | null;
  t: TFunction;
};

export function QuickLyricsPreview({ selected, t }: QuickLyricsPreviewProps) {
  return (
    <Card className={cn(styles.previewPanel, "gap-0 py-0")} role="complementary">
      <CardHeader className={styles.panelTitle}>
        <CardTitle>{t("quickLyrics.preview")}</CardTitle>
        {selected && <CardAction><Badge variant="secondary">{localizedSource(selected.source, t)}</Badge></CardAction>}
      </CardHeader>
      <CardContent className="min-h-0 px-0">{selected ? <ScrollArea className="h-full min-h-0"><pre className="font-mono text-sm leading-relaxed">{selected.lyrics}</pre></ScrollArea> : <Empty className={styles.empty}><EmptyHeader><EmptyMedia variant="icon"><FileText /></EmptyMedia><EmptyTitle>{t("quickLyrics.selectCandidate")}</EmptyTitle><EmptyDescription>{t("quickLyrics.rawHint")}</EmptyDescription></EmptyHeader></Empty>}</CardContent>
    </Card>
  );
}
