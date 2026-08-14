import type { ProviderSettings, ProviderStatus } from "../../shared/types";
import type { TFunction } from "i18next";
import { useEffect, useState, type FormEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { localizedSource } from "../../features/i18n/userText";
import { api, messageOf } from "../../shared/api";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { RangeRow, SettingsCard, SettingsHeading } from "./components";
import { GripVertical, X } from "lucide-react";
import { Button } from "../../components/ui/button";

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
    void api.getLibraryScanStatus().then((status) => setLibraryDir(status.libraryDir)).catch((error) => setError(messageOf(error)));
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
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setChangingDirectory(false);
    }
  };

  return <>
    <SettingsHeading title={t("settings.lyrics.title")} description={t("settings.lyrics.description")} onReset={() => void resetSection("lyrics")} resetting={resettingSection === "lyrics"} confirming={confirmingReset === "lyrics"} />
    <SettingsCard title={t("settings.lyrics.autoMatch")}>
      <RangeRow label={t("settings.lyrics.threshold")} value={providerView?.settings.autoApplyThreshold ?? 60} min={0} max={100} suffix="%" onChange={(autoApplyThreshold) => { if (providerView) void saveProviderSettings({ ...providerView.settings, autoApplyThreshold }); }} />
      <p className={styles.cardHint}>{t("settings.lyrics.thresholdHint")}</p>
    </SettingsCard>
    <SettingsCard title={t("settings.lyrics.currentTrack")}>
      <div className={styles.currentTrack}><div><strong>{playback.snapshot.title ?? t("settings.lyrics.noTrack")}</strong><small>{playback.snapshot.artist ?? "—"}</small></div><em>{lyrics.document ? localizedSource(lyrics.document.metadata.source, t) : t("settings.lyrics.notAssociated")}</em></div>
      <div className={styles.buttonRow}>
        <button disabled={!lyrics.trackKey} onClick={() => void api.showQuickLyricsWindow().catch((error) => setError(messageOf(error)))}>{t("settings.lyrics.manualSearch")}</button>
        <button disabled={!lyrics.trackKey} onClick={() => fileInput.current?.click()}>{t("settings.lyrics.importLrc")}</button>
        <input ref={fileInput} hidden type="file" accept=".lrc,text/plain" onChange={(event) => void handleFile(event.currentTarget.files?.[0])} />
        {lyrics.document && <button className={styles.danger} onClick={() => void lyrics.remove()}>{t("settings.lyrics.unlink")}</button>}
      </div>
      {lyrics.document && <div className={styles.offsetRow}><span>{t("settings.lyrics.offset", { value: `${lyrics.document.offsetMs > 0 ? "+" : ""}${lyrics.document.offsetMs}` })}</span><div><button onClick={() => void lyrics.changeOffset(-100)}>−100</button><button onClick={() => void lyrics.changeOffset(100)}>+100</button><button onClick={() => void lyrics.setOffset(0)}>{t("common.actions.reset")}</button></div></div>}
    </SettingsCard>
    <SettingsCard title={t("settings.lyrics.directory")}>
      <p className={styles.directoryPath}>{libraryDir ?? t("library.loadingDirectory")}</p>
      <div className={styles.buttonRow}>
        <button disabled={!libraryDir} onClick={() => void api.openLyricsDirectory().catch((error) => setError(messageOf(error)))}>{t("library.openFolder")}</button>
        <button disabled={changingDirectory} onClick={() => void changeDirectory()}>{changingDirectory ? t("library.changing") : t("library.changeFolder")}</button>
      </div>
    </SettingsCard>
    <div className={styles.advancedSection}>
      <SettingsCard title={t("settings.lyrics.titleFilters")} trailing={<Button variant="ghost" size="sm" disabled={savingTitleFilters} onClick={() => void (providerView && saveProviderSettings({ ...providerView.settings, titleFilterKeywords: defaultTitleFilterKeywords }))}>{t("settings.lyrics.restoreTitleFilters")}</Button>}>
        <p className={styles.cardHint}>{t("settings.lyrics.titleFiltersHint")}</p>
        <div className={styles.titleFilters}>{providerView?.settings.titleFilterKeywords.length ? providerView.settings.titleFilterKeywords.map((keyword, index) => <div className={styles.titleFilter} key={`${keyword}-${index}`}><span>{keyword}</span><button type="button" disabled={savingTitleFilters} onClick={() => void removeTitleFilter(index)}><X /></button></div>) : <p>{t("settings.lyrics.titleFiltersEmpty")}</p>}</div>
        <form className={styles.titleFilterForm} onSubmit={(event) => void addTitleFilter(event)}><input aria-invalid={Boolean(titleFilterError)} placeholder={t("settings.lyrics.titleFilterPlaceholder")} value={titleFilterDraft} onChange={(event) => setTitleFilterDraft(event.target.value)} /><button disabled={!providerView || !normalizedTitleFilterDraft || Boolean(titleFilterError) || savingTitleFilters}>{t("settings.lyrics.addTitleFilter")}</button>{titleFilterError && <small role="alert">{titleFilterError}</small>}</form>
      </SettingsCard>
      <SettingsCard title={t("settings.lyrics.providerPriority")} trailing={providerView && <div className={styles.shortcutControls}><button className={styles.cardHeaderButton} disabled={!providerView.settings.providers.length || testingProvider !== null} onClick={() => void testProviders(providerView.settings.providers.map((provider) => provider.id))}>{testingProvider === "*" ? t("common.actions.testing") : t("common.actions.testAll")}</button><select disabled={savingProviderOrder} value={providerView.settings.mode} onChange={(event) => void saveProviderSettings({ ...providerView.settings, mode: event.target.value as ProviderSettings["mode"] })}><option value="strict">{t("settings.lyrics.strict")}</option><option value="smart">{t("settings.lyrics.smart")}</option></select></div>}>
        <p className={styles.cardHint}>{providerView?.settings.mode === "smart" ? t("settings.lyrics.smartHint") : t("settings.lyrics.strictHint")}</p>
        <div className={styles.providers} data-dragging={Boolean(providerDrag)}>{providerView?.settings.providers.map((provider, index) => {
          const status = providerView.statuses.find((item) => item.providerId === provider.id);
          return <div className={styles.provider} data-dragging={providerDrag?.providerId === provider.id} key={provider.id} ref={(element) => { if (element) providerRows.current.set(provider.id, element); else providerRows.current.delete(provider.id); }} style={{ transform: providerDragTransform(index) }}>
            <button type="button" className={styles.dragHandle} disabled={savingProviderOrder} onPointerDown={(event) => beginProviderDrag(provider.id, index, event)} onPointerMove={continueProviderDrag} onPointerUp={finishProviderDrag} onPointerCancel={() => setProviderDrag(null)} onLostPointerCapture={() => setProviderDrag(null)}><GripVertical /></button>
            <b>#{index + 1}</b><div><strong>{status?.name ?? provider.id}</strong><small data-health={status?.health ?? "unknown"}>{healthLabel(status, t)}</small></div>
            <button aria-pressed={provider.enabled} className={styles.switch} data-on={provider.enabled} onClick={() => toggleProvider(provider.id)}><span /></button>
            <button disabled={testingProvider !== null} onClick={() => void testProviders([provider.id])}>{testingProvider === provider.id || testingProvider === "*" ? t("common.actions.testing") : t("common.actions.test")}</button>
          </div>;
        })}</div>
      </SettingsCard>
    </div>
  </>;
}
