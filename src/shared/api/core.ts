import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { reportFrontendError } from "../debugLog";

export type AppErrorCode = `command.${string}` | "config.conflict" | "unknown";

export type NotchWindowFitResponse = {
  physicalWidth: number;
  physicalHeight: number;
  sizeChanged: boolean;
};

export class AppOperationError extends Error {
  readonly code: AppErrorCode;
  readonly command: string;
  readonly cause: unknown;

  constructor(command: string, cause: unknown) {
    const detail = cause instanceof Error ? cause.message : String(cause);
    super(detail);
    this.name = "AppOperationError";
    this.command = command;
    this.cause = cause;
    this.code = command === "save_app_config_draft" && detail.startsWith("config.conflict:")
      ? "config.conflict"
      : `command.${command}`;
  }
}

export function invoke<T>(command: string, args?: Record<string, unknown>) {
  return tauriInvoke<T>(command, args).catch((error) => {
    reportFrontendError(`Tauri command '${command}' failed`, error);
    throw new AppOperationError(command, error);
  });
}
