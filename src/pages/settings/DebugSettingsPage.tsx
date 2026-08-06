import { useEffect, useMemo, useRef } from "react";
import { debugLogLevels, useDebugLogs, type DebugLogLevel } from "../../features/debug/DebugLogProvider";
import styles from "../settings.module.scss";
import { SettingsCard, SettingsHeading, ToggleRow } from "./components";

const debugLevelLabels: Record<DebugLogLevel, string> = {
  debug: "DEBUG",
  info: "INFO",
  warn: "WARN",
  error: "ERROR",
};

function formatDebugTime(value: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(value);
}

export default function DebugSettingsPage() {
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
      <SettingsHeading title="调试日志" description="查看后端、AppleScript 和前端操作产生的实时错误与调试信息。" />
      <SettingsCard title="实时日志" trailing={debugLogs.enabled && <span className={styles.debugLogCount}>{debugLogs.entries.length} / 300</span>}>
        <ToggleRow label="实时调试日志" description="仅收集本次开启后的日志；关闭、再次开启或重启应用都会清空" value={debugLogs.enabled} onChange={debugLogs.setEnabled} />
        {debugLogs.enabled ? (
          <>
            <div className={styles.debugLogToolbar}>
              <div role="group" aria-label="日志级别筛选">
                {debugLogLevels.map((level) => (
                  <button type="button" key={level} data-level={level} aria-pressed={debugLogs.visibleLevels.has(level)} onClick={() => debugLogs.toggleLevel(level)}>{debugLevelLabels[level]}</button>
                ))}
              </div>
              <button type="button" onClick={debugLogs.clear} disabled={debugLogs.entries.length === 0}>清空</button>
            </div>
            <div className={styles.debugLogViewport} ref={viewport} role="log" aria-live="polite">
              {visibleEntries.length === 0 ? (
                <p>{debugLogs.entries.length === 0 ? "等待新的日志…" : "当前筛选条件下没有日志。"}</p>
              ) : visibleEntries.map((entry) => (
                <div className={styles.debugLogEntry} data-level={entry.level} key={entry.id}>
                  <time dateTime={new Date(entry.receivedAt).toISOString()}>{formatDebugTime(entry.receivedAt)}</time>
                  <strong>{debugLevelLabels[entry.level]}</strong>
                  <code>{entry.message}</code>
                </div>
              ))}
            </div>
          </>
        ) : (
          <p className={styles.cardHint}>开启后开始收集日志；该页面没有“恢复默认”，也不会受到应用设置重置影响。</p>
        )}
      </SettingsCard>
    </>
  );
}
