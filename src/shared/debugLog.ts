import { error as writeErrorLog } from "@tauri-apps/plugin-log";

function isTauriRuntime() {
  if (typeof window === "undefined") return false;
  const internals = (window as Window & {
    __TAURI_INTERNALS__?: { invoke?: unknown; transformCallback?: unknown };
  }).__TAURI_INTERNALS__;
  return typeof internals?.invoke === "function" && typeof internals.transformCallback === "function";
}

function detailOf(error: unknown) {
  if (error instanceof Error) return error.stack || error.message;
  if (typeof error === "string") return error;
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

export function reportFrontendError(context: string, error: unknown) {
  if (!isTauriRuntime()) return;
  const detail = detailOf(error).trim();
  void writeErrorLog(detail ? `${context}：${detail}` : context).catch(() => {
    // 日志上报本身失败时不再递归记录。
  });
}
