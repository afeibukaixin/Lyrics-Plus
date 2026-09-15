import { memo, useEffect, useMemo, useRef } from "react";
import { useTranslation } from "react-i18next";
import { debugLogLevels, useDebugLogs, type DebugLogEntry, type DebugLogLevel } from "../../../features/debug/DebugLogProvider";
import { useAppLanguage } from "../../../features/i18n/I18nProvider";
import styles from "../settings.module.scss";
import { PageHeader, SettingsSection, ToggleRow } from "../shared/components";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Check } from "lucide-react";
import { cn } from "@/lib/utils";

const debugLevelLabels: Record<DebugLogLevel, string> = {
  debug: "DEBUG",
  info: "INFO",
  warn: "WARN",
  error: "ERROR",
};

const debugTimeFormatters = new Map<string, Intl.DateTimeFormat>();

function formatDebugTime(value: number, language: string) {
  let formatter = debugTimeFormatters.get(language);
  if (!formatter) {
    formatter = new Intl.DateTimeFormat(language, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });
    debugTimeFormatters.set(language, formatter);
  }
  return formatter.format(value);
}

const DebugLogEntryRow = memo(function DebugLogEntryRow({
  entry,
  language,
}: {
  entry: DebugLogEntry;
  language: string;
}) {
  return (
    <div className={styles.debugLogEntry} data-level={entry.level}>
      <time dateTime={new Date(entry.receivedAt).toISOString()}>{formatDebugTime(entry.receivedAt, language)}</time>
      <strong>{debugLevelLabels[entry.level]}</strong>
      <code>{entry.message}</code>
    </div>
  );
});

export default function DebugSettingsPage() {
  const { t } = useTranslation();
  const { language } = useAppLanguage();
  const debugLogs = useDebugLogs();
  const viewport = useRef<HTMLDivElement>(null);
  const visibleEntries = useMemo(
    () => debugLogs.entries.filter((entry) => debugLogs.visibleLevels.has(entry.level)),
    [debugLogs.entries, debugLogs.visibleLevels],
  );
  const lastVisibleEntryId = visibleEntries.length > 0
    ? visibleEntries[visibleEntries.length - 1].id
    : null;

  useEffect(() => {
    if (!debugLogs.enabled || lastVisibleEntryId === null) return;
    const element = viewport.current;
    if (!element) return;
    const frame = window.requestAnimationFrame(() => {
      if (viewport.current === element) element.scrollTop = element.scrollHeight;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [debugLogs.enabled, lastVisibleEntryId, visibleEntries.length]);

  return (
    <>
      <PageHeader title={t("settings.debug.title")} description={t("settings.debug.description")} />
      <SettingsSection id="debug-live" title={t("settings.debug.live")} trailing={debugLogs.enabled && <span className={styles.debugLogCount}>{debugLogs.entries.length} / 300</span>}>
        <ToggleRow label={t("settings.debug.toggle")} description={t("settings.debug.toggleHint")} value={debugLogs.enabled} onChange={debugLogs.setEnabled} />
        {debugLogs.enabled ? (
          <>
            <div className={styles.debugLogToolbar}>
              <ToggleGroup multiple variant="outline" size="sm" aria-label={t("settings.debug.filter")} value={[...debugLogs.visibleLevels]} onValueChange={(values) => {
                const nextLevels = new Set(values as DebugLogLevel[]);
                const changed = debugLogLevels.find((level) => nextLevels.has(level) !== debugLogs.visibleLevels.has(level));
                if (changed) debugLogs.toggleLevel(changed);
              }}>
                {debugLogLevels.map((level) => (
                  <ToggleGroupItem className="font-mono" key={level} value={level} data-level={level}>
                    <Check
                      data-icon="inline-start"
                      aria-hidden="true"
                      className={cn(!debugLogs.visibleLevels.has(level) && "invisible")}
                    />
                    {debugLevelLabels[level]}
                  </ToggleGroupItem>
                ))}
              </ToggleGroup>
              <Button type="button" variant="outline" size="sm" onClick={debugLogs.clear} disabled={debugLogs.entries.length === 0}>{t("settings.debug.clear")}</Button>
            </div>
            <ScrollArea className={styles.debugLogViewport} viewportRef={viewport} role="log" aria-live="polite">
              {visibleEntries.length === 0 ? (
                <p>{debugLogs.entries.length === 0 ? t("settings.debug.waiting") : t("settings.debug.filteredEmpty")}</p>
              ) : visibleEntries.map((entry) => <DebugLogEntryRow entry={entry} key={entry.id} language={language} />)}
            </ScrollArea>
          </>
        ) : null}
      </SettingsSection>
    </>
  );
}
