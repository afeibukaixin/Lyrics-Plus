import { createContext, startTransition, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { attachLogger, LogLevel } from "@tauri-apps/plugin-log";
import { frontendErrorDetail, reportFrontendError } from "../../shared/debugLog";
import { disposeTauriListener } from "../../shared/tauriEvent";

export type DebugLogLevel = "debug" | "info" | "warn" | "error";

export type DebugLogEntry = {
  id: number;
  receivedAt: number;
  level: DebugLogLevel;
  message: string;
};

const MAX_LOG_ENTRIES = 300;
const LOG_FLUSH_DELAY_MS = 100;
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
  const pendingEntries = useRef<DebugLogEntry[]>([]);
  const flushTimer = useRef<number | null>(null);
  const streamGeneration = useRef(0);

  const cancelPendingFlush = useCallback(() => {
    streamGeneration.current += 1;
    pendingEntries.current = [];
    if (flushTimer.current !== null) {
      window.clearTimeout(flushTimer.current);
      flushTimer.current = null;
    }
  }, []);

  const flushPending = useCallback(() => {
    flushTimer.current = null;
    if (pendingEntries.current.length === 0) return;
    const generation = streamGeneration.current;
    const batch = pendingEntries.current;
    pendingEntries.current = [];
    startTransition(() => {
      setEntries((current) => streamGeneration.current === generation
        ? [...current, ...batch].slice(-MAX_LOG_ENTRIES)
        : current);
    });
  }, []);

  const scheduleFlush = useCallback(() => {
    if (flushTimer.current !== null) return;
    flushTimer.current = window.setTimeout(flushPending, LOG_FLUSH_DELAY_MS);
  }, [flushPending]);

  const append = useCallback((level: DebugLogLevel, message: string) => {
    const entry: DebugLogEntry = {
      id: ++sequence.current,
      receivedAt: Date.now(),
      level,
      message,
    };
    pendingEntries.current.push(entry);
    scheduleFlush();
  }, [scheduleFlush]);

  const setEnabled = useCallback((next: boolean) => {
    cancelPendingFlush();
    setEntries([]);
    setVisibleLevels(new Set(debugLogLevels));
    setEnabledState(next);
  }, [cancelPendingFlush]);

  const clear = useCallback(() => {
    cancelPendingFlush();
    setEntries([]);
  }, [cancelPendingFlush]);

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
      append("info", "Frontend debug log stream attached; showing entries from this session only.");
    }).catch((error) => {
      reportFrontendError("Failed to attach the frontend debug log stream", error);
      if (!disposed) append("error", `Failed to attach the frontend debug log stream: ${frontendErrorDetail(error)}`);
    });

    const handleWindowError = (event: ErrorEvent) => {
      reportFrontendError("Unhandled error in the main window", event.error ?? event.message);
    };
    const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
      reportFrontendError("Unhandled promise rejection in the main window", event.reason);
    };
    window.addEventListener("error", handleWindowError);
    window.addEventListener("unhandledrejection", handleUnhandledRejection);

    return () => {
      disposed = true;
      cancelPendingFlush();
      disposeTauriListener(detach);
      window.removeEventListener("error", handleWindowError);
      window.removeEventListener("unhandledrejection", handleUnhandledRejection);
    };
  }, [append, cancelPendingFlush, enabled]);

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
  if (!value) throw new Error("useDebugLogs must be used within DebugLogProvider");
  return value;
}
