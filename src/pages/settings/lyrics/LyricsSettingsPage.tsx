import type { ChineseConversion, LibraryRootView, LibraryScanStatus, MatchWeights, MusixmatchTokenType, ProviderErrorKind, ProviderSettings, ProviderStatus, ProviderStatusDetail } from "../../../shared/types";
import type { TFunction } from "i18next";
import { useEffect, useState, type FormEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { localizedSource } from "../../../features/i18n/userText";
import { api, isTauriRuntime, messageOf } from "../../../shared/api";
import { createTauriListenerCleanup } from "../../../shared/tauriEvent";
import { useAppConfig } from "../../../features/config/AppConfigProvider";
import { useSettingsContext } from "../shared/SettingsContext";
import styles from "../settings.module.scss";
import { PageHeader, RangeRow, SelectRow, SettingsPage, SettingsSection, ToggleRow } from "../shared/components";
import { FolderPlus, GripVertical, RefreshCw, Settings2, Trash2, X } from "lucide-react";
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

const defaultAmllBaseUrl = "https://api.amll.dev";
const defaultMatchWeights: MatchWeights = { title: 64, artist: 16, album: 5, duration: 10, version: 5 };
const matchWeightKeys = ["title", "artist", "album", "duration", "version"] as const;
type MatchWeightKey = (typeof matchWeightKeys)[number];
const defaultCapabilityPreferenceTolerance = 10;
const defaultMaxCandidatesPerProvider = 20;

const defaultTitleFilterKeywords = [
  "feat", "ft", "featuring", "主题曲", "片头曲", "片尾曲",
  "插曲", "电影", "电视剧", "动画", "游戏", "ost",
];

function healthLabel(status: ProviderStatus | undefined, t: TFunction) {
  return t(`settings.lyrics.health.${status?.health ?? "unknown"}`);
}

function providerErrorLabel(errorKind: ProviderErrorKind, t: TFunction) {
  return t(`settings.lyrics.statusDetail.errors.${errorKind}`);
}

function providerDetailLabel(status: ProviderStatus | undefined, t: TFunction) {
  const detail: ProviderStatusDetail = status?.detail ?? { kind: "not_tested" };
  switch (detail.kind) {
    case "not_tested":
      return t("settings.lyrics.statusDetail.notTested");
    case "not_participated":
      return t("settings.lyrics.notParticipated");
    case "success":
      return detail.resultCount > 0
        ? t("settings.lyrics.statusDetail.successWithResults", { count: detail.resultCount })
        : t("settings.lyrics.statusDetail.successNoResults");
    case "partial_failure":
      return t("settings.lyrics.statusDetail.partialFailure", {
        count: detail.resultCount,
        reason: providerErrorLabel(detail.errorKind, t),
      });
    case "failure": {
      const reason = providerErrorLabel(detail.errorKind, t);
      return t("settings.lyrics.statusDetail.failure", {
        reason: detail.statusCode ? `${reason} · HTTP ${detail.statusCode}` : reason,
      });
    }
    case "timeout":
      return t("settings.lyrics.statusDetail.timeout");
    case "cooldown":
      if (detail.requiresConfiguration) return t("settings.lyrics.statusDetail.configurationCooldown");
      if (detail.retryAfterMs !== null) {
        return t("settings.lyrics.statusDetail.cooldown", {
          seconds: Math.max(1, Math.ceil(detail.retryAfterMs / 1_000)),
        });
      }
      return t("settings.lyrics.statusDetail.cooldownUnknown");
  }
}

export default function LyricsSettingsPage() {
  const { t } = useTranslation();
  const { config, setLyricsChineseConversion, setLyricsJapaneseRepairEnabled } = useAppConfig();
  const [titleFilterDraft, setTitleFilterDraft] = useState("");
  const [savingTitleFilters, setSavingTitleFilters] = useState(false);
  const [libraryDir, setLibraryDir] = useState<string | null>(null);
  const [scanStatus, setScanStatus] = useState<LibraryScanStatus | null>(null);
  const [startingScanPath, setStartingScanPath] = useState<string | null>(null);
  const [changingDirectory, setChangingDirectory] = useState(false);
  const [providerConfig, setProviderConfig] = useState<"musixmatch" | "amll_ttml" | null>(null);
  const [musixmatchTokenDraft, setMusixmatchTokenDraft] = useState("");
  const [musixmatchTokenType, setMusixmatchTokenType] = useState<MusixmatchTokenType>("desktopUserToken");
  const [amllBaseUrlDraft, setAmllBaseUrlDraft] = useState("");
  const [savingProviderConfig, setSavingProviderConfig] = useState(false);
  const [matchWeightsDraft, setMatchWeightsDraft] = useState<MatchWeights>(defaultMatchWeights);
  const [savingMatchRules, setSavingMatchRules] = useState(false);
  const [savingChineseConversion, setSavingChineseConversion] = useState(false);
  const [savingJapaneseRepair, setSavingJapaneseRepair] = useState(false);
  const [addingLibraryRoot, setAddingLibraryRoot] = useState(false);
  const {
    playback, lyrics, fileInput, providerRows, providerView, providerCredentials, testingProvider,
    libraryRoots, setLibraryRoots,
    resettingSection, confirmingReset, providerDrag, savingProviderOrder,
    saveProviderSettings, saveMusixmatchToken, clearMusixmatchToken,
    beginProviderDrag, continueProviderDrag, finishProviderDrag,
    setProviderDrag, providerDragTransform, toggleProvider, testProviders, testAllProviders, handleFile,
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
    if (providerView) setMatchWeightsDraft(providerView.settings.matchWeights);
  }, [providerView]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    const acceptStatus = (status: LibraryScanStatus) => {
      setScanStatus((current) => !current || status.scanId >= current.scanId ? status : current);
      setLibraryDir((current) => current ?? status.libraryDir);
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
    setStartingScanPath(selected);
    setError(null);
    try {
      const status = await api.setLyricsDirectory(selected);
      setLibraryDir(status.libraryDir);
      setScanStatus(status);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setChangingDirectory(false);
      setStartingScanPath(null);
    }
  };

  const updateLibraryRoot = (next: LibraryRootView) => {
    setLibraryRoots((current) => current.some((root) => root.rootId === next.rootId)
      ? current.map((root) => root.rootId === next.rootId ? next : root)
      : [...current, next]);
  };

  const addLocalRoot = async () => {
    const selected = await open({ directory: true, multiple: false, title: t("settings.lyrics.chooseLocalRoot") });
    if (!selected || Array.isArray(selected)) return;
    setAddingLibraryRoot(true);
    setStartingScanPath(selected);
    setError(null);
    try {
      const root = await api.addLibraryRoot(selected);
      updateLibraryRoot(root);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setAddingLibraryRoot(false);
      setStartingScanPath(null);
    }
  };

  const toggleLibraryRoot = async (root: LibraryRootView) => {
    const startsScan = !root.enabled;
    if (startsScan) setStartingScanPath(root.path);
    setError(null);
    try {
      const next = await api.setLibraryRootEnabled(root.rootId, !root.enabled);
      updateLibraryRoot(next);
      if (!next.enabled) {
        setScanStatus((current) => current?.libraryDir === root.path ? null : current);
      }
    } catch (error) {
      setError(messageOf(error));
    } finally {
      if (startsScan) setStartingScanPath(null);
    }
  };

  const removeLibraryRoot = async (root: LibraryRootView) => {
    setError(null);
    try {
      await api.removeLibraryRoot(root.rootId);
      setLibraryRoots((current) => current.filter((item) => item.rootId !== root.rootId));
      setScanStatus((current) => current?.libraryDir === root.path ? null : current);
    } catch (error) {
      setError(messageOf(error));
    }
  };

  const rescanRoot = async (root: LibraryRootView) => {
    setStartingScanPath(root.path);
    setError(null);
    try {
      setScanStatus(await api.rescanLibraryRoot(root.rootId));
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setStartingScanPath(null);
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

  const updateNormalizeChinese = async (normalizeChinese: boolean) => {
    if (!providerView || savingMatchRules) return;
    setSavingMatchRules(true);
    await saveProviderSettings({ ...providerView.settings, normalizeChinese });
    setSavingMatchRules(false);
  };

  const updatePreferCapabilities = async (preferCapabilities: boolean) => {
    if (!providerView || savingMatchRules) return;
    setSavingMatchRules(true);
    await saveProviderSettings({ ...providerView.settings, preferCapabilities });
    setSavingMatchRules(false);
  };

  const commitCapabilityPreferenceTolerance = async (capabilityPreferenceTolerance: number) => {
    if (!providerView) return;
    setSavingMatchRules(true);
    const saved = await saveProviderSettings({ ...providerView.settings, capabilityPreferenceTolerance });
    setSavingMatchRules(false);
    if (!saved) throw new Error("failed to save capability preference tolerance");
  };

  const updateChineseConversion = async (conversion: string) => {
    if (savingChineseConversion) return;
    setSavingChineseConversion(true);
    setError(null);
    try {
      await setLyricsChineseConversion(conversion as ChineseConversion);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSavingChineseConversion(false);
    }
  };

  const updateJapaneseRepair = async (enabled: boolean) => {
    if (savingJapaneseRepair) return;
    setSavingJapaneseRepair(true);
    setError(null);
    try {
      await setLyricsJapaneseRepairEnabled(enabled);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSavingJapaneseRepair(false);
    }
  };

  const commitAutoSearchDebounce = async (seconds: number) => {
    if (!providerView) return;
    const autoSearchDebounceMs = Math.round(seconds * 1_000 / 100) * 100;
    const saved = await saveProviderSettings({ ...providerView.settings, autoSearchDebounceMs });
    if (!saved) throw new Error("failed to save lyric search debounce");
  };

  const commitScoringNumber = async (
    key: "autoApplyThreshold" | "maxCandidatesPerProvider",
    value: number,
  ) => {
    if (!providerView) return;
    const settings = { ...providerView.settings, [key]: Math.round(value) };
    setSavingMatchRules(true);
    const saved = await saveProviderSettings(settings);
    setSavingMatchRules(false);
    if (!saved) throw new Error("failed to save lyric scoring setting");
  };

  const previewMatchWeight = (key: MatchWeightKey, value: number) => {
    setMatchWeightsDraft((current) => ({ ...current, [key]: Math.round(value) }));
  };

  const commitMatchWeight = async (key: MatchWeightKey, value: number) => {
    if (!providerView) return;
    const previous = providerView.settings.matchWeights;
    const matchWeights = { ...previous, [key]: Math.round(value) };
    if (matchWeightKeys.every((item) => matchWeights[item] === 0)) {
      setMatchWeightsDraft(previous);
      setError(t("settings.lyrics.matchWeightsEmpty"));
      throw new Error("match weights cannot all be zero");
    }
    setSavingMatchRules(true);
    const saved = await saveProviderSettings({
      ...providerView.settings,
      matchWeights,
    });
    setSavingMatchRules(false);
    if (!saved) {
      setMatchWeightsDraft(previous);
      throw new Error("failed to save lyric match weight");
    }
  };

  const matchWeightTotal = matchWeightKeys.reduce((sum, key) => sum + matchWeightsDraft[key], 0);
  const matchWeightPercentage = (key: MatchWeightKey) => matchWeightTotal === 0
    ? 0
    : Math.round(matchWeightsDraft[key] / matchWeightTotal * 1_000) / 10;

  const scanProgress = scanStatus?.phase === "indexing" && scanStatus.total
    ? Math.round(scanStatus.processed / scanStatus.total * 100)
    : 0;
  const scanInProgress = scanStatus?.phase === "discovering" || scanStatus?.phase === "indexing";
  const isScanning = startingScanPath !== null || scanInProgress;
  const activeScanPath = startingScanPath ?? (scanInProgress ? scanStatus?.libraryDir ?? null : null);
  const providerManifests = providerView?.manifests ?? [];
  const providerPreferences = providerView?.settings.providers ?? [];
  const providerEntries = providerPreferences.map((provider, index) => ({
    provider,
    index,
    manifest: providerManifests.find((manifest) => manifest.id === provider.id),
  }));
  const sections = [
    { id: "lyrics-current-track", label: t("settings.lyrics.currentTrack") },
    { id: "lyrics-output", label: t("settings.lyrics.output") },
    { id: "lyrics-directory", label: t("settings.lyrics.directory") },
    { id: "lyrics-auto-match", label: t("settings.lyrics.autoMatch") },
    { id: "lyrics-match-rules", label: t("settings.lyrics.matchRules") },
    { id: "lyrics-title-filters", label: t("settings.lyrics.titleFilters") },
    { id: "lyrics-provider-priority", label: t("settings.lyrics.providerPriority") },
  ];
  const scanStatusView = scanStatus && <div className={styles.scanStatus} data-phase={scanStatus.phase}>
    <small className={styles.scanPath}>{scanStatus.libraryDir}</small>
    {scanStatus.phase === "discovering" && <><Progress className="animate-pulse" value={100} /><strong>{t("settings.lyrics.scanDiscovering")}</strong><span>{t("settings.lyrics.scanDiscovered", { discovered: scanStatus.discovered, skipped: scanStatus.skipped })}</span></>}
    {scanStatus.phase === "indexing" && <><Progress value={scanProgress} /><strong>{t("settings.lyrics.scanIndexing", { processed: scanStatus.processed, total: scanStatus.total ?? 0 })}</strong><span>{t("settings.lyrics.scanLiveStats", scanStatus)}</span></>}
    {scanStatus.phase === "completed" && <><strong>{t("settings.lyrics.scanCompleted")}</strong><span>{t("settings.lyrics.scanSummary", scanStatus)}</span>{scanStatus.firstFailure && <small>{scanStatus.firstFailure}</small>}</>}
    {scanStatus.phase === "failed" && <><strong>{t("settings.lyrics.scanFailed")}</strong><span role="alert">{scanStatus.error}</span></>}
  </div>;
  const managedRootPath = libraryRoots.find((root) => root.rootKind === "managed")?.path ?? libraryDir;
  const isManagedScan = scanStatus?.libraryDir === managedRootPath;
  const scanStatusTargetExists = Boolean(scanStatus && libraryRoots.some((root) =>
    (root.rootKind === "managed" || (root.rootKind === "local" && root.enabled))
    && root.path === scanStatus.libraryDir
  ));

  return <SettingsPage sections={sections}>
    <PageHeader title={t("settings.lyrics.title")} description={t("settings.lyrics.description")} onReset={() => void resetSection("lyrics")} resetting={resettingSection === "lyrics"} confirming={confirmingReset === "lyrics"} />
    <SettingsSection id="lyrics-current-track" title={t("settings.lyrics.currentTrack")}>
      <div className={styles.currentTrack}><div><strong>{playback.snapshot.title ?? t("settings.lyrics.noTrack")}</strong><small>{playback.snapshot.artist ?? "—"}</small></div><em>{lyrics.document ? localizedSource(lyrics.document.metadata.source, t) : t("settings.lyrics.notAssociated")}</em></div>
      <div className={styles.buttonRow}>
        <Button variant="secondary" size="sm" disabled={!lyrics.trackKey} onClick={() => void api.showQuickLyricsWindow().catch((error) => setError(messageOf(error)))}>{t("settings.lyrics.manualSearch")}</Button>
        <Button variant="secondary" size="sm" disabled={!lyrics.trackKey} onClick={() => fileInput.current?.click()}>{t("settings.lyrics.importLrc")}</Button>
        <input ref={fileInput} hidden type="file" accept=".lrc,.lyricsfile.yaml,text/plain,application/yaml,text/yaml" onChange={(event) => void handleFile(event.currentTarget.files?.[0])} />
        {lyrics.document && <Button variant="destructive" size="sm" onClick={() => void lyrics.remove()}>{t("settings.lyrics.unlink")}</Button>}
      </div>
      {lyrics.document && <div className={styles.offsetRow}><span>{t("settings.lyrics.offset", { value: `${lyrics.document.offsetMs > 0 ? "+" : ""}${lyrics.document.offsetMs}` })}</span><div><Button variant="outline" size="sm" onClick={() => void lyrics.changeOffset(-100)}>−100</Button><Button variant="outline" size="sm" onClick={() => void lyrics.changeOffset(100)}>+100</Button><Button variant="outline" size="sm" onClick={() => void lyrics.setOffset(0)}>{t("common.actions.reset")}</Button></div></div>}
    </SettingsSection>
    <SettingsSection id="lyrics-output" title={t("settings.lyrics.output")}>
      <SelectRow
        label={t("settings.lyrics.chineseConversion")}
        description={t("settings.lyrics.chineseConversionHint")}
        value={config.lyrics.chineseConversion}
        options={[
          ["original", t("settings.lyrics.chineseConversionOriginal")],
          ["simplified", t("settings.lyrics.chineseConversionSimplified")],
          ["traditional", t("settings.lyrics.chineseConversionTraditional")],
        ]}
        disabled={savingChineseConversion}
        onChange={(value) => void updateChineseConversion(value)}
      />
      <ToggleRow
        label={t("settings.lyrics.repairSimplifiedJapanese")}
        description={t("settings.lyrics.repairSimplifiedJapaneseHint")}
        value={config.lyrics.repairSimplifiedJapanese}
        disabled={savingJapaneseRepair}
        onChange={updateJapaneseRepair}
      />
    </SettingsSection>
    <SettingsSection id="lyrics-directory" title={t("settings.lyrics.directory")}>
      <div className={styles.directoryBlock}>
        <div className={styles.sectionSubheading}><strong>{t("settings.lyrics.managedDirectory")}</strong></div>
        <p className={styles.directoryPath} title={libraryDir ?? undefined}>{libraryDir ?? t("library.loadingDirectory")}</p>
        <div className={styles.buttonRow}>
          <Button variant="secondary" size="sm" disabled={!libraryDir} onClick={() => void api.openLyricsDirectory().catch((error) => setError(messageOf(error)))}>{t("library.openFolder")}</Button>
          <Button variant="secondary" size="sm" disabled={changingDirectory || isScanning} onClick={() => void changeDirectory()}>{changingDirectory ? t("library.changing") : t("library.changeFolder")}</Button>
        </div>
        {scanStatusTargetExists && isManagedScan && scanStatusView}
      </div>
      <div className={styles.directoryBlock}>
        <div className={styles.sectionSubheading}><strong>{t("settings.lyrics.localRoots")}</strong><Button variant="secondary" size="sm" disabled={addingLibraryRoot || isScanning} onClick={() => void addLocalRoot()}><FolderPlus data-icon="inline-start" />{addingLibraryRoot ? t("settings.lyrics.addingLocalRoot") : t("settings.lyrics.addLocalRoot")}</Button></div>
        {scanStatusTargetExists && !isManagedScan && scanStatusView}
        <div className={styles.libraryRoots}>
        {libraryRoots.filter((root) => root.rootKind === "local").map((root) => <Item variant="muted" className={styles.libraryRoot} key={root.rootId}>
          <ItemContent><ItemTitle>{root.displayName}</ItemTitle><ItemDescription title={root.path}>{root.path}<br />{t("settings.lyrics.rootStats", { files: root.fileCount, unavailable: root.unavailableCount })}{root.lastScanAt ? ` · ${new Date(root.lastScanAt * 1000).toLocaleString()}` : ""}</ItemDescription>{root.lastError && <small role="alert">{root.lastError}</small>}</ItemContent>
          <ItemActions><Switch aria-label={root.displayName} checked={root.enabled} disabled={isScanning} onCheckedChange={() => void toggleLibraryRoot(root)} /><Button variant="ghost" size="icon-sm" disabled={!root.enabled || isScanning} onClick={() => void rescanRoot(root)} aria-label={t("settings.lyrics.rescanRoot")}><RefreshCw className={activeScanPath === root.path ? "animate-spin" : undefined} /></Button><Button variant="ghost" size="icon-sm" disabled={isScanning} onClick={() => void removeLibraryRoot(root)} aria-label={t("settings.lyrics.removeRoot")}><Trash2 /></Button></ItemActions>
        </Item>)}
        </div>
      </div>
    </SettingsSection>
    <SettingsSection id="lyrics-auto-match" title={t("settings.lyrics.autoMatch")}>
      <p className={styles.cardHint}>{t("settings.lyrics.thresholdHint")}</p>
      <RangeRow label={t("settings.lyrics.autoApplyThreshold")} value={providerView?.settings.autoApplyThreshold ?? 60} min={0} max={100} suffix="%" disabled={!providerView || savingMatchRules} onChange={() => undefined} onValueCommitted={(value) => commitScoringNumber("autoApplyThreshold", value)} />
      <RangeRow label={t("settings.lyrics.autoSearchDebounce")} value={(providerView?.settings.autoSearchDebounceMs ?? 2000) / 1000} min={0} max={5} step={0.1} suffix="s" disabled={!providerView} onChange={() => undefined} onValueCommitted={commitAutoSearchDebounce} />
      <p className={styles.cardHint}>{t("settings.lyrics.autoSearchDebounceHint")}</p>
      <RangeRow label={t("settings.lyrics.maxCandidatesPerProvider")} value={providerView?.settings.maxCandidatesPerProvider ?? defaultMaxCandidatesPerProvider} min={1} max={100} suffix={t("settings.lyrics.candidateCountUnit")} disabled={!providerView || savingMatchRules} onChange={() => undefined} onValueCommitted={(value) => commitScoringNumber("maxCandidatesPerProvider", value)} />
      <p className={styles.cardHint}>{t("settings.lyrics.maxCandidatesPerProviderHint")}</p>
    </SettingsSection>
    <SettingsSection id="lyrics-match-rules" title={t("settings.lyrics.matchRules")}>
      <ToggleRow label={t("settings.lyrics.preferCapabilities")} description={t("settings.lyrics.preferCapabilitiesHint")} value={providerView?.settings.preferCapabilities ?? true} disabled={!providerView || savingMatchRules} onChange={updatePreferCapabilities} />
      <RangeRow
        label={t("settings.lyrics.capabilityPreferenceTolerance")}
        value={providerView?.settings.capabilityPreferenceTolerance ?? defaultCapabilityPreferenceTolerance}
        min={0}
        max={20}
        suffix="%"
        disabled={!providerView || savingMatchRules || providerView.settings.mode !== "smart" || !providerView.settings.preferCapabilities}
        onChange={() => undefined}
        onValueCommitted={commitCapabilityPreferenceTolerance}
      />
      <p className={styles.cardHint}>{t("settings.lyrics.capabilityPreferenceToleranceHint")}</p>
      <ToggleRow label={t("settings.lyrics.normalizeChinese")} description={t("settings.lyrics.normalizeChineseHint")} value={providerView?.settings.normalizeChinese ?? true} disabled={!providerView || savingMatchRules} onChange={updateNormalizeChinese} />
      <div className={styles.matchWeights}>
        {matchWeightKeys.map((key) => <RangeRow key={key} label={t(`settings.lyrics.matchWeight.${key}`)} value={providerView?.settings.matchWeights[key] ?? defaultMatchWeights[key]} displayValue={matchWeightPercentage(key)} min={0} max={100} suffix="%" disabled={!providerView || savingMatchRules} onChange={() => undefined} onValuePreview={(value) => previewMatchWeight(key, value)} onValueCommitted={(value) => commitMatchWeight(key, value)} onPreviewCanceled={() => setMatchWeightsDraft(providerView?.settings.matchWeights ?? defaultMatchWeights)} />)}
      </div>
    </SettingsSection>
    <SettingsSection id="lyrics-title-filters" title={t("settings.lyrics.titleFilters")} trailing={<Button variant="ghost" size="sm" disabled={savingTitleFilters} onClick={() => void (providerView && saveProviderSettings({ ...providerView.settings, titleFilterKeywords: defaultTitleFilterKeywords }))}>{t("settings.lyrics.restoreTitleFilters")}</Button>}>
      <p className={styles.cardHint}>{t("settings.lyrics.titleFiltersHint")}</p>
      <div className={styles.titleFilters}>{providerView?.settings.titleFilterKeywords.length ? providerView.settings.titleFilterKeywords.map((keyword, index) => <Badge variant="secondary" className={styles.titleFilter} key={`${keyword}-${index}`}><span>{keyword}</span><IconButton label={`${t("common.actions.remove")} ${keyword}`} variant="ghost" size="icon-sm" disabled={savingTitleFilters} onClick={() => void removeTitleFilter(index)}><X /></IconButton></Badge>) : <p>{t("settings.lyrics.titleFiltersEmpty")}</p>}</div>
      <form className={styles.titleFilterForm} onSubmit={(event) => void addTitleFilter(event)}><InputGroup><InputGroupInput aria-invalid={Boolean(titleFilterError)} placeholder={t("settings.lyrics.titleFilterPlaceholder")} value={titleFilterDraft} onChange={(event) => setTitleFilterDraft(event.target.value)} /><InputGroupAddon align="inline-end"><Button size="sm" disabled={!providerView || !normalizedTitleFilterDraft || Boolean(titleFilterError) || savingTitleFilters}>{t("settings.lyrics.addTitleFilter")}</Button></InputGroupAddon></InputGroup>{titleFilterError && <small role="alert">{titleFilterError}</small>}</form>
    </SettingsSection>
    <SettingsSection id="lyrics-provider-priority" title={t("settings.lyrics.providerPriority")} trailing={providerView && <div className={styles.shortcutControls}><Button variant="secondary" size="sm" disabled={!lyrics.trackKey || !providerView.settings.providers.some((provider) => provider.enabled) || testingProvider !== null} onClick={() => void testAllProviders()}>{testingProvider === "*" ? t("common.actions.testing") : t("common.actions.testAll")}</Button><Select disabled={savingProviderOrder} items={[{ value: "strict", label: t("settings.lyrics.strict") }, { value: "smart", label: t("settings.lyrics.smart") }]} value={providerView.settings.mode} onValueChange={(mode) => void saveProviderSettings({ ...providerView.settings, mode: mode as ProviderSettings["mode"] })}><SelectTrigger className="w-32" aria-label={t("settings.lyrics.providerPriority")}><SelectValue /></SelectTrigger><SelectContent><SelectGroup><SelectItem value="strict">{t("settings.lyrics.strict")}</SelectItem><SelectItem value="smart">{t("settings.lyrics.smart")}</SelectItem></SelectGroup></SelectContent></Select></div>}>
      <p className={styles.cardHint}>{providerView?.settings.mode === "smart" ? t("settings.lyrics.smartHint") : t("settings.lyrics.strictHint")}</p>
      <p className={styles.cardHint}>{t("settings.lyrics.sourceCandidateHint")}</p>
      <ItemGroup className={styles.providers} data-dragging={Boolean(providerDrag)}>{providerEntries.map(({ provider, index, manifest }) => {
          if (!manifest) return null;
          const status = providerView?.statuses.find((item) => item.providerId === provider.id);
          const providerName = manifest.displayName;
          const detail = providerDetailLabel(status, t);
          return <Item variant="muted" className={styles.provider} data-dragging={providerDrag?.providerId === provider.id} key={provider.id} ref={(element) => { if (element) providerRows.current.set(provider.id, element); else providerRows.current.delete(provider.id); }} style={{ transform: providerDragTransform(index) }}>
            <ItemMedia><Button type="button" variant="ghost" size="icon-sm" className={styles.dragHandle} aria-label={`${providerName} #${index + 1}`} disabled={savingProviderOrder} onPointerDown={(event) => beginProviderDrag(provider.id, index, event)} onPointerMove={continueProviderDrag} onPointerUp={finishProviderDrag} onPointerCancel={() => setProviderDrag(null)} onLostPointerCapture={() => setProviderDrag(null)}><GripVertical /></Button></ItemMedia>
            <Badge variant="outline">#{index + 1}</Badge>
            <ItemContent><ItemTitle>{providerName}</ItemTitle><ItemDescription className={styles.providerStatus} data-health={status?.health ?? "unknown"}>{healthLabel(status, t)} · {detail}</ItemDescription></ItemContent>
            <ItemActions>
              {(provider.id === "musixmatch" || provider.id === "amll_ttml") && <IconButton label={t("settings.lyrics.providerConfig.configure", { source: providerName })} tooltip={t("settings.lyrics.providerConfig.configure", { source: providerName })} variant="ghost" size="icon-sm" onClick={() => openProviderConfig(provider.id)}><Settings2 /></IconButton>}
              <Switch aria-label={providerName} checked={provider.enabled} onCheckedChange={() => toggleProvider(provider.id)} />
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
