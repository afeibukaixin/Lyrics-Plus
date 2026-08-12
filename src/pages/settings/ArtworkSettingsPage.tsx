import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { UiIcon } from "../../components/UiIcon";
import { api, messageOf } from "../../shared/api";
import { itunesCountryForLanguage } from "../../shared/languages";
import type { ArtworkCacheStatus, ArtworkSettings, ArtworkSettingsView, ItunesStorefront } from "../../shared/types";
import { resolveLanguage } from "../../features/i18n/i18n";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { ApplicationList, SelectRow, SettingsCard, SettingsHeading, ToggleRow } from "./components";

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

export default function ArtworkSettingsPage() {
  const { t } = useTranslation();
  const { config, playback, resettingSection, confirmingReset, resetSection, setError, setNotice } = useSettingsContext();
  const [view, setView] = useState<ArtworkSettingsView | null>(null);
  const [cache, setCache] = useState<ArtworkCacheStatus | null>(null);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState<string | null>(null);
  const [dragging, setDragging] = useState<number | null>(null);
  const [applicationIcons, setApplicationIcons] = useState<Record<string, string>>({});
  const [confirmingClear, setConfirmingClear] = useState(false);
  const clearTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    void Promise.all([api.getArtworkSettings(), api.getArtworkCacheStatus()])
      .then(([settings, status]) => { setView(settings); setCache(status); })
      .catch((error) => setError(messageOf(error)));
    return () => { if (clearTimer.current) clearTimeout(clearTimer.current); };
  }, [setError]);

  useEffect(() => {
    setView((current) => current ? { ...current, settings: config.artwork } : current);
  }, [config.artwork]);

  const applications = view?.settings.alwaysNetworkApplications ?? [];
  const autoCountry = itunesCountryForLanguage(resolveLanguage(config.app.language));
  const itunesCountry = view?.settings.itunesStorefront === "auto" || !view
    ? autoCountry
    : view.settings.itunesStorefront;
  const applicationKey = applications.map((application) => application.bundleId).join("\n");
  useEffect(() => {
    if (!applicationKey) {
      setApplicationIcons({});
      return;
    }
    void api.getApplicationIcons(applicationKey.split("\n"))
      .then(setApplicationIcons)
      .catch(() => setApplicationIcons({}));
  }, [applicationKey]);

  const save = async (settings: ArtworkSettings) => {
    setSaving(true);
    setError(null);
    try { setView(await api.setArtworkSettings(settings)); }
    catch (error) { setError(messageOf(error)); }
    finally { setSaving(false); }
  };

  const moveProvider = (target: number) => {
    if (dragging == null || !view || dragging === target) return;
    const providers = [...view.settings.providers];
    const [provider] = providers.splice(dragging, 1);
    providers.splice(target, 0, provider);
    setDragging(null);
    void save({ ...view.settings, providers });
  };

  const currentSystemApplication = playback.snapshot.player === "system"
    && playback.snapshot.sourceAppBundleId
    && playback.snapshot.isRunning
    ? { name: playback.snapshot.sourceAppName ?? playback.snapshot.sourceAppBundleId, bundleId: playback.snapshot.sourceAppBundleId }
    : null;
  const canAddCurrentApplication = Boolean(currentSystemApplication)
    && !applications.some((application) => application.bundleId === currentSystemApplication?.bundleId);

  const addCurrentApplication = async () => {
    if (!view || !currentSystemApplication) return;
    setSaving(true);
    setError(null);
    try {
      const application = await api.resolveApplicationByBundleId(currentSystemApplication.bundleId);
      await save({ ...view.settings, alwaysNetworkApplications: [...applications, application] });
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSaving(false);
    }
  };

  const chooseApplications = async () => {
    if (!view) return;
    const selected = await open({
      multiple: true,
      filters: [{ name: t("settings.artwork.applicationPicker"), extensions: ["app"] }],
    });
    if (!selected) return;
    setSaving(true);
    setError(null);
    try {
      const resolved = await api.resolveSystemMediaApplications(Array.isArray(selected) ? selected : [selected]);
      await save({ ...view.settings, alwaysNetworkApplications: [...applications, ...resolved] });
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSaving(false);
    }
  };

  const removeApplication = (bundleId: string) => {
    if (!view) return;
    void save({
      ...view.settings,
      alwaysNetworkApplications: applications.filter((application) => application.bundleId !== bundleId),
    });
  };

  const test = async (providerIds: string[]) => {
    if (testing || providerIds.length === 0) return;
    setTesting(providerIds.length === 1 ? providerIds[0] : "*");
    try {
      const statuses = await Promise.all(providerIds.map((providerId) => api.testArtworkProvider(providerId, itunesCountry)));
      setView((current) => current ? { ...current, statuses: current.statuses.map((item) => statuses.find((status) => status.providerId === item.providerId) ?? item) } : current);
    } catch (error) { setError(messageOf(error)); }
    finally { setTesting(null); }
  };

  const changeDirectory = async () => {
    const path = await open({ directory: true, multiple: false, defaultPath: cache?.directory, title: t("settings.artwork.chooseFolder") });
    if (!path) return;
    setSaving(true);
    try { setCache(await api.setArtworkCacheDirectory(path)); }
    catch (error) { setError(messageOf(error)); }
    finally { setSaving(false); }
  };

  const clearCache = async () => {
    if (!confirmingClear) {
      setConfirmingClear(true);
      setNotice(t("settings.artwork.clearConfirm"));
      clearTimer.current = setTimeout(() => setConfirmingClear(false), 4000);
      return;
    }
    if (clearTimer.current) clearTimeout(clearTimer.current);
    setConfirmingClear(false);
    setSaving(true);
    try {
      setCache(await api.clearArtworkCache());
      setNotice(t("settings.artwork.cleared"));
    } catch (error) { setError(messageOf(error)); }
    finally { setSaving(false); }
  };

  return <>
    <SettingsHeading
      title={t("settings.artwork.title")}
      description={t("settings.artwork.description")}
      onReset={() => void resetSection("artwork")}
      resetting={resettingSection === "artwork"}
      confirming={confirmingReset === "artwork"}
    />
    <SettingsCard title={t("settings.artwork.networkFallback")}>
      <ToggleRow
        label={t("settings.artwork.networkFallback")}
        description={t("settings.artwork.networkFallbackHint")}
        value={view?.settings.networkFallback ?? false}
        disabled={!view || saving}
        onChange={(networkFallback) => { if (view) return save({ ...view.settings, networkFallback }); }}
      />
      <SelectRow
        label={t("settings.artwork.itunesStorefront")}
        description={t("settings.artwork.itunesStorefrontHint")}
        value={view?.settings.itunesStorefront ?? "auto"}
        disabled={!view || saving}
        options={[
          ["auto", t("settings.artwork.itunesStorefrontAuto", { region: t(`settings.artwork.region.${autoCountry}`) })],
          ...(["CN", "TW", "HK", "US"] as const).map((country) => [country, t(`settings.artwork.region.${country}`)] as [string, string]),
        ]}
        onChange={(itunesStorefront) => { if (view) void save({ ...view.settings, itunesStorefront: itunesStorefront as ItunesStorefront }); }}
      />
      <p className={styles.cardHint}>{t("settings.artwork.onlineNotice")}</p>
    </SettingsCard>
    <SettingsCard title={t("settings.artwork.alwaysNetworkApplications")}>
      <div className={styles.systemApplicationsToolbar}>
        <p className={styles.cardHint}>{t("settings.artwork.alwaysNetworkApplicationsHint")}</p>
        <div className={styles.shortcutControls}>
          <button className={styles.shortcutReset} disabled={!view || saving || !canAddCurrentApplication} onClick={() => void addCurrentApplication()}><UiIcon name="plus" />{t("settings.artwork.addCurrentApplication")}</button>
          <button className={styles.shortcutReset} disabled={!view || saving} onClick={() => void chooseApplications()}><UiIcon name="plus" />{t("settings.artwork.chooseApplications")}</button>
        </div>
      </div>
      <ApplicationList applications={applications} icons={applicationIcons} busy={saving} emptyLabel={t("settings.artwork.alwaysNetworkApplicationsEmpty")} removeLabel={t("common.actions.remove")} onRemove={removeApplication} />
    </SettingsCard>
    <SettingsCard title={t("settings.artwork.providers")} trailing={view && <button className={styles.cardHeaderButton} disabled={testing !== null} onClick={() => void test(view.settings.providers.map((provider) => provider.id))}>{testing === "*" ? t("common.actions.testing") : t("common.actions.testAll")}</button>}>
      <p className={styles.cardHint}>{t("settings.artwork.providersHint")}</p>
      <div className={styles.providers} data-dragging={dragging != null}>
        {view?.settings.providers.map((provider, index) => {
          const status = view.statuses.find((item) => item.providerId === provider.id);
          return <div
            className={styles.provider}
            data-dragging={dragging === index}
            draggable={!saving}
            key={provider.id}
            onDragStart={() => setDragging(index)}
            onDragOver={(event) => event.preventDefault()}
            onDrop={() => moveProvider(index)}
            onDragEnd={() => setDragging(null)}
          >
            <button type="button" className={styles.dragHandle} aria-label={t("settings.artwork.dragProvider", { provider: status?.name ?? provider.id })}><UiIcon name="drag" /></button>
            <b>#{index + 1}</b>
            <div><strong>{status?.name ?? provider.id}</strong><small data-health={status?.available ? "available" : status?.message ? "unavailable" : "unknown"}>{status?.available ? t("settings.artwork.available") : status?.message ?? t("settings.artwork.untested")}</small></div>
            <button aria-label={status?.name ?? provider.id} aria-pressed={provider.enabled} className={styles.switch} data-on={provider.enabled} disabled={saving} onClick={() => void save({ ...view.settings, providers: view.settings.providers.map((item) => item.id === provider.id ? { ...item, enabled: !item.enabled } : item) })}><span /></button>
            <button disabled={testing !== null} onClick={() => void test([provider.id])}>{testing === provider.id || testing === "*" ? t("common.actions.testing") : t("common.actions.test")}</button>
          </div>;
        })}
      </div>
    </SettingsCard>
    <SettingsCard title={t("settings.artwork.cache") }>
      <p className={styles.cardHint}>{cache?.directory ?? "—"}</p>
      <p className={styles.cardHint}>{cache ? t("settings.artwork.cacheUsage", { count: cache.fileCount, size: formatBytes(cache.totalBytes) }) : t("settings.artwork.loading")}</p>
      {cache?.warning && <p className={styles.cardHint} data-error="true">{cache.warning}</p>}
      <div className={styles.buttonRow}>
        <button disabled={!cache || saving} onClick={() => void api.openArtworkCacheDirectory().catch((error) => setError(messageOf(error)))}>{t("settings.artwork.openFolder")}</button>
        <button disabled={saving} onClick={() => void changeDirectory()}>{t("settings.artwork.changeFolder")}</button>
        <button className={styles.danger} data-confirming={confirmingClear} disabled={saving} onClick={() => void clearCache()}>{confirmingClear ? t("common.actions.confirmAgain") : t("settings.artwork.clear")}</button>
      </div>
    </SettingsCard>
  </>;
}
