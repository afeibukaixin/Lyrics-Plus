import {
  useCallback,
  useMemo,
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
} from "react";

import { api, isTauriRuntime } from "../../../shared/api";
import {
  defaultLyricsBaseAppearance,
  type AppConfig,
  type ChineseConversion,
  type GlobalShortcutSettings,
  type LanguagePreference,
  type LyricsBaseAppearance,
  type LyricsDisplayPreferences,
  type LyricsModeStyleInheritance,
  type LyricsStyleMode,
  type RegisteredApplication,
  type SystemMediaFilterMode,
  type ThemePreference,
} from "../../../shared/types";

import { applyPendingNotchPreferences, materializeLyricsStyleInheritance } from "./inheritance";
import type { NotchPreferencesWriteState } from "./subscription";

export type ConfigActions = {
  setTheme: (theme: ThemePreference) => Promise<void>;
  setLanguage: (language: LanguagePreference) => Promise<void>;
  setGlobalShortcuts: (shortcuts: GlobalShortcutSettings) => Promise<void>;
  setSystemMediaFilterMode: (mode: SystemMediaFilterMode) => Promise<void>;
  setSystemMediaApplications: (applications: RegisteredApplication[]) => Promise<void>;
  setPlayerFollowerApplication: (application: RegisteredApplication | null) => Promise<void>;
  setDockIconHidden: (hidden: boolean) => Promise<void>;
  setMenuBarIconHidden: (hidden: boolean) => Promise<void>;
  setSilentStartup: (enabled: boolean) => Promise<void>;
  setAutoCheckUpdates: (enabled: boolean) => Promise<void>;
  setLyricsWindowsShowOnAllSpaces: (enabled: boolean) => Promise<void>;
  setOverlayHideWhenNotPlaying: (hidden: boolean) => Promise<void>;
  setStatusBarLyricsEnabled: (enabled: boolean) => Promise<void>;
  setListLyricsVisible: (visible: boolean) => Promise<void>;
  setListLyricsOptions: (showTranslation: boolean, showRomanization: boolean) => Promise<void>;
  setListLyricsLocked: (locked: boolean) => Promise<void>;
  setLyricsChineseConversion: (conversion: ChineseConversion) => Promise<void>;
  setLyricsJapaneseRepairEnabled: (enabled: boolean) => Promise<void>;
  setNotchLyricsVisible: (visible: boolean) => Promise<void>;
  setLyricsDisplayPreferences: <Mode extends Exclude<LyricsStyleMode, "desktop">>(
    mode: Mode,
    preferences: LyricsDisplayPreferences[Mode],
  ) => Promise<void>;
  setLyricsBaseAppearance: (appearance: LyricsBaseAppearance) => Promise<void>;
  setLyricsStyleInheritance: (mode: LyricsStyleMode, inheritance: LyricsModeStyleInheritance) => Promise<void>;
  resetLyricsBaseAppearance: () => Promise<void>;
  syncConfig: (config: AppConfig) => void;
};

export function useConfigActions(
  config: AppConfig,
  loaded: boolean,
  resolvedTheme: "light" | "dark",
  setConfig: Dispatch<SetStateAction<AppConfig>>,
  configRef: MutableRefObject<AppConfig>,
  notchPreferencesWriteRef: MutableRefObject<NotchPreferencesWriteState>,
): ConfigActions {
  const setLyricsDisplayPreferences = useCallback(async <Mode extends keyof LyricsDisplayPreferences>(
    mode: Mode,
    preferences: LyricsDisplayPreferences[Mode],
  ) => {
    if (!isTauriRuntime()) {
      setConfig((current) => materializeLyricsStyleInheritance({
        ...current,
        lyrics: {
          ...current.lyrics,
          displays: { ...current.lyrics.displays, [mode]: preferences },
        },
      }));
      return;
    }

    if (mode !== "notch") {
      setConfig(await api.setLyricsDisplayPreferences(mode, preferences));
      return;
    }

    const writes = notchPreferencesWriteRef.current;
    const notchPreferences = preferences as LyricsDisplayPreferences["notch"];
    const version = writes.version + 1;
    writes.version = version;
    if (!writes.pending) {
      writes.confirmed = configRef.current.lyrics.displays.notch;
    }
    writes.pending = notchPreferences;
    setConfig((current) => applyPendingNotchPreferences(current, notchPreferences));

    const operation = writes.queue
      .catch(() => undefined)
      .then(async () => {
        try {
          const saved = await api.setLyricsDisplayPreferences("notch", notchPreferences);
          writes.confirmed = saved.lyrics.displays.notch;
          if (writes.version !== version) return;
          writes.pending = null;
          setConfig(saved);
        } catch (error) {
          if (writes.version === version) {
            writes.pending = null;
            try {
              const authoritative = await api.getAppConfig();
              writes.confirmed = authoritative.lyrics.displays.notch;
              setConfig(authoritative);
            } catch {
              const confirmed = writes.confirmed;
              if (confirmed) {
                setConfig((current) => applyPendingNotchPreferences(current, confirmed));
              }
            }
          }
          throw error;
        }
      });
    writes.queue = operation.then(() => undefined, () => undefined);
    return operation;
  }, []);

  return useMemo<ConfigActions>(() => ({
    setTheme: async (theme) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, app: { ...current.app, theme } }));
        return;
      }
      setConfig(await api.setTheme(theme));
    },
    setLanguage: async (language) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, app: { ...current.app, language } }));
        return;
      }
      setConfig(await api.setLanguage(language));
    },
    setGlobalShortcuts: async (shortcuts) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, app: { ...current.app, shortcuts } }));
        return;
      }
      setConfig(await api.setGlobalShortcuts(shortcuts));
    },
    setSystemMediaFilterMode: async (mode) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, app: { ...current.app, systemMediaFilterMode: mode } }));
        return;
      }
      setConfig(await api.setSystemMediaFilterMode(mode));
    },
    setSystemMediaApplications: async (applications) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, app: { ...current.app, systemMediaApplications: applications } }));
        return;
      }
      setConfig(await api.setSystemMediaApplications(applications));
    },
    setPlayerFollowerApplication: async (application) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, app: { ...current.app, playerFollowerApplication: application } }));
        return;
      }
      setConfig(await api.setPlayerFollowerApplication(application));
    },
    setDockIconHidden: async (hidden) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({
          ...current,
          app: { ...current.app, hideDockIcon: hidden },
        }));
        return;
      }
      setConfig(await api.setDockIconHidden(hidden));
    },
    setMenuBarIconHidden: async (hidden) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({
          ...current,
          app: { ...current.app, hideMenuBarIcon: hidden },
        }));
        return;
      }
      setConfig(await api.setMenuBarIconHidden(hidden));
    },
    setSilentStartup: async (enabled) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, app: { ...current.app, silentStartup: enabled } }));
        return;
      }
      setConfig(await api.setSilentStartup(enabled));
    },
    setAutoCheckUpdates: async (enabled) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, app: { ...current.app, autoCheckUpdates: enabled } }));
        return;
      }
      setConfig(await api.setAutoCheckUpdates(enabled));
    },
    setLyricsWindowsShowOnAllSpaces: async (enabled) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({
          ...current,
          app: { ...current.app, lyricsWindowsShowOnAllSpaces: enabled },
        }));
        return;
      }
      setConfig(await api.setLyricsWindowsShowOnAllSpaces(enabled));
    },
    setOverlayHideWhenNotPlaying: async (hidden) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({
          ...current,
          overlay: { ...current.overlay, hideWhenNotPlaying: hidden },
        }));
        return;
      }
      setConfig(await api.setOverlayHideWhenNotPlaying(hidden));
    },
    setStatusBarLyricsEnabled: async (enabled) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, lyrics: { ...current.lyrics, displays: { ...current.lyrics.displays, statusBar: { ...current.lyrics.displays.statusBar, enabled } } } }));
        return;
      }
      setConfig(await api.setStatusBarLyricsEnabled(enabled));
    },
    setListLyricsVisible: async (enabled) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, lyrics: { ...current.lyrics, displays: { ...current.lyrics.displays, listWindow: { ...current.lyrics.displays.listWindow, enabled } } } }));
        return;
      }
      setConfig(await api.setListLyricsVisible(enabled));
    },
    setListLyricsOptions: async (showTranslation, showRomanization) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, lyrics: { ...current.lyrics, displays: { ...current.lyrics.displays, listWindow: { ...current.lyrics.displays.listWindow, showTranslation, showRomanization } } } }));
        return;
      }
      setConfig(await api.setListLyricsOptions(showTranslation, showRomanization));
    },
    setListLyricsLocked: async (locked) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({
          ...current,
          lyrics: {
            ...current.lyrics,
            displays: {
              ...current.lyrics.displays,
              listWindow: { ...current.lyrics.displays.listWindow, locked },
            },
          },
        }));
        return;
      }
      setConfig(await api.setListLyricsLocked(locked));
    },
    setLyricsChineseConversion: async (conversion) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({
          ...current,
          lyrics: { ...current.lyrics, chineseConversion: conversion },
        }));
        return;
      }
      setConfig(await api.setLyricsChineseConversion(conversion));
    },
    setLyricsJapaneseRepairEnabled: async (enabled) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({
          ...current,
          lyrics: { ...current.lyrics, repairSimplifiedJapanese: enabled },
        }));
        return;
      }
      setConfig(await api.setLyricsJapaneseRepairEnabled(enabled));
    },
    setNotchLyricsVisible: async (enabled) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, lyrics: { ...current.lyrics, displays: { ...current.lyrics.displays, notch: { ...current.lyrics.displays.notch, enabled } } } }));
        return;
      }
      setConfig(await api.setNotchLyricsVisible(enabled));
    },
    setLyricsDisplayPreferences,
    setLyricsBaseAppearance: async (appearance) => {
      if (!isTauriRuntime()) {
        setConfig((current) => materializeLyricsStyleInheritance({
          ...current,
          lyrics: { ...current.lyrics, baseAppearance: appearance },
        }));
        return;
      }
      setConfig(await api.setLyricsBaseAppearance(appearance));
    },
    setLyricsStyleInheritance: async (mode, inheritance) => {
      if (!isTauriRuntime()) {
        setConfig((current) => materializeLyricsStyleInheritance({
          ...current,
          lyrics: {
            ...current.lyrics,
            styleInheritance: { ...current.lyrics.styleInheritance, [mode]: inheritance },
          },
        }));
        return;
      }
      setConfig(await api.setLyricsStyleInheritance(mode, inheritance));
    },
    resetLyricsBaseAppearance: async () => {
      if (!isTauriRuntime()) {
        setConfig((current) => materializeLyricsStyleInheritance({
          ...current,
          lyrics: { ...current.lyrics, baseAppearance: defaultLyricsBaseAppearance },
        }));
        return;
      }
      setConfig(await api.resetLyricsBaseAppearance());
    },
    syncConfig: setConfig,
  }), [config, loaded, resolvedTheme, setLyricsDisplayPreferences]);
}
