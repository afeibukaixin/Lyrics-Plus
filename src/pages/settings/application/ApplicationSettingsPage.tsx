import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Monitor, Moon, Sun } from "lucide-react";

import { defaultGlobalShortcuts, type GlobalShortcutSettings, type GlobalShortcutStatus, type LanguagePreference, type ThemePreference } from "../../../shared/types";
import { api, messageOf } from "../../../shared/api";
import { languageRegistry, supportedLanguages } from "../../../shared/languages";
import { normalizeLanguagePreference } from "../../../features/i18n/i18n";
import { Button } from "@/components/ui/button";
import { Field, FieldContent, FieldDescription, FieldTitle } from "@/components/ui/field";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

import { useSettingsContext } from "../shared/SettingsContext";
import styles from "../settings.module.scss";
import { PageHeader, SelectRow, SettingsPage, SettingsSection, ToggleRow } from "../shared/components";

const languageOptions = supportedLanguages.map((code) => ({ code, label: languageRegistry[code].nativeLabel }));
type ShortcutAction = keyof GlobalShortcutSettings;
const shortcutActions: ShortcutAction[] = [
  "toggleOverlay",
  "unlockOverlay",
  "resetOverlay",
  "toggleStatusBarLyrics",
  "toggleListLyrics",
  "toggleNotchLyrics",
  "switchLyrics",
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

export default function ApplicationSettingsPage() {
  const {
    config,
    setTheme,
    setLanguage,
    setGlobalShortcuts,
    setDockIconHidden,
    setMenuBarIconHidden,
    setSilentStartup,
    setLyricsWindowsShowOnAllSpaces,
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

  useEffect(() => {
    if (recording) shortcutRecorderRefs.current[recording]?.focus();
  }, [recording]);

  useEffect(() => {
    void api.getGlobalShortcutStatus().then(setShortcutStatus).catch(() => setShortcutStatus(null));
  }, []);

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

  return <SettingsPage sections={[
    { id: "application-startup", label: t("settings.player.startup") },
    { id: "application-display", label: t("settings.app.display") },
    { id: "application-shortcuts", label: t("settings.app.shortcuts") },
  ]}>
    <PageHeader
      title={t("settings.app.title")}
      description={t("settings.app.description")}
      onReset={() => void resetSection("application")}
      resetting={resettingSection === "application"}
      confirming={confirmingReset === "application"}
    />
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
  </SettingsPage>;
}
