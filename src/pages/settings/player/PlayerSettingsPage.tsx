import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { Plus } from "lucide-react";

import type { PlayerFollowerServiceState, PlayerSelection, SystemMediaFilterMode } from "../../../shared/types";
import { api, messageOf } from "../../../shared/api";
import { playbackStatusText } from "../../../features/i18n/userText";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

import { useSettingsContext } from "../shared/SettingsContext";
import styles from "../settings.module.scss";
import { PageHeader, SelectRow, SettingsPage, SettingsSection } from "../shared/components";
import { ApplicationList } from "./ApplicationList";

const playerOptions: PlayerSelection[] = ["auto", "apple_music", "spotify", "system"];

export default function PlayerSettingsPage() {
  const {
    config,
    setSystemMediaFilterMode,
    setSystemMediaApplications,
    setPlayerFollowerApplication,
    playback,
    resettingSection,
    confirmingReset,
    setError,
    setNotice,
    resetSection,
  } = useSettingsContext();
  const { t } = useTranslation();
  const [savingApplications, setSavingApplications] = useState(false);
  const [savingFollower, setSavingFollower] = useState(false);
  const [followerStatus, setFollowerStatus] = useState<PlayerFollowerServiceState | null>(null);
  const [applicationIcons, setApplicationIcons] = useState<Record<string, string>>({});
  const [applicationNames, setApplicationNames] = useState<Record<string, string>>({});

  useEffect(() => {
    void api.getPlayerFollowerServiceStatus().then(setFollowerStatus).catch(() => setFollowerStatus("not_found"));
    let disposed = false;
    const bundleIds = [...new Set([
      ...config.app.systemMediaApplications.map((application) => application.bundleId),
      ...(config.app.playerFollowerApplication ? [config.app.playerFollowerApplication.bundleId] : []),
    ])];
    if (bundleIds.length === 0) {
      setApplicationIcons({});
      setApplicationNames({});
      return () => { disposed = true; };
    }
    void api.getApplicationIcons(bundleIds).then((icons) => {
      if (!disposed) setApplicationIcons(icons);
    }).catch(() => {
      if (!disposed) setApplicationIcons({});
    });
    void Promise.all(bundleIds.map(async (bundleId) => {
      try {
        const application = await api.resolveApplicationByBundleId(bundleId);
        return [bundleId, application.name] as const;
      } catch {
        return null;
      }
    })).then((applications) => {
      if (disposed) return;
      setApplicationNames(Object.fromEntries(
        applications.filter((application): application is readonly [string, string] => application !== null),
      ));
    });
    return () => { disposed = true; };
  }, [config.app.playerFollowerApplication?.bundleId, config.app.systemMediaApplications]);

  const saveApplications = async (applications: typeof config.app.systemMediaApplications) => {
    setSavingApplications(true);
    setError(null);
    try {
      await setSystemMediaApplications(applications);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSavingApplications(false);
    }
  };

  const currentSystemApplication = playback.snapshot.player === "system"
    && playback.snapshot.sourceAppBundleId
    && (playback.snapshot.isRunning || playback.snapshot.errorCode === "source_not_allowed")
    ? {
        name: playback.snapshot.sourceAppName ?? playback.snapshot.sourceAppBundleId,
        bundleId: playback.snapshot.sourceAppBundleId,
      }
    : null;
  const canAddCurrentSystemApplication = Boolean(currentSystemApplication)
    && !config.app.systemMediaApplications.some(
      (application) => application.bundleId === currentSystemApplication?.bundleId,
    );

  const addCurrentSystemApplication = async () => {
    if (!currentSystemApplication) return;
    setSavingApplications(true);
    setError(null);
    try {
      const resolved = await api.resolveApplicationByBundleId(currentSystemApplication.bundleId);
      await setSystemMediaApplications([...config.app.systemMediaApplications, resolved]);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSavingApplications(false);
    }
  };

  const chooseSystemApplications = async () => {
    const selected = await open({
      multiple: true,
      filters: [{ name: t("settings.app.systemApplicationsPicker"), extensions: ["app"] }],
    });
    if (!selected) return;
    setSavingApplications(true);
    setError(null);
    try {
      const resolved = await api.resolveSystemMediaApplications(
        Array.isArray(selected) ? selected : [selected],
      );
      await setSystemMediaApplications([...config.app.systemMediaApplications, ...resolved]);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSavingApplications(false);
    }
  };

  const refreshFollowerStatus = async () => {
    await api.getPlayerFollowerServiceStatus().then(setFollowerStatus).catch(() => setFollowerStatus("not_found"));
  };

  const chooseFollower = async () => {
    const selected = await open({ multiple: false, filters: [{ name: t("settings.app.playerFollowerPicker"), extensions: ["app"] }] });
    if (!selected) return;
    setSavingFollower(true);
    setError(null);
    setNotice(null);
    try {
      await setPlayerFollowerApplication(await api.resolvePlayerFollowerApplication(selected));
    } catch (error) {
      setError(messageOf(error));
    } finally {
      await refreshFollowerStatus();
      setSavingFollower(false);
    }
  };

  const clearFollower = async () => {
    setSavingFollower(true);
    setNotice(null);
    try {
      await setPlayerFollowerApplication(null);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      await refreshFollowerStatus();
      setSavingFollower(false);
    }
  };

  const reregisterFollower = async () => {
    if (!config.app.playerFollowerApplication) return;
    setSavingFollower(true);
    setError(null);
    setNotice(null);
    try {
      await api.reregisterPlayerFollowerService();
      setNotice(t("settings.app.playerFollowerReregistered"));
    } catch (error) {
      setError(messageOf(error));
    } finally {
      await refreshFollowerStatus();
      setSavingFollower(false);
    }
  };

  const followerUnavailable = followerStatus === null || followerStatus === "development" || followerStatus === "unsupported";
  const canReregisterFollower = Boolean(config.app.playerFollowerApplication) && !followerUnavailable;
  const systemMediaAllowlist = config.app.systemMediaFilterMode === "allowlist";
  const playbackStatus = playbackStatusText(playback.snapshot, t);
  const playbackNeutral = playback.snapshot.errorCode === "waiting" || playback.snapshot.errorCode === "no_unique_player";
  const playbackHasActions = playback.snapshot.errorCode === "automation_denied"
    || playback.snapshot.errorCode === "multiple_playing"
    || ["not_installed", "response_timeout", "invalid_response", "unavailable"].includes(playback.snapshot.errorCode ?? "");

  return <SettingsPage sections={[
    { id: "player-mode", label: t("settings.app.player") },
    { id: "player-system-applications", label: t("settings.app.systemApplications") },
    { id: "player-follower", label: t("settings.app.playerFollower") },
  ]}>
    <PageHeader
      title={t("settings.player.title")}
      description={t("settings.player.description")}
      onReset={() => void resetSection("player")}
      resetting={resettingSection === "player"}
      confirming={confirmingReset === "player"}
    />
    {(playbackStatus || playback.configError || playback.snapshotLoadError) && <Alert variant={playbackNeutral && !playback.configError && !playback.snapshotLoadError ? "default" : "warning"} className="mb-3 flex flex-wrap items-center justify-between gap-x-4 gap-y-2 p-3">
      <div className="min-w-[min(16rem,100%)] flex-1">
        <AlertTitle className="mb-0">{playbackNeutral ? t("settings.player.idleStatus") : t("settings.player.attentionStatus")}</AlertTitle>
        <AlertDescription className="mt-0.5">
          <span>{playback.configError ?? playback.snapshotLoadError ?? playbackStatus}</span>
        </AlertDescription>
      </div>
      {playbackHasActions && <div className="flex flex-none flex-wrap gap-2">
        {playback.snapshot.errorCode === "automation_denied" && <Button size="sm" variant="outline" onClick={() => void api.openAutomationSystemSettings().catch((error) => setError(messageOf(error)))}>{t("settings.player.openAutomationSettings")}</Button>}
        {playback.snapshot.errorCode === "multiple_playing" && <><Button size="sm" variant="outline" onClick={() => void playback.setSelection("apple_music").catch((error) => setError(messageOf(error)))}>Apple Music</Button><Button size="sm" variant="outline" onClick={() => void playback.setSelection("spotify").catch((error) => setError(messageOf(error)))}>Spotify</Button></>}
        {["not_installed", "response_timeout", "invalid_response", "unavailable"].includes(playback.snapshot.errorCode ?? "") && <><Button size="sm" variant="outline" onClick={() => void playback.refreshSnapshot()}>{t("settings.player.detectAgain")}</Button><Button size="sm" variant="ghost" onClick={() => void playback.setSelection("auto").catch((error) => setError(messageOf(error)))}>{t("settings.player.useAuto")}</Button></>}
      </div>}
    </Alert>}
    <SettingsSection id="player-mode" title={t("settings.app.player")}>
      <SelectRow label={t("settings.app.playerMode")} description={t("settings.app.playerHint")} value={playback.selection} options={playerOptions.map((option) => [option, option === "auto" ? t("settings.app.playerAuto") : option === "apple_music" ? "Apple Music" : option === "spotify" ? "Spotify" : t("settings.app.playerSystem")])} onChange={(selection) => void playback.setSelection(selection as PlayerSelection).catch((error) => setError(messageOf(error)))} />
    </SettingsSection>
    <SettingsSection id="player-system-applications" title={t("settings.app.systemApplications")}>
      <SelectRow
        label={t("settings.app.systemMediaFilterMode")}
        value={config.app.systemMediaFilterMode}
        options={[
          ["allowlist", t("settings.app.systemMediaAllowlist")],
          ["blocklist", t("settings.app.systemMediaBlocklist")],
        ]}
        onChange={(mode) => void setSystemMediaFilterMode(mode as SystemMediaFilterMode).catch((error) => setError(messageOf(error)))}
      />
      <div className={styles.systemApplicationsToolbar}>
        <p className={styles.cardHint}>{t(systemMediaAllowlist ? "settings.app.systemApplicationsAllowlistHint" : "settings.app.systemApplicationsBlocklistHint")}</p>
        <div className={styles.shortcutControls}>
          <Button variant="outline" size="sm" disabled={savingApplications || !canAddCurrentSystemApplication} onClick={() => void addCurrentSystemApplication()}><Plus />{t(systemMediaAllowlist ? "settings.app.addAllowedApplication" : "settings.app.addBlockedApplication")}</Button>
          <Button variant="outline" size="sm" disabled={savingApplications} onClick={() => void chooseSystemApplications()}><Plus />{t(systemMediaAllowlist ? "settings.app.chooseAllowedApplications" : "settings.app.chooseBlockedApplications")}</Button>
        </div>
      </div>
      <ApplicationList
        applications={config.app.systemMediaApplications}
        icons={applicationIcons}
        names={applicationNames}
        busy={savingApplications}
        emptyLabel={t(systemMediaAllowlist ? "settings.app.systemApplicationsAllowlistEmpty" : "settings.app.systemApplicationsBlocklistEmpty")}
        removeLabel={t("common.actions.remove")}
        onRemove={(bundleId) => void saveApplications(config.app.systemMediaApplications.filter((application) => application.bundleId !== bundleId))}
      />
    </SettingsSection>
    <SettingsSection id="player-follower" title={t("settings.app.playerFollower")}>
      {canReregisterFollower && <p className={styles.cardHint}>{t("settings.app.playerFollowerAdhocWarning")}</p>}
      <div className={styles.systemApplicationsToolbar}>
        <p className={styles.cardHint}>{t("settings.app.playerFollowerHint")}</p>
        <div className={styles.shortcutControls}>
          {followerStatus === "development" && <Badge variant="secondary">{t("settings.app.playerFollowerDevelopmentShort")}</Badge>}
          <Button variant="outline" size="sm" disabled={savingFollower || followerUnavailable} onClick={() => void chooseFollower()}><Plus />{t("settings.app.choosePlayerFollower")}</Button>
          {canReregisterFollower && <Button variant="outline" size="sm" disabled={savingFollower} onClick={() => void reregisterFollower()}>{t("settings.app.retryPlayerFollower")}</Button>}
        </div>
      </div>
      <ApplicationList applications={config.app.playerFollowerApplication ? [config.app.playerFollowerApplication] : []} icons={applicationIcons} names={applicationNames} busy={savingFollower || followerUnavailable} emptyLabel={t("settings.app.playerFollowerEmpty")} removeLabel={t("common.actions.remove")} onRemove={() => void clearFollower()} />
      {followerStatus === "unsupported" && <p className={styles.cardHint} data-error="true">{t("settings.app.playerFollowerUnsupported")}</p>}
      {(followerStatus === "not_found" || followerStatus === "not_registered") && config.app.playerFollowerApplication && <div className={styles.systemApplicationsToolbar}>
        <p className={styles.cardHint} data-error="true">{t(followerStatus === "not_found" ? "settings.app.playerFollowerNotFound" : "settings.app.playerFollowerNotRegistered")}</p>
      </div>}
      {followerStatus === "requires_approval" && <div className={styles.systemApplicationsToolbar}>
        <p className={styles.cardHint} data-error="true">{t("settings.app.playerFollowerApproval")}</p>
        <Button variant="outline" size="sm" disabled={savingFollower} onClick={() => void api.openPlayerFollowerSystemSettings().catch((error) => setError(messageOf(error)))}>{t("settings.app.openLoginItems")}</Button>
      </div>}
    </SettingsSection>
  </SettingsPage>;
}
