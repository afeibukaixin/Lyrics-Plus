import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { Plus } from "lucide-react";
import { defaultGlobalShortcuts, type GlobalShortcutSettings, type GlobalShortcutStatus, type LanguagePreference, type PlayerFollowerServiceState, type PlayerSelection, type SystemMediaFilterMode } from "../../shared/types";
import { api, messageOf } from "../../shared/api";
import { languageRegistry, supportedLanguages } from "../../shared/languages";
import { normalizeLanguagePreference } from "../../features/i18n/i18n";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { ApplicationList, RangeRow, SelectRow, SettingsCard, SettingsHeading, ToggleRow } from "./components";

const playerOptions: PlayerSelection[] = ["auto", "apple_music", "spotify", "system"];
const languageOptions = supportedLanguages.map((code) => ({ code, label: languageRegistry[code].nativeLabel }));
type ShortcutAction = keyof GlobalShortcutSettings;
const shortcutActions: ShortcutAction[] = ["toggleOverlay", "unlockOverlay", "resetOverlay"];

function shortcutDisplay(value: string) {
  const mac = navigator.userAgent.includes("Mac");
  const labels: Record<string, string> = {
    commandorcontrol: mac ? "⌘" : "Ctrl",
    commandorctrl: mac ? "⌘" : "Ctrl",
    super: mac ? "⌘" : "Super",
    control: "Ctrl",
    ctrl: "Ctrl",
    shift: "⇧",
    alt: mac ? "⌥" : "Alt",
    option: "⌥",
  };
  return value.split("+").map((token) => labels[token.toLowerCase()] ?? token.replace(/^Key/, "").replace(/^Digit/, "")).join(" ");
}

function shortcutFromEvent(event: React.KeyboardEvent<HTMLButtonElement>) {
  if (["Meta", "Control", "Alt", "Shift"].includes(event.key)) return null;
  const modifiers = [
    event.metaKey ? "Super" : null,
    event.ctrlKey ? "Control" : null,
    event.altKey ? "Alt" : null,
    event.shiftKey ? "Shift" : null,
  ].filter((value): value is string => Boolean(value));
  if (modifiers.length === 0 || !event.code || event.code === "Unidentified") return null;
  return [...modifiers, event.code].join("+");
}

export default function AppSettingsPage() {
  const {
    config,
    setUiFontScale,
    setLanguage,
    setGlobalShortcuts,
    setSystemMediaFilterMode,
    setSystemMediaApplications,
    setPlayerFollowerApplication,
    setDockIconHidden,
    setSilentStartup,
    playback,
    resettingSection,
    confirmingReset,
    setError,
    resetSection,
  } = useSettingsContext();
  const { t } = useTranslation();
  const [recording, setRecording] = useState<ShortcutAction | null>(null);
  const [savingShortcut, setSavingShortcut] = useState(false);
  const [shortcutStatus, setShortcutStatus] = useState<GlobalShortcutStatus | null>(null);
  const [savingApplications, setSavingApplications] = useState(false);
  const [savingFollower, setSavingFollower] = useState(false);
  const [followerStatus, setFollowerStatus] = useState<PlayerFollowerServiceState | null>(null);
  const [applicationIcons, setApplicationIcons] = useState<Record<string, string>>({});

  useEffect(() => {
    void api.getGlobalShortcutStatus().then(setShortcutStatus).catch(() => setShortcutStatus(null));
  }, []);

  useEffect(() => {
    void api.getPlayerFollowerServiceStatus().then(setFollowerStatus).catch(() => setFollowerStatus("not_found"));
    const bundleIds = [...new Set([
      ...config.app.systemMediaApplications.map((application) => application.bundleId),
      ...(config.app.playerFollowerApplication ? [config.app.playerFollowerApplication.bundleId] : []),
    ])];
    if (bundleIds.length === 0) {
      setApplicationIcons({});
      return;
    }
    void api.getApplicationIcons(bundleIds).then(setApplicationIcons).catch(() => setApplicationIcons({}));
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
    try {
      await setPlayerFollowerApplication(null);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      await refreshFollowerStatus();
      setSavingFollower(false);
    }
  };

  const retryFollower = async () => {
    if (!config.app.playerFollowerApplication) return;
    setSavingFollower(true);
    try {
      await setPlayerFollowerApplication(config.app.playerFollowerApplication);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      await refreshFollowerStatus();
      setSavingFollower(false);
    }
  };

  const saveShortcut = async (action: ShortcutAction, value: string) => {
    setSavingShortcut(true);
    setError(null);
    try {
      await setGlobalShortcuts({ ...config.app.shortcuts, [action]: value });
      await api.getGlobalShortcutStatus().then(setShortcutStatus).catch(() => setShortcutStatus(null));
      setRecording(null);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSavingShortcut(false);
    }
  };

  const unavailableShortcuts = shortcutStatus ? shortcutActions.filter((action) => !shortcutStatus[action]) : [];
  const followerUnavailable = followerStatus === null || followerStatus === "development" || followerStatus === "unsupported";
  const systemMediaAllowlist = config.app.systemMediaFilterMode === "allowlist";

  return <>
    <SettingsHeading title={t("settings.player.title")} description={t("settings.player.description")} onReset={() => void resetSection("player")} resetting={resettingSection === "player"} confirming={confirmingReset === "player"} />
    <SettingsCard title={t("settings.app.player")}>
      <SelectRow label={t("settings.app.playerMode")} description={t("settings.app.playerHint")} value={playback.selection} options={playerOptions.map((option) => [option, option === "auto" ? t("settings.app.playerAuto") : option === "apple_music" ? "Apple Music" : option === "spotify" ? "Spotify" : t("settings.app.playerSystem")])} onChange={(selection) => playback.setSelection(selection as PlayerSelection)} />
    </SettingsCard>
    <SettingsCard title={t("settings.app.systemApplications")}>
      <SelectRow
        label={t("settings.app.systemMediaFilterMode")}
        description={t("settings.app.systemMediaFilterModeHint")}
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
          <button className={styles.shortcutReset} disabled={savingApplications || !canAddCurrentSystemApplication} onClick={() => void addCurrentSystemApplication()}><Plus />{t(systemMediaAllowlist ? "settings.app.addAllowedApplication" : "settings.app.addBlockedApplication")}</button>
          <button className={styles.shortcutReset} disabled={savingApplications} onClick={() => void chooseSystemApplications()}><Plus />{t(systemMediaAllowlist ? "settings.app.chooseAllowedApplications" : "settings.app.chooseBlockedApplications")}</button>
        </div>
      </div>
      <ApplicationList
        applications={config.app.systemMediaApplications}
        icons={applicationIcons}
        busy={savingApplications}
        emptyLabel={t(systemMediaAllowlist ? "settings.app.systemApplicationsAllowlistEmpty" : "settings.app.systemApplicationsBlocklistEmpty")}
        removeLabel={t("common.actions.remove")}
        onRemove={(bundleId) => void saveApplications(config.app.systemMediaApplications.filter((application) => application.bundleId !== bundleId))}
      />
    </SettingsCard>
    <SettingsCard title={t("settings.app.playerFollower")}>
      <div className={styles.systemApplicationsToolbar}>
        <p className={styles.cardHint}>{t("settings.app.playerFollowerHint")}</p>
        <button className={styles.shortcutReset} disabled={savingFollower || followerUnavailable} onClick={() => void chooseFollower()}><Plus />{t("settings.app.choosePlayerFollower")}</button>
      </div>
      <ApplicationList applications={config.app.playerFollowerApplication ? [config.app.playerFollowerApplication] : []} icons={applicationIcons} busy={savingFollower || followerUnavailable} emptyLabel={t("settings.app.playerFollowerEmpty")} removeLabel={t("common.actions.remove")} onRemove={() => void clearFollower()} />
      {followerStatus === "development" && <p className={styles.cardHint}>{t("settings.app.playerFollowerDevelopment")}</p>}
      {followerStatus === "unsupported" && <p className={styles.cardHint} data-error="true">{t("settings.app.playerFollowerUnsupported")}</p>}
      {(followerStatus === "not_found" || followerStatus === "not_registered") && config.app.playerFollowerApplication && <div className={styles.systemApplicationsToolbar}>
        <p className={styles.cardHint} data-error="true">{t(followerStatus === "not_found" ? "settings.app.playerFollowerNotFound" : "settings.app.playerFollowerNotRegistered")}</p>
        <button className={styles.shortcutReset} disabled={savingFollower} onClick={() => void retryFollower()}>{t("settings.app.retryPlayerFollower")}</button>
      </div>}
      {followerStatus === "requires_approval" && <div className={styles.systemApplicationsToolbar}>
        <p className={styles.cardHint} data-error="true">{t("settings.app.playerFollowerApproval")}</p>
        <button className={styles.shortcutReset} onClick={() => void api.openPlayerFollowerSystemSettings().catch((error) => setError(messageOf(error)))}>{t("settings.app.openLoginItems")}</button>
      </div>}
    </SettingsCard>
    <SettingsCard title={t("settings.player.startup")}>
      <ToggleRow label={t("settings.app.silentStartup")} description={t("settings.player.silentStartupHint")} value={config.app.silentStartup} onChange={(enabled) => setSilentStartup(enabled).catch((error) => setError(messageOf(error)))} />
      <ToggleRow label={t("settings.app.hideDock")} description={t("settings.app.hideDockHint")} value={config.app.hideDockIcon} onChange={(hidden) => setDockIconHidden(hidden).catch((error) => setError(messageOf(error)))} />
    </SettingsCard>
    <SettingsCard title={t("settings.app.display")}>
      <SelectRow label={t("settings.app.language.label")} description={t("settings.app.language.description")} value={normalizeLanguagePreference(config.app.language)} options={[["system", t("common.language.system")], ...languageOptions.map(({ code, label }) => [code, label] as [string, string])]} onChange={(language) => void setLanguage(language as LanguagePreference).catch((error) => setError(messageOf(error)))} />
      <RangeRow label={t("settings.app.fontScale")} value={config.app.uiFontScale} min={80} max={150} step={10} suffix="%" onChange={(scale) => void setUiFontScale(scale).catch((error) => setError(messageOf(error)))} />
    </SettingsCard>
    <SettingsCard title={t("settings.app.shortcuts")}>
      <div className={styles.shortcutRow}><span>{t("settings.app.openSettings")}</span><kbd>⌘ ,</kbd></div>
      {shortcutActions.map((action) => {
        const active = recording === action;
        const isDefault = config.app.shortcuts[action] === defaultGlobalShortcuts[action];
        return <div className={styles.shortcutRow} key={action}><span>{t(`settings.app.${action}`)}</span><div className={styles.shortcutControls}>
          <button autoFocus={active} className={styles.shortcutRecorder} data-recording={active} disabled={savingShortcut} onClick={() => setRecording(active ? null : action)} onKeyDown={(event) => {
            if (!active) return;
            event.preventDefault();
            if (event.key === "Escape") return setRecording(null);
            const shortcut = shortcutFromEvent(event);
            if (shortcut) void saveShortcut(action, shortcut);
          }}>{active ? t("settings.app.record") : shortcutDisplay(config.app.shortcuts[action])}</button>
          <button className={styles.shortcutReset} disabled={savingShortcut || isDefault} onClick={() => void saveShortcut(action, defaultGlobalShortcuts[action])}>{t("common.actions.resetDefault")}</button>
        </div></div>;
      })}
      {unavailableShortcuts.length > 0 && <p className={styles.cardHint} data-error="true">{t("settings.app.shortcutUnavailable", { actions: unavailableShortcuts.map((action) => t(`settings.app.${action}`)).join(", ") })}</p>}
    </SettingsCard>
  </>;
}
