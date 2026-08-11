import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { defaultGlobalShortcuts, type GlobalShortcutSettings, type GlobalShortcutStatus, type LanguagePreference, type PlayerSelection } from "../../shared/types";
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

export default function AppSettingsPage() {
  const {
    config,
    setUiFontScale,
    setLanguage,
    setGlobalShortcuts,
    setSystemMediaApplications,
    setDockIconHidden,
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
  const { t } = useTranslation();

  useEffect(() => {
    void api.getGlobalShortcutStatus().then(setShortcutStatus).catch(() => setShortcutStatus(null));
  }, []);

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
  const currentBundleId = playback.snapshot.sourceAppBundleId;
  const canAddCurrent = Boolean(currentBundleId)
    && !["com.apple.Music", "com.spotify.client"].includes(currentBundleId ?? "")
    && !config.app.systemMediaApplications.some((application) => application.bundleId === currentBundleId);
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
  const chooseApplications = async () => {
    const paths = await open({
      multiple: true,
      filters: [{ name: t("settings.app.systemApplicationsPicker"), extensions: ["app"] }],
    });
    if (!paths) return;
    setSavingApplications(true);
    setError(null);
    try {
      const resolved = await api.resolveSystemMediaApplications(Array.isArray(paths) ? paths : [paths]);
      await setSystemMediaApplications([...config.app.systemMediaApplications, ...resolved]);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSavingApplications(false);
    }
  };

  return (
    <>
      <SettingsHeading title={t("settings.app.title")} description={t("settings.app.description")} onReset={() => void resetSection("app")} resetting={resettingSection === "app"} confirming={confirmingReset === "app"} />
      <SettingsCard title={t("settings.app.player")}>
        <div className={styles.playerOptions}>{playerOptions.map((option) => <button key={option} data-active={playback.selection === option} onClick={() => playback.setSelection(option)}>{option === "auto" ? t("settings.app.playerAuto") : option === "apple_music" ? "Apple Music" : option === "spotify" ? "Spotify" : t("settings.app.playerSystem")}</button>)}</div>
      </SettingsCard>
      <SettingsCard title={t("settings.app.systemApplications")}>
        <div className={styles.systemApplicationsToolbar}>
          <p className={styles.cardHint}>{t("settings.app.systemApplicationsHint")}</p>
          <div className={styles.shortcutControls}>
            <button className={styles.shortcutReset} disabled={savingApplications || !canAddCurrent} onClick={() => void saveApplications([...config.app.systemMediaApplications, { name: playback.snapshot.sourceAppName ?? currentBundleId!, bundleId: currentBundleId! }])}>{t("settings.app.addCurrentApplication")}</button>
            <button className={styles.shortcutReset} disabled={savingApplications} onClick={() => void chooseApplications()}>{t("settings.app.chooseApplications")}</button>
          </div>
        </div>
        {config.app.systemMediaApplications.map((application) => (
          <div className={styles.shortcutRow} key={application.bundleId}>
            <span>{application.name}<small>{application.bundleId}</small></span>
            <button className={styles.shortcutReset} disabled={savingApplications} onClick={() => void saveApplications(config.app.systemMediaApplications.filter((item) => item.bundleId !== application.bundleId))}>{t("common.actions.remove")}</button>
          </div>
        ))}
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
