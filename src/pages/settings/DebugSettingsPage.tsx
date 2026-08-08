import { useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { debugLogLevels, useDebugLogs, type DebugLogLevel } from "../../features/debug/DebugLogProvider";
import { useAppLanguage } from "../../features/i18n/I18nProvider";
import styles from "../settings.module.scss";
import { SettingsCard, SettingsHeading, ToggleRow } from "./components";

const debugLevelLabels: Record<DebugLogLevel, string> = {
  debug: "DEBUG",
  info: "INFO",
  warn: "WARN",
  error: "ERROR",
};

function formatDebugTime(value: number, language: string) {
  return new Intl.DateTimeFormat(language, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(value);
}

export default function DebugSettingsPage() {
  const { t } = useTranslation();
  const { language } = useAppLanguage();
  const debugLogs = useDebugLogs();
  const viewport = useRef<HTMLDivElement>(null);
  const visibleEntries = useMemo(
    () => debugLogs.entries.filter((entry) => debugLogs.visibleLevels.has(entry.level)),
    [debugLogs.entries, debugLogs.visibleLevels],
  );

  useEffect(() => {
    if (!debugLogs.enabled) return;
    const element = viewport.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [debugLogs.enabled, visibleEntries.length]);

  return (
    <>
      <SettingsHeading title={t("settings.debug.title")} description={t("settings.debug.description")} />
      <SettingsCard title={t("settings.debug.live")} trailing={debugLogs.enabled && <span className={styles.debugLogCount}>{debugLogs.entries.length} / 300</span>}>
        <ToggleRow label={t("settings.debug.toggle")} description={t("settings.debug.toggleHint")} value={debugLogs.enabled} onChange={debugLogs.setEnabled} />
        {debugLogs.enabled ? (
          <>
            <div className={styles.debugLogToolbar}>
              <div role="group" aria-label={t("settings.debug.filter")}>
                {debugLogLevels.map((level) => (
                  <button type="button" key={level} data-level={level} aria-pressed={debugLogs.visibleLevels.has(level)} onClick={() => debugLogs.toggleLevel(level)}>{debugLevelLabels[level]}</button>
                ))}
              </div>
              <button type="button" onClick={debugLogs.clear} disabled={debugLogs.entries.length === 0}>{t("settings.debug.clear")}</button>
            </div>
            <div className={styles.debugLogViewport} ref={viewport} role="log" aria-live="polite">
              {visibleEntries.length === 0 ? (
                <p>{debugLogs.entries.length === 0 ? t("settings.debug.waiting") : t("settings.debug.filteredEmpty")}</p>
              ) : visibleEntries.map((entry) => (
                <div className={styles.debugLogEntry} data-level={entry.level} key={entry.id}>
                  <time dateTime={new Date(entry.receivedAt).toISOString()}>{formatDebugTime(entry.receivedAt, language)}</time>
                  <strong>{debugLevelLabels[entry.level]}</strong>
                  <code>{entry.message}</code>
                </div>
              ))}
            </div>
          </>
        ) : (
          <p className={styles.cardHint}>{t("settings.debug.disabledHint")}</p>
        )}
      </SettingsCard>
    </>
  );
}
