import { appI18n } from "../features/i18n/i18n";
import {
  AppOperationError,
  type AppErrorCode,
} from "./api/core";
import { applicationApi } from "./api/application";
import { legalApi } from "./api/legal";
import { lyricsApi } from "./api/lyrics";
import { overlayApi } from "./api/overlay";
import { playbackApi } from "./api/playback";
import { settingsApi } from "./api/settings";
import type { PlaybackSnapshot } from "./types";

export { isTauriRuntime } from "./tauriEvent";
export { AppOperationError };
export type { AppErrorCode, NotchWindowFitResponse } from "./api/core";

export const api = {
  ...legalApi,
  ...playbackApi,
  ...lyricsApi,
  ...overlayApi,
  ...applicationApi,
  ...settingsApi,
};

export function messageOf(error: unknown): string {
  if (error instanceof AppOperationError) {
    if ([
      "set_global_shortcuts",
      "set_provider_settings",
      "set_system_media_filter_mode",
      "set_system_media_applications",
      "resolve_system_media_applications",
      "resolve_player_follower_application",
      "set_player_follower_application",
      "resolve_application_by_bundle_id",
      "control_playback",
      "seek_playback",
      "get_playback_artwork",
      "start_playback_spectrum",
      "stop_playback_spectrum",
      "get_playback_spectrum_state",
    ].includes(error.command) && error.message) {
      return error.message;
    }
    return error.code === "config.conflict"
      ? appI18n.t("errors.configConflict")
      : appI18n.t("errors.command");
  }
  return appI18n.t("errors.unknown");
}

export function errorCodeOf(error: unknown): AppErrorCode {
  return error instanceof AppOperationError ? error.code : "unknown";
}

export function trackKeyOf(snapshot: PlaybackSnapshot): string | null {
  const title = snapshot.title?.trim();
  const artist = snapshot.artist?.trim();
  const trackId = snapshot.trackId?.trim();
  if (!snapshot.player || !title || !artist) return null;
  if (trackId) return `${snapshot.player}:${trackId}`;
  const fallback = `${title}|${artist}|${snapshot.durationMs ?? 0}`
    .toLowerCase()
    .replace(/\s+/g, " ")
    .trim();
  return `${snapshot.player}:fallback:${fallback}`;
}
