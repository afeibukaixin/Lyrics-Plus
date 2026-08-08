import { error as writeErrorLog } from "@tauri-apps/plugin-log";

function isTauriRuntime() {
  if (typeof window === "undefined") return false;
  const internals = (window as Window & {
    __TAURI_INTERNALS__?: { invoke?: unknown; transformCallback?: unknown };
  }).__TAURI_INTERNALS__;
  return typeof internals?.invoke === "function" && typeof internals.transformCallback === "function";
}

function detailOf(error: unknown, seen: Set<unknown>): string {
  if (seen.has(error)) return "[Circular error cause]";
  seen.add(error);

  if (error instanceof Error) {
    const cause = (error as Error & { cause?: unknown }).cause;
    const detail = error.stack || error.message || error.name;
    return cause === undefined
      ? detail
      : `${detail}\nCaused by: ${detailOf(cause, seen)}`;
  }
  if (typeof error === "string") return error;
  try {
    return JSON.stringify(error) ?? String(error);
  } catch {
    return String(error);
  }
}

export function frontendErrorDetail(error: unknown) {
  return detailOf(error, new Set());
}

export function reportFrontendError(context: string, error: unknown) {
  if (!isTauriRuntime()) return;
  const detail = frontendErrorDetail(error).trim();
  void writeErrorLog(detail ? `${context}: ${detail}` : context).catch(() => {
    // Do not recurse when reporting the log entry itself fails.
  });
}
