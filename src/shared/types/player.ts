export type PlayerKind = "apple_music" | "spotify" | "system";
export type PlayerSelection = "auto" | PlayerKind;
export type PlaybackAction =
  | "play"
  | "pause"
  | "toggle_play_pause"
  | "previous"
  | "next";
export type SystemMediaFilterMode = "allowlist" | "blocklist";
export type PlaybackErrorCode =
  | "waiting"
  | "not_installed"
  | "automation_denied"
  | "response_timeout"
  | "invalid_response"
  | "multiple_playing"
  | "no_unique_player"
  | "source_not_allowed"
  | "unavailable";

export type RegisteredApplication = {
  name: string;
  bundleId: string;
};

export type PlayerFollowerServiceState =
  | "development"
  | "unsupported"
  | "not_registered"
  | "enabled"
  | "requires_approval"
  | "not_found";

export type GlobalShortcutSettings = {
  toggleOverlay: string;
  unlockOverlay: string;
  resetOverlay: string;
  toggleStatusBarLyrics: string;
  toggleListLyrics: string;
  toggleNotchLyrics: string;
  switchLyrics: string;
};

export type GlobalShortcutStatus = Record<keyof GlobalShortcutSettings, boolean>;

export type LegalNoticeStatus = {
  currentVersion: number;
  accepted: boolean;
};

export const defaultGlobalShortcuts: GlobalShortcutSettings = {
  toggleOverlay: "CommandOrControl+Shift+KeyL",
  unlockOverlay: "CommandOrControl+Shift+KeyU",
  resetOverlay: "CommandOrControl+Shift+Digit0",
  toggleStatusBarLyrics: "",
  toggleListLyrics: "",
  toggleNotchLyrics: "",
  switchLyrics: "CommandOrControl+Shift+KeyY",
};
export type PlaybackSnapshot = {
  player: PlayerKind | null;
  isRunning: boolean;
  isPlaying: boolean;
  trackId: string | null;
  title: string | null;
  artist: string | null;
  album: string | null;
  sourceAppName: string | null;
  sourceAppBundleId: string | null;
  artworkId: string | null;
  durationMs: number | null;
  positionMs: number | null;
  observedAtMs: number;
  errorCode: PlaybackErrorCode | null;
  error: string | null;
};

export type PlaybackArtwork = {
  id: string;
  mimeType: string;
  dataBase64: string;
  accentColor: string;
  spectrumColors: PlaybackSpectrumColors;
};

export type PlaybackSpectrumColors = {
  left: PlaybackSpectrumColumnColors;
  center: PlaybackSpectrumColumnColors;
  right: PlaybackSpectrumColumnColors;
};

export type PlaybackSpectrumColumnColors = {
  top: string;
  middle: string;
  bottom: string;
};

export type PlaybackSpectrumStatus =
  | "idle"
  | "waiting"
  | "starting"
  | "running"
  | "permission_denied"
  | "unsupported"
  | "unavailable";

export type PlaybackSpectrumBands = [
  number,
  number,
  number,
  number,
  number,
  number,
];

export type PlaybackSpectrumFrame = {
  bands: PlaybackSpectrumBands;
  sourceAppBundleId: string | null;
  observedAtMs: number;
};

export type PlaybackSpectrumState = {
  status: PlaybackSpectrumStatus;
  sourceAppBundleId: string | null;
  error: string | null;
};
