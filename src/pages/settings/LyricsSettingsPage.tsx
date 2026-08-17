import type { LibraryScanStatus, MusixmatchTokenType, ProviderSettings, ProviderStatus } from "../../shared/types";
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
import { PageHeader, RangeRow, SettingsPage, SettingsSection } from "./components";
import { GripVertical, Settings2, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { IconButton } from "@/components/ui/icon-button";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Item, ItemActions, ItemContent, ItemDescription, ItemGroup, ItemMedia, ItemTitle } from "@/components/ui/item";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";

const defaultAmllBaseUrl = "https://amlldb.bikonoo.com";

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
  const [providerConfig, setProviderConfig] = useState<"musixmatch" | "amll_ttml" | null>(null);
  const [musixmatchTokenDraft, setMusixmatchTokenDraft] = useState("");
  const [musixmatchTokenType, setMusixmatchTokenType] = useState<MusixmatchTokenType>("desktopUserToken");
  const [amllBaseUrlDraft, setAmllBaseUrlDraft] = useState("");
  const [savingProviderConfig, setSavingProviderConfig] = useState(false);
  const {
    playback, lyrics, fileInput, providerRows, providerView, providerCredentials, testingProvider,
    resettingSection, confirmingReset, providerDrag, savingProviderOrder,
    saveProviderSettings, saveMusixmatchToken, clearMusixmatchToken,
    beginProviderDrag, continueProviderDrag, finishProviderDrag,
    setProviderDrag, providerDragTransform, toggleProvider, testProviders, handleFile,
    resetSection, setError,
  } = useSettingsContext();
  const normalizedTitleFilterDraft = titleFilterDraft.trim();
  const normalizedTokenDraft = musixmatchTokenDraft.trim();
  const normalizedAmllBaseUrlDraft = amllBaseUrlDraft.trim().replace(/\/+$/, "");
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

  const openProviderConfig = (providerId: string) => {
    if (providerId !== "musixmatch" && providerId !== "amll_ttml") return;
    setMusixmatchTokenDraft("");
    setMusixmatchTokenType(providerCredentials?.musixmatchTokenType ?? "desktopUserToken");
    setAmllBaseUrlDraft(providerView?.settings.amllBaseUrl ?? defaultAmllBaseUrl);
    setProviderConfig(providerId);
  };

  const saveMusixmatch = async (event: FormEvent) => {
    event.preventDefault();
    if (!normalizedTokenDraft || savingProviderConfig) return;
    setSavingProviderConfig(true);
    const saved = await saveMusixmatchToken(musixmatchTokenType, normalizedTokenDraft);
    setSavingProviderConfig(false);
    if (saved) {
      setMusixmatchTokenDraft("");
      setProviderConfig(null);
    }
  };

  const clearMusixmatch = async () => {
    if (savingProviderConfig) return;
    setSavingProviderConfig(true);
    const cleared = await clearMusixmatchToken();
    setSavingProviderConfig(false);
    if (cleared) setMusixmatchTokenDraft("");
  };

  const saveAmllBaseUrl = async (event: FormEvent) => {
    event.preventDefault();
    if (!providerView || !normalizedAmllBaseUrlDraft || savingProviderConfig) return;
    setSavingProviderConfig(true);
    const saved = await saveProviderSettings({
      ...providerView.settings,
      amllBaseUrl: normalizedAmllBaseUrlDraft,
    });
    setSavingProviderConfig(false);
    if (saved) setProviderConfig(null);
  };

  const scanActive = scanStatus?.phase === "discovering" || scanStatus?.phase === "indexing";
  const scanProgress = scanStatus?.phase === "indexing" && scanStatus.total
    ? Math.round(scanStatus.processed / scanStatus.total * 100)
    : 0;
  const sections = [
    { id: "lyrics-auto-match", label: t("settings.lyrics.autoMatch") },
    { id: "lyrics-current-track", label: t("settings.lyrics.currentTrack") },
    { id: "lyrics-directory", label: t("settings.lyrics.directory") },
    { id: "lyrics-title-filters", label: t("settings.lyrics.titleFilters") },
    { id: "lyrics-provider-priority", label: t("settings.lyrics.providerPriority") },
  ];

  return <SettingsPage sections={sections}>
    <PageHeader title={t("settings.lyrics.title")} description={t("settings.lyrics.description")} onReset={() => void resetSection("lyrics")} resetting={resettingSection === "lyrics"} confirming={confirmingReset === "lyrics"} />
    <SettingsSection id="lyrics-auto-match" title={t("settings.lyrics.autoMatch")}>
      <RangeRow label={t("settings.lyrics.threshold")} value={providerView?.settings.autoApplyThreshold ?? 60} min={0} max={100} suffix="%" onChange={(autoApplyThreshold) => { if (providerView) void saveProviderSettings({ ...providerView.settings, autoApplyThreshold }); }} />
      <p className={styles.cardHint}>{t("settings.lyrics.thresholdHint")}</p>
    </SettingsSection>
    <SettingsSection id="lyrics-current-track" title={t("settings.lyrics.currentTrack")}>
      <div className={styles.currentTrack}><div><strong>{playback.snapshot.title ?? t("settings.lyrics.noTrack")}</strong><small>{playback.snapshot.artist ?? "—"}</small></div><em>{lyrics.document ? localizedSource(lyrics.document.metadata.source, t) : t("settings.lyrics.notAssociated")}</em></div>
      <div className={styles.buttonRow}>
        <Button variant="secondary" size="sm" disabled={!lyrics.trackKey} onClick={() => void api.showQuickLyricsWindow().catch((error) => setError(messageOf(error)))}>{t("settings.lyrics.manualSearch")}</Button>
        <Button variant="secondary" size="sm" disabled={!lyrics.trackKey} onClick={() => fileInput.current?.click()}>{t("settings.lyrics.importLrc")}</Button>
        <input ref={fileInput} hidden type="file" accept=".lrc,text/plain" onChange={(event) => void handleFile(event.currentTarget.files?.[0])} />
        {lyrics.document && <Button variant="destructive" size="sm" onClick={() => void lyrics.remove()}>{t("settings.lyrics.unlink")}</Button>}
      </div>
      {lyrics.document && <div className={styles.offsetRow}><span>{t("settings.lyrics.offset", { value: `${lyrics.document.offsetMs > 0 ? "+" : ""}${lyrics.document.offsetMs}` })}</span><div><Button variant="outline" size="sm" onClick={() => void lyrics.changeOffset(-100)}>−100</Button><Button variant="outline" size="sm" onClick={() => void lyrics.changeOffset(100)}>+100</Button><Button variant="outline" size="sm" onClick={() => void lyrics.setOffset(0)}>{t("common.actions.reset")}</Button></div></div>}
    </SettingsSection>
    <SettingsSection id="lyrics-directory" title={t("settings.lyrics.directory")}>
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
    <SettingsSection id="lyrics-title-filters" title={t("settings.lyrics.titleFilters")} trailing={<Button variant="ghost" size="sm" disabled={savingTitleFilters} onClick={() => void (providerView && saveProviderSettings({ ...providerView.settings, titleFilterKeywords: defaultTitleFilterKeywords }))}>{t("settings.lyrics.restoreTitleFilters")}</Button>}>
      <p className={styles.cardHint}>{t("settings.lyrics.titleFiltersHint")}</p>
      <div className={styles.titleFilters}>{providerView?.settings.titleFilterKeywords.length ? providerView.settings.titleFilterKeywords.map((keyword, index) => <Badge variant="secondary" className={styles.titleFilter} key={`${keyword}-${index}`}><span>{keyword}</span><IconButton label={`${t("common.actions.remove")} ${keyword}`} variant="ghost" size="icon-sm" disabled={savingTitleFilters} onClick={() => void removeTitleFilter(index)}><X /></IconButton></Badge>) : <p>{t("settings.lyrics.titleFiltersEmpty")}</p>}</div>
      <form className={styles.titleFilterForm} onSubmit={(event) => void addTitleFilter(event)}><InputGroup><InputGroupInput aria-invalid={Boolean(titleFilterError)} placeholder={t("settings.lyrics.titleFilterPlaceholder")} value={titleFilterDraft} onChange={(event) => setTitleFilterDraft(event.target.value)} /><InputGroupAddon align="inline-end"><Button size="sm" disabled={!providerView || !normalizedTitleFilterDraft || Boolean(titleFilterError) || savingTitleFilters}>{t("settings.lyrics.addTitleFilter")}</Button></InputGroupAddon></InputGroup>{titleFilterError && <small role="alert">{titleFilterError}</small>}</form>
    </SettingsSection>
    <SettingsSection id="lyrics-provider-priority" title={t("settings.lyrics.providerPriority")} trailing={providerView && <div className={styles.shortcutControls}><Button variant="secondary" size="sm" disabled={!providerView.settings.providers.length || testingProvider !== null} onClick={() => void testProviders(providerView.settings.providers.map((provider) => provider.id))}>{testingProvider === "*" ? t("common.actions.testing") : t("common.actions.testAll")}</Button><Select disabled={savingProviderOrder} items={[{ value: "strict", label: t("settings.lyrics.strict") }, { value: "smart", label: t("settings.lyrics.smart") }]} value={providerView.settings.mode} onValueChange={(mode) => void saveProviderSettings({ ...providerView.settings, mode: mode as ProviderSettings["mode"] })}><SelectTrigger className="w-32" aria-label={t("settings.lyrics.providerPriority")}><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="strict">{t("settings.lyrics.strict")}</SelectItem><SelectItem value="smart">{t("settings.lyrics.smart")}</SelectItem></SelectGroup></SelectContent></Select></div>}>
      <p className={styles.cardHint}>{providerView?.settings.mode === "smart" ? t("settings.lyrics.smartHint") : t("settings.lyrics.strictHint")}</p>
      <ItemGroup className={styles.providers} data-dragging={Boolean(providerDrag)}>{providerView?.settings.providers.map((provider, index) => {
        const status = providerView.statuses.find((item) => item.providerId === provider.id);
        return <Item variant="muted" className={styles.provider} data-dragging={providerDrag?.providerId === provider.id} key={provider.id} ref={(element) => { if (element) providerRows.current.set(provider.id, element); else providerRows.current.delete(provider.id); }} style={{ transform: providerDragTransform(index) }}>
          <ItemMedia><Button type="button" variant="ghost" size="icon-sm" className={styles.dragHandle} aria-label={`${status?.name ?? provider.id} #${index + 1}`} disabled={savingProviderOrder} onPointerDown={(event) => beginProviderDrag(provider.id, index, event)} onPointerMove={continueProviderDrag} onPointerUp={finishProviderDrag} onPointerCancel={() => setProviderDrag(null)} onLostPointerCapture={() => setProviderDrag(null)}><GripVertical /></Button></ItemMedia>
          <Badge variant="outline">#{index + 1}</Badge>
          <ItemContent><ItemTitle>{status?.name ?? provider.id}</ItemTitle><ItemDescription className={styles.providerStatus} data-health={status?.health ?? "unknown"} title={status?.message ?? undefined}>{healthLabel(status, t)}{status?.message ? ` · ${status.message}` : ""}</ItemDescription></ItemContent>
          <ItemActions>
            {(provider.id === "musixmatch" || provider.id === "amll_ttml") && <IconButton label={t("settings.lyrics.providerConfig.configure", { source: status?.name ?? provider.id })} tooltip={t("settings.lyrics.providerConfig.configure", { source: status?.name ?? provider.id })} variant="ghost" size="icon-sm" onClick={() => openProviderConfig(provider.id)}><Settings2 /></IconButton>}
            <Switch aria-label={status?.name ?? provider.id} checked={provider.enabled} onCheckedChange={() => toggleProvider(provider.id)} />
            <Button variant="secondary" size="sm" disabled={testingProvider !== null} onClick={() => void testProviders([provider.id])}>{testingProvider === provider.id || testingProvider === "*" ? t("common.actions.testing") : t("common.actions.test")}</Button>
          </ItemActions>
        </Item>;
      })}</ItemGroup>
    </SettingsSection>
    <Dialog open={providerConfig !== null} onOpenChange={(open) => { if (!open && !savingProviderConfig) setProviderConfig(null); }}>
      <DialogContent className="max-h-[calc(100vh-2rem)] overflow-y-auto">
        {providerConfig === "musixmatch" ? <form onSubmit={(event) => void saveMusixmatch(event)}>
          <DialogHeader><DialogTitle>{t("settings.lyrics.providerConfig.musixmatchTitle")}</DialogTitle><DialogDescription>{t("settings.lyrics.providerConfig.musixmatchDescription")}</DialogDescription></DialogHeader>
          <FieldGroup className={styles.providerConfigFields}>
            <Field><FieldLabel htmlFor="musixmatch-token-type">{t("settings.lyrics.providerConfig.tokenType")}</FieldLabel><Select items={[{ value: "desktopUserToken", label: t("settings.lyrics.providerConfig.desktopToken") }, { value: "developerApiKey", label: t("settings.lyrics.providerConfig.developerApiKey") }]} value={musixmatchTokenType} onValueChange={(value) => setMusixmatchTokenType(value as MusixmatchTokenType)}><SelectTrigger id="musixmatch-token-type"><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="desktopUserToken">{t("settings.lyrics.providerConfig.desktopToken")}</SelectItem><SelectItem value="developerApiKey">{t("settings.lyrics.providerConfig.developerApiKey")}</SelectItem></SelectGroup></SelectContent></Select><FieldDescription>{t(`settings.lyrics.providerConfig.${musixmatchTokenType === "desktopUserToken" ? "desktopTokenHint" : "developerApiKeyHint"}`)}</FieldDescription></Field>
            <Field><FieldLabel htmlFor="musixmatch-token">{t("settings.lyrics.providerConfig.token")}</FieldLabel><Input id="musixmatch-token" type="password" autoComplete="off" value={musixmatchTokenDraft} onChange={(event) => setMusixmatchTokenDraft(event.target.value)} placeholder={providerCredentials?.musixmatchConfigured ? t("settings.lyrics.providerConfig.tokenConfigured") : t("settings.lyrics.providerConfig.tokenPlaceholder")} /><FieldDescription>{providerCredentials?.musixmatchConfigured ? t("settings.lyrics.providerConfig.configuredStatus") : t("settings.lyrics.providerConfig.notConfiguredStatus")} · {providerCredentials?.musixmatchConfigured ? t("settings.lyrics.providerConfig.configuredHint") : t("settings.lyrics.providerConfig.tokenHint")}</FieldDescription></Field>
          </FieldGroup>
          <DialogFooter>
            {providerCredentials?.musixmatchConfigured && <Button type="button" variant="destructive" disabled={savingProviderConfig} onClick={() => void clearMusixmatch()}>{t("settings.lyrics.providerConfig.clearToken")}</Button>}
            <Button type="button" variant="secondary" disabled={testingProvider !== null} onClick={() => void testProviders(["musixmatch"])}>{testingProvider === "musixmatch" ? t("common.actions.testing") : t("common.actions.test")}</Button>
            <Button disabled={!normalizedTokenDraft || savingProviderConfig}>{t("common.actions.save")}</Button>
          </DialogFooter>
        </form> : providerConfig === "amll_ttml" ? <form onSubmit={(event) => void saveAmllBaseUrl(event)}>
          <DialogHeader><DialogTitle>{t("settings.lyrics.providerConfig.amllTitle")}</DialogTitle><DialogDescription>{t("settings.lyrics.providerConfig.amllDescription")}</DialogDescription></DialogHeader>
          <FieldGroup className={styles.providerConfigFields}>
            <Field><FieldLabel htmlFor="amll-base-url">{t("settings.lyrics.providerConfig.baseUrl")}</FieldLabel><Input id="amll-base-url" inputMode="url" value={amllBaseUrlDraft} onChange={(event) => setAmllBaseUrlDraft(event.target.value)} /><FieldDescription>{t("settings.lyrics.providerConfig.baseUrlHint")}</FieldDescription></Field>
          </FieldGroup>
          <DialogFooter>
            <Button type="button" variant="ghost" disabled={savingProviderConfig} onClick={() => setAmllBaseUrlDraft(defaultAmllBaseUrl)}>{t("common.actions.resetDefault")}</Button>
            <Button type="button" variant="secondary" disabled={testingProvider !== null} onClick={() => void testProviders(["amll_ttml"])}>{testingProvider === "amll_ttml" ? t("common.actions.testing") : t("common.actions.test")}</Button>
            <Button disabled={!normalizedAmllBaseUrlDraft || savingProviderConfig}>{t("common.actions.save")}</Button>
          </DialogFooter>
        </form> : null}
      </DialogContent>
    </Dialog>
  </SettingsPage>;
}
