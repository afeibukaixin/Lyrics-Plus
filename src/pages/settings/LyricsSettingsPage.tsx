import type { ProviderSettings, ProviderStatus } from "../../shared/types";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import { localizedSource } from "../../features/i18n/userText";
import { api, messageOf } from "../../shared/api";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { RangeRow, SettingsCard, SettingsHeading } from "./components";

function healthLabel(status: ProviderStatus | undefined, t: TFunction) {
  return t(`settings.lyrics.health.${status?.health ?? "unknown"}`);
}

export default function LyricsSettingsPage() {
  const { t } = useTranslation();
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
    testProvider,
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

  return (
    <>
      <SettingsHeading title={t("settings.lyrics.title")} description={t("settings.lyrics.description")} onReset={() => void resetSection("lyrics")} resetting={resettingSection === "lyrics"} confirming={confirmingReset === "lyrics"} />
      <SettingsCard title={t("settings.lyrics.autoMatch")}>
        <RangeRow label={t("settings.lyrics.threshold")} value={providerView?.settings.autoApplyThreshold ?? 60} min={0} max={100} suffix="%" onChange={(autoApplyThreshold) => {
          if (providerView) void saveProviderSettings({ ...providerView.settings, autoApplyThreshold });
        }} />
        <p className={styles.cardHint}>{t("settings.lyrics.thresholdHint")}</p>
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
      <SettingsCard title={t("settings.lyrics.providerPriority")} trailing={providerView && <select aria-label={t("settings.lyrics.providerPriority")} disabled={savingProviderOrder} value={providerView.settings.mode} onChange={(event) => void saveProviderSettings({ ...providerView.settings, mode: event.target.value as ProviderSettings["mode"] })}><option value="strict">{t("settings.lyrics.strict")}</option><option value="smart">{t("settings.lyrics.smart")}</option></select>}>
        <p className={styles.cardHint}>{providerView?.settings.mode === "smart" ? t("settings.lyrics.smartHint") : t("settings.lyrics.strictHint")}</p>
        <div className={styles.providers} data-dragging={Boolean(providerDrag)} aria-busy={savingProviderOrder}>{providerView?.settings.providers.map((provider, index) => {
          const status = providerView.statuses.find((item) => item.providerId === provider.id);
          return <div className={styles.provider} data-dragging={providerDrag?.providerId === provider.id} key={provider.id} ref={(element) => { if (element) providerRows.current.set(provider.id, element); else providerRows.current.delete(provider.id); }} style={{ transform: providerDragTransform(index) }}>
            <button type="button" className={styles.dragHandle} disabled={savingProviderOrder} aria-label={t("settings.lyrics.dragProvider", { provider: status?.name ?? provider.id })} onPointerDown={(event) => beginProviderDrag(provider.id, index, event)} onPointerMove={continueProviderDrag} onPointerUp={finishProviderDrag} onPointerCancel={() => setProviderDrag(null)} onLostPointerCapture={() => setProviderDrag(null)}>⠿</button>
            <b>#{index + 1}</b><div><strong>{status?.name ?? provider.id}</strong><small data-health={status?.health ?? "unknown"}>{healthLabel(status, t)} · {t(`settings.lyrics.healthHint.${status?.health ?? "unknown"}`)}</small></div>
            <button aria-label={status?.name ?? provider.id} aria-pressed={provider.enabled} className={styles.switch} disabled={savingProviderOrder} data-on={provider.enabled} onClick={() => toggleProvider(provider.id)}><span /></button>
            <button disabled={testingProvider === provider.id} onClick={() => void testProvider(provider.id)}>{testingProvider === provider.id ? t("common.actions.testing") : t("common.actions.test")}</button>
          </div>;
        })}</div>
      </SettingsCard>
    </>
  );
}
