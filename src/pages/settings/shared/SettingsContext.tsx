import { useOutletContext } from "react-router";
import type { Dispatch, PointerEvent as ReactPointerEvent, RefObject, SetStateAction } from "react";
import { useAppConfig } from "../../../features/config/AppConfigProvider";
import { useLyrics } from "../../../features/lyrics/useLyrics";
import { usePlayback } from "../../../features/player/usePlayback";
import type {
  OverlaySettings,
  OverlayStyle,
  ProviderCredentialView,
  ProviderSettings,
  ProviderSettingsView,
  MusixmatchTokenType,
  SettingsSection,
} from "../../../shared/types";

export type ProviderDragState = {
  providerId: string;
  pointerId: number;
  sourceIndex: number;
  targetIndex: number;
  startY: number;
  currentY: number;
  positions: Array<{ top: number; center: number }>;
};

export type SettingsOutletContext = {
  config: ReturnType<typeof useAppConfig>["config"];
  setTheme: ReturnType<typeof useAppConfig>["setTheme"];
  setLanguage: ReturnType<typeof useAppConfig>["setLanguage"];
  setGlobalShortcuts: ReturnType<typeof useAppConfig>["setGlobalShortcuts"];
  setSystemMediaFilterMode: ReturnType<typeof useAppConfig>["setSystemMediaFilterMode"];
  setSystemMediaApplications: ReturnType<typeof useAppConfig>["setSystemMediaApplications"];
  setPlayerFollowerApplication: ReturnType<typeof useAppConfig>["setPlayerFollowerApplication"];
  setDockIconHidden: ReturnType<typeof useAppConfig>["setDockIconHidden"];
  setMenuBarIconHidden: ReturnType<typeof useAppConfig>["setMenuBarIconHidden"];
  setSilentStartup: ReturnType<typeof useAppConfig>["setSilentStartup"];
  setLyricsWindowsShowOnAllSpaces: ReturnType<typeof useAppConfig>["setLyricsWindowsShowOnAllSpaces"];
  setOverlayHideWhenNotPlaying: ReturnType<typeof useAppConfig>["setOverlayHideWhenNotPlaying"];
  setStatusBarLyricsEnabled: ReturnType<typeof useAppConfig>["setStatusBarLyricsEnabled"];
  setListLyricsVisible: ReturnType<typeof useAppConfig>["setListLyricsVisible"];
  setListLyricsOptions: ReturnType<typeof useAppConfig>["setListLyricsOptions"];
  setListLyricsLocked: ReturnType<typeof useAppConfig>["setListLyricsLocked"];
  setNotchLyricsVisible: ReturnType<typeof useAppConfig>["setNotchLyricsVisible"];
  setLyricsDisplayPreferences: ReturnType<typeof useAppConfig>["setLyricsDisplayPreferences"];
  setLyricsBaseAppearance: ReturnType<typeof useAppConfig>["setLyricsBaseAppearance"];
  setLyricsStyleInheritance: ReturnType<typeof useAppConfig>["setLyricsStyleInheritance"];
  resetLyricsBaseAppearance: ReturnType<typeof useAppConfig>["resetLyricsBaseAppearance"];
  playback: ReturnType<typeof usePlayback>;
  lyrics: ReturnType<typeof useLyrics>;
  fileInput: RefObject<HTMLInputElement | null>;
  providerRows: RefObject<Map<string, HTMLDivElement>>;
  overlaySettings: OverlaySettings;
  style: OverlayStyle;
  providerView: ProviderSettingsView | null;
  providerCredentials: ProviderCredentialView | null;
  testingProvider: string | null;
  resettingSection: SettingsSection | null;
  confirmingReset: SettingsSection | null;
  providerDrag: ProviderDragState | null;
  savingProviderOrder: boolean;
  setError: Dispatch<SetStateAction<string | null>>;
  setNotice: Dispatch<SetStateAction<string | null>>;
  updateStyle: (patch: Partial<OverlayStyle>) => Promise<boolean>;
  setVisible: (visible: boolean) => Promise<void>;
  setLocked: (locked: boolean) => Promise<void>;
  saveProviderSettings: (settings: ProviderSettings) => Promise<boolean>;
  saveMusixmatchToken: (tokenType: MusixmatchTokenType, token: string) => Promise<boolean>;
  clearMusixmatchToken: () => Promise<boolean>;
  beginProviderDrag: (providerId: string, sourceIndex: number, event: ReactPointerEvent<HTMLButtonElement>) => void;
  continueProviderDrag: (event: ReactPointerEvent<HTMLButtonElement>) => void;
  finishProviderDrag: (event: ReactPointerEvent<HTMLButtonElement>) => void;
  setProviderDrag: Dispatch<SetStateAction<ProviderDragState | null>>;
  providerDragTransform: (index: number) => string | undefined;
  toggleProvider: (id: string) => void;
  testProviders: (providerIds: string[]) => Promise<void>;
  testAllProviders: () => Promise<void>;
  handleFile: (file?: File) => Promise<void>;
  resetSection: (target: SettingsSection) => Promise<void>;
  resetOverlayBounds: () => Promise<void>;
  syncAppliedConfig: (imported: Parameters<ReturnType<typeof useAppConfig>["syncConfig"]>[0], appearanceOnly: boolean) => Promise<void>;
};

export function useSettingsContext() {
  return useOutletContext<SettingsOutletContext>();
}
