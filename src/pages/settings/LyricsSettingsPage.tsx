import type { LibraryScanStatus, ProviderSettings, ProviderStatus } from "../../shared/types";
import type { TFunction } from "i18next";
import { useEffect, useState, type FormEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { localizedSource } from "../../features/i18n/userText";
import { api, isTauriRuntime, messageOf } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { RangeRow, SettingsSection, PageHeader } from "./components";
import { ChevronDown, GripVertical, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { IconButton } from "@/components/ui/icon-button";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemMedia, ItemTitle } from "@/components/ui/item";

const defaultTitleFilterKeywords = [
  "feat", "ft", "featuring", "主题曲", "片头曲", "片尾曲",
  "插曲", "电影", "电视剧", "动画", "游戏", "ost",
];

function healthLabel(status: ProviderStatus | undefined, t: TFunction) {
  return t(`settings.lyrics.health.${status?.health ?? "unknown"}`);
}

export default function LyricsSettingsPage() {
  const { t } = useTranslation();
  const [titleFilterDraft, setTitleFilterDraft] = useState("");
  const [savingTitleFilters, setSavingTitleFilters] = useState(false);
  const [libraryDir, setLibraryDir] = useState<string | null>(null);
  const [scanStatus, setScanStatus] = useState<LibraryScanStatus | null>(null);
  const [changingDirectory, setChangingDirectory] = useState(false);
  const {
    playback, lyrics, fileInput, providerRows, providerView, testingProvider,
    resettingSection, confirmingReset, providerDrag, savingProviderOrder,
    saveProviderSettings, beginProviderDrag, continueProviderDrag, finishProviderDrag,
    setProviderDrag, providerDragTransform, toggleProvider, testProviders, handleFile,
    resetSection, setError,
  } = useSettingsContext();
  const normalizedTitleFilterDraft = titleFilterDraft.trim();
  const titleFilterError = !normalizedTitleFilterDraft
    ? null
    : normalizedTitleFilterDraft.length > 64
      ? t("settings.lyrics.titleFilterTooLong")
      : (providerView?.settings.titleFilterKeywords ?? []).some((keyword) => keyword.toLocaleLowerCase() === normalizedTitleFilterDraft.toLocaleLowerCase())
        ? t("settings.lyrics.titleFilterDuplicate")
        : (providerView?.settings.titleFilterKeywords.length ?? 0) >= 32
          ? t("settings.lyrics.titleFilterLimit")
          : null;

  useEffect(() => {
    if (!isTauriRuntime()) return;
    const acceptStatus = (status: LibraryScanStatus) => {
      setScanStatus((current) => !current || status.scanId >= current.scanId ? status : current);
      setLibraryDir(status.libraryDir);
    };
    const cleanup = createTauriListenerCleanup(
      listen<LibraryScanStatus>("lyrics://library-scan-progress", ({ payload }) => acceptStatus(payload)),
    );
    void api.getLibraryScanStatus().then(acceptStatus).catch((error) => setError(messageOf(error)));
    return cleanup;
  }, [setError]);

  const addTitleFilter = async (event: FormEvent) => {
    event.preventDefault();
    if (!providerView || !normalizedTitleFilterDraft || titleFilterError || savingTitleFilters) return;
    setSavingTitleFilters(true);
    const saved = await saveProviderSettings({ ...providerView.settings, titleFilterKeywords: [...providerView.settings.titleFilterKeywords, normalizedTitleFilterDraft] });
    if (saved) setTitleFilterDraft("");
    setSavingTitleFilters(false);
  };

  const removeTitleFilter = async (index: number) => {
    if (!providerView || savingTitleFilters) return;
    setSavingTitleFilters(true);
    await saveProviderSettings({ ...providerView.settings, titleFilterKeywords: providerView.settings.titleFilterKeywords.filter((_, itemIndex) => itemIndex !== index) });
    setSavingTitleFilters(false);
  };

  const changeDirectory = async () => {
    const selected = await open({ directory: true, multiple: false, defaultPath: libraryDir ?? undefined, title: t("library.chooseFolder") });
    if (!selected) return;
    setChangingDirectory(true);
    setError(null);
    try {
      const status = await api.setLyricsDirectory(selected);
      setLibraryDir(status.libraryDir);
      setScanStatus(status);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setChangingDirectory(false);
    }
  };

  const rescanLibrary = async () => {
    setError(null);
    try {
      setScanStatus(await api.rescanLyricsLibrary());
    } catch (error) {
      setError(messageOf(error));
    }
  };

  const scanActive = scanStatus?.phase === "discovering" || scanStatus?.phase === "indexing";
  const scanProgress = scanStatus?.phase === "indexing" && scanStatus.total
    ? Math.round(scanStatus.processed / scanStatus.total * 100)
    : 0;

  return <>
    <PageHeader title={t("settings.lyrics.title")} description={t("settings.lyrics.description")} onReset={() => void resetSection("lyrics")} resetting={resettingSection === "lyrics"} confirming={confirmingReset === "lyrics"} />
    <SettingsSection title={t("settings.lyrics.autoMatch")}>
      <RangeRow label={t("settings.lyrics.threshold")} value={providerView?.settings.autoApplyThreshold ?? 60} min={0} max={100} suffix="%" onChange={(autoApplyThreshold) => { if (providerView) void saveProviderSettings({ ...providerView.settings, autoApplyThreshold }); }} />
      <p className={styles.cardHint}>{t("settings.lyrics.thresholdHint")}</p>
    </SettingsSection>
    <SettingsSection title={t("settings.lyrics.currentTrack")}>
      <div className={styles.currentTrack}><div><strong>{playback.snapshot.title ?? t("settings.lyrics.noTrack")}</strong><small>{playback.snapshot.artist ?? "—"}</small></div><em>{lyrics.document ? localizedSource(lyrics.document.metadata.source, t) : t("settings.lyrics.notAssociated")}</em></div>
      <div className={styles.buttonRow}>
        <Button variant="secondary" size="sm" disabled={!lyrics.trackKey} onClick={() => void api.showQuickLyricsWindow().catch((error) => setError(messageOf(error)))}>{t("settings.lyrics.manualSearch")}</Button>
        <Button variant="secondary" size="sm" disabled={!lyrics.trackKey} onClick={() => fileInput.current?.click()}>{t("settings.lyrics.importLrc")}</Button>
        <input ref={fileInput} hidden type="file" accept=".lrc,text/plain" onChange={(event) => void handleFile(event.currentTarget.files?.[0])} />
        {lyrics.document && <Button variant="destructive" size="sm" onClick={() => void lyrics.remove()}>{t("settings.lyrics.unlink")}</Button>}
      </div>
      {lyrics.document && <div className={styles.offsetRow}><span>{t("settings.lyrics.offset", { value: `${lyrics.document.offsetMs > 0 ? "+" : ""}${lyrics.document.offsetMs}` })}</span><div><Button variant="outline" size="sm" onClick={() => void lyrics.changeOffset(-100)}>−100</Button><Button variant="outline" size="sm" onClick={() => void lyrics.changeOffset(100)}>+100</Button><Button variant="outline" size="sm" onClick={() => void lyrics.setOffset(0)}>{t("common.actions.reset")}</Button></div></div>}
    </SettingsSection>
    <SettingsSection title={t("settings.lyrics.directory")}>
      <p className={styles.directoryPath}>{libraryDir ?? t("library.loadingDirectory")}</p>
      {scanStatus && <div className={styles.scanStatus} data-phase={scanStatus.phase}>
        {scanStatus.phase === "discovering" && <><Progress className="animate-pulse" value={100} /><strong>{t("settings.lyrics.scanDiscovering")}</strong><span>{t("settings.lyrics.scanDiscovered", { discovered: scanStatus.discovered, skipped: scanStatus.skipped })}</span></>}
        {scanStatus.phase === "indexing" && <><Progress value={scanProgress} /><strong>{t("settings.lyrics.scanIndexing", { processed: scanStatus.processed, total: scanStatus.total ?? 0 })}</strong><span>{t("settings.lyrics.scanLiveStats", scanStatus)}</span></>}
        {scanStatus.phase === "completed" && <><strong>{t("settings.lyrics.scanCompleted")}</strong><span>{t("settings.lyrics.scanSummary", scanStatus)}</span>{scanStatus.firstFailure && <small>{scanStatus.firstFailure}</small>}</>}
        {scanStatus.phase === "failed" && <><strong>{t("settings.lyrics.scanFailed")}</strong><span role="alert">{scanStatus.error}</span></>}
      </div>}
      <div className={styles.buttonRow}>
        <Button variant="secondary" size="sm" disabled={!libraryDir} onClick={() => void api.openLyricsDirectory().catch((error) => setError(messageOf(error)))}>{t("library.openFolder")}</Button>
        <Button variant="secondary" size="sm" disabled={changingDirectory} onClick={() => void changeDirectory()}>{changingDirectory ? t("library.changing") : t("library.changeFolder")}</Button>
        <Button variant="secondary" size="sm" disabled={!libraryDir} onClick={() => void rescanLibrary()}>{scanActive ? t("settings.lyrics.restartScan") : t("settings.lyrics.rescan")}</Button>
      </div>
    </SettingsSection>
    <Collapsible className={styles.advancedSection}>
      <CollapsibleTrigger render={<Button variant="outline" className={styles.advancedTrigger} />}>
        {t("settings.shell.advanced")}<ChevronDown data-icon="inline-end" />
      </CollapsibleTrigger>
      <CollapsibleContent className={styles.advancedContent}>
      <SettingsSection title={t("settings.lyrics.titleFilters")} trailing={<Button variant="ghost" size="sm" disabled={savingTitleFilters} onClick={() => void (providerView && saveProviderSettings({ ...providerView.settings, titleFilterKeywords: defaultTitleFilterKeywords }))}>{t("settings.lyrics.restoreTitleFilters")}</Button>}>
        <p className={styles.cardHint}>{t("settings.lyrics.titleFiltersHint")}</p>
        <div className={styles.titleFilters}>{providerView?.settings.titleFilterKeywords.length ? providerView.settings.titleFilterKeywords.map((keyword, index) => <Badge variant="secondary" className={styles.titleFilter} key={`${keyword}-${index}`}><span>{keyword}</span><IconButton label={`${t("common.actions.remove")} ${keyword}`} variant="ghost" size="icon-sm" disabled={savingTitleFilters} onClick={() => void removeTitleFilter(index)}><X /></IconButton></Badge>) : <p>{t("settings.lyrics.titleFiltersEmpty")}</p>}</div>
        <form className={styles.titleFilterForm} onSubmit={(event) => void addTitleFilter(event)}><InputGroup><InputGroupInput aria-invalid={Boolean(titleFilterError)} placeholder={t("settings.lyrics.titleFilterPlaceholder")} value={titleFilterDraft} onChange={(event) => setTitleFilterDraft(event.target.value)} /><InputGroupAddon align="inline-end"><Button size="sm" disabled={!providerView || !normalizedTitleFilterDraft || Boolean(titleFilterError) || savingTitleFilters}>{t("settings.lyrics.addTitleFilter")}</Button></InputGroupAddon></InputGroup>{titleFilterError && <small role="alert">{titleFilterError}</small>}</form>
      </SettingsSection>
      <SettingsSection title={t("settings.lyrics.providerPriority")} trailing={providerView && <div className={styles.shortcutControls}><Button variant="secondary" size="sm" disabled={!providerView.settings.providers.length || testingProvider !== null} onClick={() => void testProviders(providerView.settings.providers.map((provider) => provider.id))}>{testingProvider === "*" ? t("common.actions.testing") : t("common.actions.testAll")}</Button><Select disabled={savingProviderOrder} items={[{ value: "strict", label: t("settings.lyrics.strict") }, { value: "smart", label: t("settings.lyrics.smart") }]} value={providerView.settings.mode} onValueChange={(mode) => void saveProviderSettings({ ...providerView.settings, mode: mode as ProviderSettings["mode"] })}><SelectTrigger className="w-32" aria-label={t("settings.lyrics.providerPriority")}><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="strict">{t("settings.lyrics.strict")}</SelectItem><SelectItem value="smart">{t("settings.lyrics.smart")}</SelectItem></SelectGroup></SelectContent></Select></div>}>
        <p className={styles.cardHint}>{providerView?.settings.mode === "smart" ? t("settings.lyrics.smartHint") : t("settings.lyrics.strictHint")}</p>
        <ItemGroup className={styles.providers} data-dragging={Boolean(providerDrag)}>{providerView?.settings.providers.map((provider, index) => {
          const status = providerView.statuses.find((item) => item.providerId === provider.id);
          return <Item variant="muted" className={styles.provider} data-dragging={providerDrag?.providerId === provider.id} key={provider.id} ref={(element) => { if (element) providerRows.current.set(provider.id, element); else providerRows.current.delete(provider.id); }} style={{ transform: providerDragTransform(index) }}>
            <ItemMedia><Button type="button" variant="ghost" size="icon-sm" className={styles.dragHandle} aria-label={`${status?.name ?? provider.id} #${index + 1}`} disabled={savingProviderOrder} onPointerDown={(event) => beginProviderDrag(provider.id, index, event)} onPointerMove={continueProviderDrag} onPointerUp={finishProviderDrag} onPointerCancel={() => setProviderDrag(null)} onLostPointerCapture={() => setProviderDrag(null)}><GripVertical /></Button></ItemMedia>
            <Badge variant="outline">#{index + 1}</Badge>
            <ItemContent><ItemTitle>{status?.name ?? provider.id}</ItemTitle><ItemDescription className={styles.providerStatus} data-health={status?.health ?? "unknown"}>{healthLabel(status, t)}</ItemDescription></ItemContent>
            <ItemActions><Switch aria-label={status?.name ?? provider.id} checked={provider.enabled} onCheckedChange={() => toggleProvider(provider.id)} /><Button variant="secondary" size="sm" disabled={testingProvider !== null} onClick={() => void testProviders([provider.id])}>{testingProvider === provider.id || testingProvider === "*" ? t("common.actions.testing") : t("common.actions.test")}</Button></ItemActions>
          </Item>;
        })}</ItemGroup>
      </SettingsSection>
      </CollapsibleContent>
    </Collapsible>
  </>;
}
