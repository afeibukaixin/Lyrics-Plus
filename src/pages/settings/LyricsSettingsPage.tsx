import type { ProviderSettings, ProviderStatus } from "../../shared/types";
import type { TFunction } from "i18next";
import { useState, type FormEvent } from "react";
import { useTranslation } from "react-i18next";
import { localizedSource } from "../../features/i18n/userText";
import { api, messageOf } from "../../shared/api";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { RangeRow, SettingsCard, SettingsHeading } from "./components";
import { UiIcon } from "../../components/UiIcon";

function healthLabel(status: ProviderStatus | undefined, t: TFunction) {
  return t(`settings.lyrics.health.${status?.health ?? "unknown"}`);
}

export default function LyricsSettingsPage() {
  const { t } = useTranslation();
  const [titleFilterDraft, setTitleFilterDraft] = useState("");
  const [savingTitleFilters, setSavingTitleFilters] = useState(false);
  const {
    playback,
    lyrics,
    fileInput,
    providerRows,
    providerView,
    testingProvider,
    resettingSection,
    confirmingReset,
    providerDrag,
    savingProviderOrder,
    saveProviderSettings,
    beginProviderDrag,
    continueProviderDrag,
    finishProviderDrag,
    setProviderDrag,
    providerDragTransform,
    toggleProvider,
    testProviders,
    handleFile,
    resetSection,
    setError,
  } = useSettingsContext();

  const lyricCapabilities = lyrics.document
    ? [
        lyrics.document.tracks.translation ? t("common.feature.hasTranslation") : t("common.feature.noTranslation"),
        lyrics.document.tracks.romanization ? t("common.feature.hasRomanization") : t("common.feature.noRomanization"),
        lyrics.document.tracks.original.lines.some((line) => line.words?.length) ? t("common.feature.hasWordTiming") : t("common.feature.noWordTiming"),
      ].join(" · ")
    : t("settings.common.capabilitiesHint");

  const addTitleFilter = async (event: FormEvent) => {
    event.preventDefault();
    if (!providerView || !titleFilterDraft.trim() || savingTitleFilters) return;
    setSavingTitleFilters(true);
    const saved = await saveProviderSettings({
      ...providerView.settings,
      titleFilterKeywords: [...providerView.settings.titleFilterKeywords, titleFilterDraft],
    });
    if (saved) setTitleFilterDraft("");
    setSavingTitleFilters(false);
  };

  const removeTitleFilter = async (index: number) => {
    if (!providerView || savingTitleFilters) return;
    setSavingTitleFilters(true);
    await saveProviderSettings({
      ...providerView.settings,
      titleFilterKeywords: providerView.settings.titleFilterKeywords.filter((_, itemIndex) => itemIndex !== index),
    });
    setSavingTitleFilters(false);
  };

  return (
    <>
      <SettingsHeading title={t("settings.lyrics.title")} description={t("settings.lyrics.description")} onReset={() => void resetSection("lyrics")} resetting={resettingSection === "lyrics"} confirming={confirmingReset === "lyrics"} />
      <SettingsCard title={t("settings.lyrics.autoMatch")}>
        <RangeRow label={t("settings.lyrics.threshold")} value={providerView?.settings.autoApplyThreshold ?? 60} min={0} max={100} suffix="%" onChange={(autoApplyThreshold) => {
          if (providerView) void saveProviderSettings({ ...providerView.settings, autoApplyThreshold });
        }} />
        <p className={styles.cardHint}>{t("settings.lyrics.thresholdHint")}</p>
      </SettingsCard>
      <SettingsCard title={t("settings.lyrics.titleFilters")}>
        <p className={styles.cardHint}>{t("settings.lyrics.titleFiltersHint")}</p>
        <div className={styles.titleFilters}>
          {providerView?.settings.titleFilterKeywords.length
            ? providerView.settings.titleFilterKeywords.map((keyword, index) => <div className={styles.titleFilter} key={`${keyword}-${index}`}>
                <span>{keyword}</span>
                <button type="button" disabled={savingTitleFilters} aria-label={t("settings.lyrics.removeTitleFilter", { index: index + 1 })} onClick={() => void removeTitleFilter(index)}><UiIcon name="close" /></button>
              </div>)
            : <p>{t("settings.lyrics.titleFiltersEmpty")}</p>}
        </div>
        <form className={styles.titleFilterForm} onSubmit={(event) => void addTitleFilter(event)}>
          <input aria-label={t("settings.lyrics.titleFilterPlaceholder")} placeholder={t("settings.lyrics.titleFilterPlaceholder")} spellCheck={false} value={titleFilterDraft} onChange={(event) => setTitleFilterDraft(event.target.value)} />
          <button disabled={!providerView || !titleFilterDraft.trim() || savingTitleFilters}>{t("settings.lyrics.addTitleFilter")}</button>
        </form>
      </SettingsCard>
      <SettingsCard title={t("settings.lyrics.currentTrack")}>
        <div className={styles.currentTrack}><div><strong>{playback.snapshot.title ?? t("settings.lyrics.noTrack")}</strong><small>{playback.snapshot.artist ?? "—"}</small></div><em>{lyrics.document ? localizedSource(lyrics.document.metadata.source, t) : t("settings.lyrics.notAssociated")}</em></div>
        <p className={styles.cardHint}>{lyricCapabilities}</p>
        <div className={styles.buttonRow}>
          <button disabled={!lyrics.trackKey} onClick={() => void api.showQuickLyricsWindow().catch((error) => setError(messageOf(error)))}>{t("settings.lyrics.manualSearch")}</button>
          <button disabled={!lyrics.trackKey} onClick={() => fileInput.current?.click()}>{t("settings.lyrics.importLrc")}</button>
          <input ref={fileInput} hidden type="file" accept=".lrc,text/plain" onChange={(event) => void handleFile(event.currentTarget.files?.[0])} />
          {lyrics.document && <button className={styles.danger} onClick={() => void lyrics.remove()}>{t("settings.lyrics.unlink")}</button>}
        </div>
        {lyrics.document && <div className={styles.offsetRow}><span>{t("settings.lyrics.offset", { value: `${lyrics.document.offsetMs > 0 ? "+" : ""}${lyrics.document.offsetMs}` })}</span><div><button onClick={() => void lyrics.changeOffset(-100)}>−100</button><button onClick={() => void lyrics.changeOffset(100)}>+100</button><button onClick={() => void lyrics.setOffset(0)}>{t("common.actions.reset")}</button></div></div>}
      </SettingsCard>
      <SettingsCard title={t("settings.lyrics.providerPriority")} trailing={providerView && <div className={styles.shortcutControls}><button className={styles.cardHeaderButton} disabled={!providerView.settings.providers.length || testingProvider !== null} onClick={() => void testProviders(providerView.settings.providers.map((provider) => provider.id))}>{testingProvider === "*" ? t("common.actions.testing") : t("common.actions.testAll")}</button><select aria-label={t("settings.lyrics.providerPriority")} disabled={savingProviderOrder} value={providerView.settings.mode} onChange={(event) => void saveProviderSettings({ ...providerView.settings, mode: event.target.value as ProviderSettings["mode"] })}><option value="strict">{t("settings.lyrics.strict")}</option><option value="smart">{t("settings.lyrics.smart")}</option></select></div>}>
        <p className={styles.cardHint}>{providerView?.settings.mode === "smart" ? t("settings.lyrics.smartHint") : t("settings.lyrics.strictHint")}</p>
        <p className={styles.cardHint}>{t("settings.lyrics.onlineProviderNotice")}</p>
        <div className={styles.providers} data-dragging={Boolean(providerDrag)} aria-busy={savingProviderOrder || testingProvider !== null}>{providerView?.settings.providers.map((provider, index) => {
          const status = providerView.statuses.find((item) => item.providerId === provider.id);
          return <div className={styles.provider} data-dragging={providerDrag?.providerId === provider.id} key={provider.id} ref={(element) => { if (element) providerRows.current.set(provider.id, element); else providerRows.current.delete(provider.id); }} style={{ transform: providerDragTransform(index) }}>
            <button type="button" className={styles.dragHandle} disabled={savingProviderOrder} aria-label={t("settings.lyrics.dragProvider", { provider: status?.name ?? provider.id })} onPointerDown={(event) => beginProviderDrag(provider.id, index, event)} onPointerMove={continueProviderDrag} onPointerUp={finishProviderDrag} onPointerCancel={() => setProviderDrag(null)} onLostPointerCapture={() => setProviderDrag(null)}><UiIcon name="drag" /></button>
            <b>#{index + 1}</b><div><strong>{status?.name ?? provider.id}</strong><small data-health={status?.health ?? "unknown"} title={status?.message ?? undefined}>{healthLabel(status, t)} · {t(`settings.lyrics.healthHint.${status?.health ?? "unknown"}`)}{status?.message ? ` · ${status.message}` : ""}</small></div>
            <button aria-label={status?.name ?? provider.id} aria-pressed={provider.enabled} className={styles.switch} disabled={savingProviderOrder} data-on={provider.enabled} onClick={() => toggleProvider(provider.id)}><span /></button>
            <button disabled={testingProvider !== null} onClick={() => void testProviders([provider.id])}>{testingProvider === provider.id || testingProvider === "*" ? t("common.actions.testing") : t("common.actions.test")}</button>
          </div>;
        })}</div>
      </SettingsCard>
    </>
  );
}
