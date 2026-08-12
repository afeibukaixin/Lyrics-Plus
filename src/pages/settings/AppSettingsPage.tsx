import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { UiIcon } from "../../components/UiIcon";
import { defaultGlobalShortcuts, type GlobalShortcutSettings, type GlobalShortcutStatus, type LanguagePreference, type PlayerFollowerServiceState, type PlayerSelection, type RegisteredApplication, type SystemMediaFilterMode } from "../../shared/types";
import { api, messageOf } from "../../shared/api";
import { languageRegistry, supportedLanguages } from "../../shared/languages";
import { localizedSource, playbackStatusText } from "../../features/i18n/userText";
import { normalizeLanguagePreference } from "../../features/i18n/i18n";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { RangeRow, SelectRow, SettingsCard, SettingsHeading, ToggleRow } from "./components";

const playerOptions: PlayerSelection[] = ["auto", "apple_music", "spotify", "system"];
const languageOptions = supportedLanguages.map((code) => ({ code, label: languageRegistry[code].nativeLabel }));

type ShortcutAction = keyof GlobalShortcutSettings;

const shortcutActions: ShortcutAction[] = ["toggleOverlay", "unlockOverlay", "resetOverlay"];
const applicationIconCache: Record<string, string> = {};

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
  return value.split("+").map((token) => {
    const normalized = token.toLowerCase();
    if (labels[normalized]) return labels[normalized];
    return token.replace(/^Key/, "").replace(/^Digit/, "");
  }).join(" ");
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

function ApplicationList({ applications, icons, busy, emptyLabel, removeLabel, onRemove }: {
  applications: RegisteredApplication[];
  icons: Record<string, string>;
  busy: boolean;
  emptyLabel: string;
  removeLabel: string;
  onRemove: (bundleId: string) => void;
}) {
  if (applications.length === 0) return <p className={styles.applicationEmpty}>{emptyLabel}</p>;
  return (
    <div className={styles.applicationList}>
      {applications.map((application) => {
        return <div className={styles.applicationItem} key={application.bundleId} title={application.bundleId}>
          {icons[application.bundleId]
            ? <img alt="" src={icons[application.bundleId]} />
            : <span className={styles.applicationIconFallback}><UiIcon name="musicNote" /></span>}
          <strong>{application.name}</strong>
          <button
            aria-label={`${removeLabel} ${application.name}`}
            className={styles.applicationRemove}
            disabled={busy}
            title={removeLabel}
            onClick={() => onRemove(application.bundleId)}
          ><UiIcon name="close" /></button>
        </div>;
      })}
    </div>
  );
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
    lyrics,
    resettingSection,
    confirmingReset,
    setError,
    resetSection,
  } = useSettingsContext();
  const [recording, setRecording] = useState<ShortcutAction | null>(null);
  const [savingShortcut, setSavingShortcut] = useState(false);
  const [shortcutStatus, setShortcutStatus] = useState<GlobalShortcutStatus | null>(null);
  const [savingApplications, setSavingApplications] = useState(false);
  const [savingPlayerFollower, setSavingPlayerFollower] = useState(false);
  const [playerFollowerService, setPlayerFollowerService] = useState<PlayerFollowerServiceState | null>(null);
  const [applicationIcons, setApplicationIcons] = useState<Record<string, string>>(() => ({ ...applicationIconCache }));
  const { t } = useTranslation();

  const iconBundleIds = [...new Set([
    ...config.app.systemMediaApplications.map((application) => application.bundleId),
    ...(config.app.playerFollowerApplication ? [config.app.playerFollowerApplication.bundleId] : []),
  ])];
  const iconKey = iconBundleIds.join("\n");

  useEffect(() => {
    void api.getGlobalShortcutStatus().then(setShortcutStatus).catch(() => setShortcutStatus(null));
  }, []);

  useEffect(() => {
    void api.getPlayerFollowerServiceStatus().then(setPlayerFollowerService).catch(() => setPlayerFollowerService("not_found"));
  }, [config.app.playerFollowerApplication?.bundleId]);

  useEffect(() => {
    const cached = Object.fromEntries(iconBundleIds.flatMap((bundleId) => (
      applicationIconCache[bundleId] ? [[bundleId, applicationIconCache[bundleId]]] : []
    )));
    setApplicationIcons(cached);
    let cancelled = false;
    for (const bundleId of iconBundleIds) {
      if (applicationIconCache[bundleId]) continue;
      void api.getApplicationIcons([bundleId]).then((icons) => {
        Object.assign(applicationIconCache, icons);
        if (!cancelled) setApplicationIcons((current) => ({ ...current, ...icons }));
      }).catch(() => undefined);
    }
    return () => { cancelled = true; };
  }, [iconKey]);

  const saveShortcut = async (action: ShortcutAction, value: string) => {
    setSavingShortcut(true);
    setError(null);
    try {
      await setGlobalShortcuts({ ...config.app.shortcuts, [action]: value });
      void api.getGlobalShortcutStatus().then(setShortcutStatus).catch(() => setShortcutStatus(null));
      setRecording(null);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSavingShortcut(false);
    }
  };

  const diagnostics = playback.commandError
    ?? playbackStatusText(playback.snapshot, t)
    ?? lyrics.error
    ?? (lyrics.document
      ? t("settings.app.diagnosticsLyrics", {
          source: localizedSource(lyrics.document.metadata.source, t),
          count: lyrics.document.tracks.original.lines.length,
        })
      : t("settings.app.diagnosticsEmpty"));
  const shortcutLabel = (action: ShortcutAction) => t(`settings.app.${action}`);
  const unavailableShortcuts = shortcutStatus
    ? shortcutActions.filter((action) => !shortcutStatus[action])
    : [];
  const languagePreference = normalizeLanguagePreference(config.app.language);
  const systemMediaAllowlist = config.app.systemMediaFilterMode === "allowlist";
  const currentSystemApplication = playback.snapshot.player === "system"
    && playback.snapshot.sourceAppBundleId
    && (playback.snapshot.isRunning || playback.snapshot.errorCode === "source_not_allowed")
    ? { name: playback.snapshot.sourceAppName ?? playback.snapshot.sourceAppBundleId, bundleId: playback.snapshot.sourceAppBundleId }
    : null;
  const canAddCurrentSystem = Boolean(currentSystemApplication)
    && !config.app.systemMediaApplications.some((application) => application.bundleId === currentSystemApplication?.bundleId);
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
      const resolved = await api.resolveSystemMediaApplications(Array.isArray(selected) ? selected : [selected]);
      await setSystemMediaApplications([...config.app.systemMediaApplications, ...resolved]);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSavingApplications(false);
    }
  };
  const choosePlayerFollower = async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: t("settings.app.playerFollowerPicker"), extensions: ["app"] }],
    });
    if (!selected) return;
    setSavingPlayerFollower(true);
    setError(null);
    try {
      const resolved = await api.resolvePlayerFollowerApplication(selected);
      await setPlayerFollowerApplication(resolved);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      await api.getPlayerFollowerServiceStatus().then(setPlayerFollowerService).catch(() => setPlayerFollowerService("not_found"));
      setSavingPlayerFollower(false);
    }
  };
  const clearPlayerFollower = async () => {
    setSavingPlayerFollower(true);
    setError(null);
    try {
      await setPlayerFollowerApplication(null);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      await api.getPlayerFollowerServiceStatus().then(setPlayerFollowerService).catch(() => setPlayerFollowerService("not_found"));
      setSavingPlayerFollower(false);
    }
  };
  const retryPlayerFollower = async () => {
    if (!config.app.playerFollowerApplication) return;
    setSavingPlayerFollower(true);
    setError(null);
    try {
      await setPlayerFollowerApplication(config.app.playerFollowerApplication);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      await api.getPlayerFollowerServiceStatus().then(setPlayerFollowerService).catch(() => setPlayerFollowerService("not_found"));
      setSavingPlayerFollower(false);
    }
  };
  const followerUnavailable = playerFollowerService === null
    || playerFollowerService === "development"
    || playerFollowerService === "unsupported";
  return (
    <>
      <SettingsHeading title={t("settings.app.title")} description={t("settings.app.description")} onReset={() => void resetSection("app")} resetting={resettingSection === "app"} confirming={confirmingReset === "app"} />
      <SettingsCard title={t("settings.app.player")}>
        <SelectRow
          label={t("settings.app.playerMode")}
          description={t("settings.app.playerHint")}
          value={playback.selection}
          options={playerOptions.map((option) => [option, option === "auto" ? t("settings.app.playerAuto") : option === "apple_music" ? "Apple Music" : option === "spotify" ? "Spotify" : t("settings.app.playerSystem")])}
          onChange={(selection) => playback.setSelection(selection as PlayerSelection)}
        />
      </SettingsCard>
      <SettingsCard title={t("settings.app.playerFollower")}>
        <div className={styles.systemApplicationsToolbar}>
          <p className={styles.cardHint}>{t("settings.app.playerFollowerHint")}</p>
          <button className={styles.shortcutReset} disabled={savingPlayerFollower || followerUnavailable} onClick={() => void choosePlayerFollower()}><UiIcon name="plus" />{t("settings.app.choosePlayerFollower")}</button>
        </div>
        <ApplicationList applications={config.app.playerFollowerApplication ? [config.app.playerFollowerApplication] : []} icons={applicationIcons} busy={savingPlayerFollower || followerUnavailable} emptyLabel={t("settings.app.playerFollowerEmpty")} removeLabel={t("common.actions.remove")} onRemove={() => void clearPlayerFollower()} />
        {playerFollowerService === "development" && <p className={styles.cardHint}>{t("settings.app.playerFollowerDevelopment")}</p>}
        {playerFollowerService === "unsupported" && <p className={styles.cardHint} data-error="true">{t("settings.app.playerFollowerUnsupported")}</p>}
        {(playerFollowerService === "not_found" || playerFollowerService === "not_registered") && config.app.playerFollowerApplication && (
          <div className={styles.systemApplicationsToolbar}>
            <p className={styles.cardHint} data-error="true">{t(playerFollowerService === "not_found" ? "settings.app.playerFollowerNotFound" : "settings.app.playerFollowerNotRegistered")}</p>
            <button className={styles.shortcutReset} disabled={savingPlayerFollower} onClick={() => void retryPlayerFollower()}>{t("settings.app.retryPlayerFollower")}</button>
          </div>
        )}
        {playerFollowerService === "requires_approval" && (
          <div className={styles.systemApplicationsToolbar}>
            <p className={styles.cardHint} data-error="true">{t("settings.app.playerFollowerApproval")}</p>
            <button className={styles.shortcutReset} onClick={() => void api.openPlayerFollowerSystemSettings().catch((error) => setError(messageOf(error)))}>{t("settings.app.openLoginItems")}</button>
          </div>
        )}
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
            <button className={styles.shortcutReset} disabled={savingApplications || !canAddCurrentSystem} onClick={() => void addCurrentSystemApplication()}><UiIcon name="plus" />{t(systemMediaAllowlist ? "settings.app.addAllowedApplication" : "settings.app.addBlockedApplication")}</button>
            <button className={styles.shortcutReset} disabled={savingApplications} onClick={() => void chooseSystemApplications()}><UiIcon name="plus" />{t(systemMediaAllowlist ? "settings.app.chooseAllowedApplications" : "settings.app.chooseBlockedApplications")}</button>
          </div>
        </div>
        <ApplicationList applications={config.app.systemMediaApplications} icons={applicationIcons} busy={savingApplications} emptyLabel={t(systemMediaAllowlist ? "settings.app.systemApplicationsAllowlistEmpty" : "settings.app.systemApplicationsBlocklistEmpty")} removeLabel={t("common.actions.remove")} onRemove={(bundleId) => void saveApplications(config.app.systemMediaApplications.filter((application) => application.bundleId !== bundleId))} />
      </SettingsCard>
      <SettingsCard title={t("settings.app.display")}>
        <SelectRow
          label={t("settings.app.language.label")}
          description={t("settings.app.language.description")}
          value={languagePreference}
          options={[
            ["system", t("common.language.system")],
            ...languageOptions.map(({ code, label }) => [code, label] as [string, string]),
          ]}
          onChange={(language) => void setLanguage(language as LanguagePreference).catch((value) => setError(messageOf(value)))}
        />
        <RangeRow label={t("settings.app.fontScale")} value={config.app.uiFontScale} min={80} max={150} step={10} suffix="%" onChange={(scale) => void setUiFontScale(scale).catch((value) => setError(messageOf(value)))} />
        <p className={styles.cardHint}>{t("settings.app.fontScaleHint")}</p>
      </SettingsCard>
      <SettingsCard title={t("settings.app.dockMenu")}>
        <ToggleRow label={t("settings.app.hideDock")} description={t("settings.app.hideDockHint")} value={config.app.hideDockIcon} onChange={(hidden) => setDockIconHidden(hidden).catch((value) => setError(messageOf(value)))} />
        <ToggleRow label={t("settings.app.silentStartup")} description={t("settings.app.silentStartupHint")} value={config.app.silentStartup} onChange={(enabled) => setSilentStartup(enabled).catch((value) => setError(messageOf(value)))} />
      </SettingsCard>
      <SettingsCard title={t("settings.app.shortcuts")}>
        <div className={styles.shortcutRow}><span>{t("settings.app.openSettings")}</span><kbd>⌘ ,</kbd></div>
        {shortcutActions.map((action) => {
          const isRecording = recording === action;
          const isDefault = config.app.shortcuts[action] === defaultGlobalShortcuts[action];
          return (
            <div className={styles.shortcutRow} key={action}>
              <span>{shortcutLabel(action)}</span>
              <div className={styles.shortcutControls}>
                <button
                  autoFocus={isRecording}
                  className={styles.shortcutRecorder}
                  data-recording={isRecording}
                  disabled={savingShortcut}
                  key={isRecording ? "recording" : "idle"}
                  onClick={() => setRecording(isRecording ? null : action)}
                  onKeyDown={(event) => {
                    if (!isRecording) return;
                    event.preventDefault();
                    event.stopPropagation();
                    if (event.key === "Escape") {
                      setRecording(null);
                      return;
                    }
                    const shortcut = shortcutFromEvent(event);
                    if (shortcut) void saveShortcut(action, shortcut);
                  }}
                >{isRecording ? t("settings.app.record") : shortcutDisplay(config.app.shortcuts[action])}</button>
                <button className={styles.shortcutReset} disabled={savingShortcut || isDefault} onClick={() => void saveShortcut(action, defaultGlobalShortcuts[action])}>{t("common.actions.resetDefault")}</button>
              </div>
            </div>
          );
        })}
        {unavailableShortcuts.length > 0 && (
          <p className={styles.cardHint} data-error="true">
            {t("settings.app.shortcutUnavailable", {
              actions: unavailableShortcuts.map(shortcutLabel).join(", "),
            })}
          </p>
        )}
      </SettingsCard>
      <SettingsCard title={t("settings.app.diagnostics")}><div className={styles.diagnostics} data-error={Boolean(playback.commandError || playback.snapshot.errorCode || lyrics.error)}><i /><span>{diagnostics}</span></div></SettingsCard>
    </>
  );
}
