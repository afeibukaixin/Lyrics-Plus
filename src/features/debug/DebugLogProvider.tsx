import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { attachLogger, LogLevel } from "@tauri-apps/plugin-log";
import { reportFrontendError } from "../../shared/debugLog";
import { disposeTauriListener } from "../../shared/tauriEvent";

export type DebugLogLevel = "debug" | "info" | "warn" | "error";

export type DebugLogEntry = {
  id: number;
  receivedAt: number;
  level: DebugLogLevel;
  message: string;
};

const MAX_LOG_ENTRIES = 300;
export const debugLogLevels: DebugLogLevel[] = ["debug", "info", "warn", "error"];

type DebugLogContextValue = {
  enabled: boolean;
  entries: DebugLogEntry[];
  visibleLevels: Set<DebugLogLevel>;
  setEnabled: (enabled: boolean) => void;
  toggleLevel: (level: DebugLogLevel) => void;
  clear: () => void;
};

const DebugLogContext = createContext<DebugLogContextValue | null>(null);

function levelOf(level: LogLevel): DebugLogLevel | null {
  switch (level) {
    case LogLevel.Debug: return "debug";
    case LogLevel.Info: return "info";
    case LogLevel.Warn: return "warn";
    case LogLevel.Error: return "error";
    default: return null;
  }
}

export function DebugLogProvider({ children }: { children: React.ReactNode }) {
  const [enabled, setEnabledState] = useState(false);
  const [entries, setEntries] = useState<DebugLogEntry[]>([]);
  const [visibleLevels, setVisibleLevels] = useState<Set<DebugLogLevel>>(
    () => new Set(debugLogLevels),
  );
  const sequence = useRef(0);

  const append = useCallback((level: DebugLogLevel, message: string) => {
    const entry: DebugLogEntry = {
      id: ++sequence.current,
      receivedAt: Date.now(),
      level,
      message,
    };
    setEntries((current) => [...current, entry].slice(-MAX_LOG_ENTRIES));
  }, []);

  const setEnabled = useCallback((next: boolean) => {
    setEntries([]);
    setVisibleLevels(new Set(debugLogLevels));
    setEnabledState(next);
  }, []);

  const clear = useCallback(() => setEntries([]), []);

  const toggleLevel = useCallback((level: DebugLogLevel) => {
    setVisibleLevels((current) => {
      const next = new Set(current);
      if (next.has(level)) next.delete(level);
      else next.add(level);
      return next;
    });
  }, []);

  useEffect(() => {
    if (!enabled) return;
    let disposed = false;
    let detach: (() => void) | undefined;

    void attachLogger(({ level, message }) => {
      const normalized = levelOf(level);
      if (!disposed && normalized) append(normalized, message);
    }).then((unlisten) => {
      if (disposed) {
        disposeTauriListener(unlisten);
        return;
      }
      detach = unlisten;
      append("info", "调试日志已开启，仅显示本次开启后的记录。");
    }).catch((error) => {
      if (!disposed) append("error", `连接日志流失败：${String(error)}`);
    });

    const handleWindowError = (event: ErrorEvent) => {
      reportFrontendError("主窗口未处理异常", event.error ?? event.message);
    };
    const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
      reportFrontendError("主窗口未处理 Promise 拒绝", event.reason);
    };
    window.addEventListener("error", handleWindowError);
    window.addEventListener("unhandledrejection", handleUnhandledRejection);

    return () => {
      disposed = true;
      disposeTauriListener(detach);
      window.removeEventListener("error", handleWindowError);
      window.removeEventListener("unhandledrejection", handleUnhandledRejection);
    };
  }, [append, enabled]);

  const value = useMemo<DebugLogContextValue>(() => ({
    enabled,
    entries,
    visibleLevels,
    setEnabled,
    toggleLevel,
    clear,
  }), [clear, enabled, entries, setEnabled, toggleLevel, visibleLevels]);

  return <DebugLogContext.Provider value={value}>{children}</DebugLogContext.Provider>;
}

export function useDebugLogs() {
  const value = useContext(DebugLogContext);
  if (!value) throw new Error("useDebugLogs 必须在 DebugLogProvider 内使用");
  return value;
}
