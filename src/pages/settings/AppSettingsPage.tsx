import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { Monitor, Moon, Plus, Sun } from "lucide-react";
import { defaultGlobalShortcuts, type GlobalShortcutSettings, type GlobalShortcutStatus, type LanguagePreference, type PlayerFollowerServiceState, type PlayerSelection, type SystemMediaFilterMode, type ThemePreference } from "../../shared/types";
import { api, messageOf } from "../../shared/api";
import { languageRegistry, supportedLanguages } from "../../shared/languages";
import { normalizeLanguagePreference } from "../../features/i18n/i18n";
import { playbackStatusText } from "../../features/i18n/userText";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Field, FieldContent, FieldDescription, FieldTitle } from "@/components/ui/field";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { ApplicationList, PageHeader, SelectRow, SettingsPage, SettingsSection, ToggleRow } from "./components";

const playerOptions: PlayerSelection[] = ["auto", "apple_music", "spotify", "system"];
const languageOptions = supportedLanguages.map((code) => ({ code, label: languageRegistry[code].nativeLabel }));
type ShortcutAction = keyof GlobalShortcutSettings;
const shortcutActions: ShortcutAction[] = [
  "toggleOverlay",
  "unlockOverlay",
  "resetOverlay",
  "toggleStatusBarLyrics",
  "toggleListLyrics",
  "toggleNotchLyrics",
];

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

export default function AppSettingsPage({ scope }: { scope: "player" | "application" }) {
  const {
    config,
    setTheme,
    setLanguage,
    setGlobalShortcuts,
    setSystemMediaFilterMode,
    setSystemMediaApplications,
    setPlayerFollowerApplication,
    setDockIconHidden,
    setMenuBarIconHidden,
    setSilentStartup,
    setLyricsWindowsShowOnAllSpaces,
    playback,
    resettingSection,
    confirmingReset,
    setError,
    resetSection,
  } = useSettingsContext();
  const { t } = useTranslation();
  const [recording, setRecording] = useState<ShortcutAction | null>(null);
  const shortcutRecorderRefs = useRef<Partial<Record<ShortcutAction, HTMLButtonElement | null>>>({});
  const [savingShortcut, setSavingShortcut] = useState(false);
  const [shortcutStatus, setShortcutStatus] = useState<GlobalShortcutStatus | null>(null);
  const [savingApplications, setSavingApplications] = useState(false);
  const [savingFollower, setSavingFollower] = useState(false);
  const [followerStatus, setFollowerStatus] = useState<PlayerFollowerServiceState | null>(null);
  const [applicationIcons, setApplicationIcons] = useState<Record<string, string>>({});
  const [applicationNames, setApplicationNames] = useState<Record<string, string>>({});

  useEffect(() => {
    if (recording) shortcutRecorderRefs.current[recording]?.focus();
  }, [recording]);

  useEffect(() => {
    void api.getGlobalShortcutStatus().then(setShortcutStatus).catch(() => setShortcutStatus(null));
  }, []);

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

  const unavailableShortcuts = shortcutStatus
    ? shortcutActions.filter((action) => config.app.shortcuts[action].trim() && !shortcutStatus[action])
    : [];
  const followerUnavailable = followerStatus === null || followerStatus === "development" || followerStatus === "unsupported";
  const systemMediaAllowlist = config.app.systemMediaFilterMode === "allowlist";
  const playbackStatus = playbackStatusText(playback.snapshot, t);
  const playbackNeutral = playback.snapshot.errorCode === "waiting" || playback.snapshot.errorCode === "no_unique_player";
  const playbackHasActions = playback.snapshot.errorCode === "automation_denied"
    || playback.snapshot.errorCode === "multiple_playing"
    || ["not_installed", "response_timeout", "invalid_response", "unavailable"].includes(playback.snapshot.errorCode ?? "");
  const sections = scope === "player"
    ? [
        { id: "player-mode", label: t("settings.app.player") },
        { id: "player-system-applications", label: t("settings.app.systemApplications") },
        { id: "player-follower", label: t("settings.app.playerFollower") },
      ]
    : [
        { id: "application-startup", label: t("settings.player.startup") },
        { id: "application-display", label: t("settings.app.display") },
        { id: "application-shortcuts", label: t("settings.app.shortcuts") },
      ];

  return <SettingsPage sections={sections}>
    <PageHeader
      title={t(scope === "player" ? "settings.player.title" : "settings.app.title")}
      description={t(scope === "player" ? "settings.player.description" : "settings.app.description")}
      onReset={() => void resetSection(scope)}
      resetting={resettingSection === scope}
      confirming={confirmingReset === scope}
    />
    {scope === "player" && <>
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
      <div className={styles.systemApplicationsToolbar}>
        <p className={styles.cardHint}>{t("settings.app.playerFollowerHint")}</p>
        <div className={styles.shortcutControls}>
          {followerStatus === "development" && <Badge variant="secondary">{t("settings.app.playerFollowerDevelopmentShort")}</Badge>}
          <Button variant="outline" size="sm" disabled={savingFollower || followerUnavailable} onClick={() => void chooseFollower()}><Plus />{t("settings.app.choosePlayerFollower")}</Button>
        </div>
      </div>
      <ApplicationList applications={config.app.playerFollowerApplication ? [config.app.playerFollowerApplication] : []} icons={applicationIcons} names={applicationNames} busy={savingFollower || followerUnavailable} emptyLabel={t("settings.app.playerFollowerEmpty")} removeLabel={t("common.actions.remove")} onRemove={() => void clearFollower()} />
      {followerStatus === "unsupported" && <p className={styles.cardHint} data-error="true">{t("settings.app.playerFollowerUnsupported")}</p>}
      {(followerStatus === "not_found" || followerStatus === "not_registered") && config.app.playerFollowerApplication && <div className={styles.systemApplicationsToolbar}>
        <p className={styles.cardHint} data-error="true">{t(followerStatus === "not_found" ? "settings.app.playerFollowerNotFound" : "settings.app.playerFollowerNotRegistered")}</p>
        <Button variant="outline" size="sm" disabled={savingFollower} onClick={() => void retryFollower()}>{t("settings.app.retryPlayerFollower")}</Button>
      </div>}
      {followerStatus === "requires_approval" && <div className={styles.systemApplicationsToolbar}>
        <p className={styles.cardHint} data-error="true">{t("settings.app.playerFollowerApproval")}</p>
        <Button variant="outline" size="sm" onClick={() => void api.openPlayerFollowerSystemSettings().catch((error) => setError(messageOf(error)))}>{t("settings.app.openLoginItems")}</Button>
      </div>}
    </SettingsSection>
    </>}
    {scope === "application" && <>
    <SettingsSection id="application-startup" title={t("settings.player.startup")}>
      <ToggleRow label={t("settings.app.silentStartup")} description={t("settings.player.silentStartupHint")} value={config.app.silentStartup} onChange={(enabled) => setSilentStartup(enabled).catch((error) => setError(messageOf(error)))} />
      <ToggleRow label={t("settings.app.hideDock")} description={t("settings.app.hideDockHint")} value={config.app.hideDockIcon} onChange={(hidden) => setDockIconHidden(hidden).catch((error) => setError(messageOf(error)))} />
      <ToggleRow label={t("settings.app.hideMenuBarIcon")} description={t("settings.app.hideMenuBarIconHint")} value={config.app.hideMenuBarIcon} onChange={(hidden) => setMenuBarIconHidden(hidden).catch((error) => setError(messageOf(error)))} />
    </SettingsSection>
    <SettingsSection id="application-display" title={t("settings.app.display")}>
      <Field orientation="horizontal" className={styles.settingRow}>
        <FieldContent>
          <FieldTitle>{t("settings.app.themeLabel")}</FieldTitle>
          <FieldDescription>{t("settings.app.themeHint")}</FieldDescription>
        </FieldContent>
        <ToggleGroup variant="outline" size="sm" spacing={0} value={[config.app.theme]} onValueChange={(values) => { const theme = values[0] as ThemePreference | undefined; if (theme) void setTheme(theme).catch((error) => setError(messageOf(error))); }}>
          <ToggleGroupItem value="light" aria-label={t("settings.theme.light")}><Sun data-icon="inline-start" /><span>{t("settings.theme.light")}</span></ToggleGroupItem>
          <ToggleGroupItem value="dark" aria-label={t("settings.theme.dark")}><Moon data-icon="inline-start" /><span>{t("settings.theme.dark")}</span></ToggleGroupItem>
          <ToggleGroupItem value="system" aria-label={t("settings.theme.system")}><Monitor data-icon="inline-start" /><span>{t("settings.theme.system")}</span></ToggleGroupItem>
        </ToggleGroup>
      </Field>
      <SelectRow label={t("settings.app.language.label")} description={t("settings.app.language.description")} value={normalizeLanguagePreference(config.app.language)} options={[["system", t("common.language.system")], ...languageOptions.map(({ code, label }) => [code, label] as [string, string])]} onChange={(language) => void setLanguage(language as LanguagePreference).catch((error) => setError(messageOf(error)))} />
      <ToggleRow
        label={t("settings.app.lyricsWindowsShowOnAllSpaces")}
        description={t("settings.app.lyricsWindowsShowOnAllSpacesHint")}
        value={config.app.lyricsWindowsShowOnAllSpaces}
        onChange={(enabled) => void setLyricsWindowsShowOnAllSpaces(enabled).catch((error) => setError(messageOf(error)))}
      />
    </SettingsSection>
    <SettingsSection id="application-shortcuts" title={t("settings.app.shortcuts")}>
      <div className={styles.shortcutRow}><span>{t("settings.app.openSettings")}</span><kbd>⌘ ,</kbd></div>
      {shortcutActions.map((action) => {
        const active = recording === action;
        const isDefault = config.app.shortcuts[action] === defaultGlobalShortcuts[action];
        return <div className={styles.shortcutRow} key={action}><span>{t(`settings.app.${action}`)}</span><div className={styles.shortcutControls}>
          <Button ref={(element) => { shortcutRecorderRefs.current[action] = element; }} variant="outline" size="sm" className={styles.shortcutRecorder} aria-pressed={active} data-recording={active} disabled={savingShortcut} onClick={() => setRecording(active ? null : action)} onKeyDown={(event) => {
            if (!active) return;
            event.preventDefault();
            if (event.key === "Escape") return setRecording(null);
            const shortcut = shortcutFromEvent(event);
            if (shortcut) void saveShortcut(action, shortcut);
          }}>{active ? t("settings.app.record") : config.app.shortcuts[action].trim() ? shortcutDisplay(config.app.shortcuts[action]) : t("settings.app.shortcutUnset")}</Button>
          <Button variant="ghost" size="sm" disabled={savingShortcut || isDefault} onClick={() => void saveShortcut(action, defaultGlobalShortcuts[action])}>{t("common.actions.resetDefault")}</Button>
        </div></div>;
      })}
      {unavailableShortcuts.length > 0 && <p className={styles.cardHint} data-error="true">{t("settings.app.shortcutUnavailable", { actions: unavailableShortcuts.map((action) => t(`settings.app.${action}`)).join(", ") })}</p>}
    </SettingsSection>
    </>}
  </SettingsPage>;
}
