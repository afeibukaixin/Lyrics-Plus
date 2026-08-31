import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauriRuntime } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import {
  defaultGlobalShortcuts,
  defaultLyricsBaseAppearance,
  defaultLyricsStyleInheritance,
  defaultListLyricsAppearance,
  defaultNotchLyricsAppearance,
  defaultOverlayStyle,
  defaultStatusBarLyricsAppearance,
  type AppConfig,
  type ChineseConversion,
  type GlobalShortcutSettings,
  type LanguagePreference,
  type LyricsDisplayPreferences,
  type LyricsBaseAppearance,
  type LyricsModeStyleInheritance,
  type LyricsStyleMode,
  type RegisteredApplication,
  type SystemMediaFilterMode,
  type ThemePreference,
} from "../../shared/types";

const defaultOverlayAppearance = (({
  horizontalMaxWidth: _horizontalMaxWidth,
  verticalMaxHeight: _verticalMaxHeight,
  ...appearance
}: typeof defaultOverlayStyle) => appearance)(defaultOverlayStyle);

const defaultTitleFilterKeywords = [
  "feat", "ft", "featuring", "主题曲", "片头曲", "片尾曲",
  "插曲", "电影", "电视剧", "动画", "游戏", "ost",
];

function materializeLyricsStyleInheritance(config: AppConfig): AppConfig {
  const base = config.lyrics.baseAppearance;
  const inheritance = config.lyrics.styleInheritance;
  const next: AppConfig = {
    ...config,
    lyrics: {
      ...config.lyrics,
      displays: {
        statusBar: { ...config.lyrics.displays.statusBar, appearance: { ...config.lyrics.displays.statusBar.appearance } },
        listWindow: { ...config.lyrics.displays.listWindow, appearance: { ...config.lyrics.displays.listWindow.appearance } },
        notch: { ...config.lyrics.displays.notch, appearance: { ...config.lyrics.displays.notch.appearance } },
      },
    },
    overlay: { ...config.overlay, appearance: { ...config.overlay.appearance } },
  };
  if (inheritance.desktop.inheritFontFamily) next.overlay.appearance.fontFamily = base.fontFamily;
  if (inheritance.desktop.inheritColors) Object.assign(next.overlay.appearance, {
    activeColor: base.activeColor,
    inactiveColor: base.inactiveColor,
    translationColor: base.translationColor,
    romanizationColor: base.romanizationColor,
    solidColor: base.backgroundColor,
  });
  if (inheritance.statusBar.inheritFontFamily) next.lyrics.displays.statusBar.appearance.fontFamily = base.fontFamily;
  if (inheritance.statusBar.inheritColors) Object.assign(next.lyrics.displays.statusBar.appearance, {
    textColor: base.activeColor,
    inactiveColor: base.inactiveColor,
    highlightColor: base.activeColor,
  });
  if (inheritance.listWindow.inheritFontFamily) next.lyrics.displays.listWindow.appearance.fontFamily = base.fontFamily;
  if (inheritance.listWindow.inheritColors) Object.assign(next.lyrics.displays.listWindow.appearance, {
    activeColor: base.activeColor,
    inactiveColor: base.inactiveColor,
    translationColor: base.translationColor,
    romanizationColor: base.romanizationColor,
  });
  if (inheritance.notch.inheritFontFamily) next.lyrics.displays.notch.appearance.fontFamily = base.fontFamily;
  if (inheritance.notch.inheritColors) Object.assign(next.lyrics.displays.notch.appearance, {
    activeColor: base.activeColor,
    inactiveColor: base.inactiveColor,
    translationColor: base.translationColor,
    romanizationColor: base.romanizationColor,
    backgroundColor: base.backgroundColor,
  });
  return next;
}

function applyPendingNotchPreferences(
  config: AppConfig,
  pending: LyricsDisplayPreferences["notch"] | null,
): AppConfig {
  if (!pending) return config;
  return {
    ...config,
    lyrics: {
      ...config.lyrics,
      displays: { ...config.lyrics.displays, notch: pending },
    },
  };
}

const defaultConfig: AppConfig = {
  schemaVersion: 57,
  app: { theme: "dark", language: "system", playerSelection: "auto", systemMediaFilterMode: "allowlist", systemMediaApplications: [], playerFollowerApplication: null, hideDockIcon: false, hideMenuBarIcon: false, silentStartup: false, autoCheckUpdates: true, lyricsWindowsShowOnAllSpaces: false, shortcuts: defaultGlobalShortcuts },
  lyrics: {
    chineseConversion: "original",
    providers: {
      mode: "smart",
      autoApplyThreshold: 60,
      preferCapabilities: true,
      matchWeights: { title: 39, artist: 36, album: 8, duration: 17 },
      normalizeChinese: true,
      titleFilterKeywords: defaultTitleFilterKeywords,
      amllBaseUrl: "https://amlldb.bikonoo.com",
      providers: [
        { id: "lrclib", enabled: true },
        { id: "kugou", enabled: true },
        { id: "qqmusic", enabled: true },
        { id: "netease", enabled: true },
        { id: "kuwo", enabled: true },
        { id: "amll_ttml", enabled: true },
        { id: "migu", enabled: true },
        { id: "musixmatch", enabled: true },
      ],
    },
    displays: {
      statusBar: { enabled: false, hideWhenNotPlaying: false, appearance: defaultStatusBarLyricsAppearance },
      listWindow: { enabled: false, alwaysOnTop: false, showTranslation: true, showRomanization: false, appearance: defaultListLyricsAppearance },
      notch: {
        enabled: false,
        hideWhenNotPlaying: false,
        monitorId: null,
        showLyrics: false,
        leftSlot: "artwork",
        rightSlot: "spectrum",
        layout: "single",
        doubleLineMode: "rolling",
        showTranslation: false,
        showRomanization: false,
        appearance: defaultNotchLyricsAppearance,
      },
    },
    baseAppearance: defaultLyricsBaseAppearance,
    styleInheritance: defaultLyricsStyleInheritance,
  },
  overlay: {
    visible: true,
    locked: false,
    hideWhenNotPlaying: false,
    appearance: defaultOverlayAppearance,
  },
};

type AppConfigContextValue = {
  config: AppConfig;
  resolvedTheme: "light" | "dark";
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
  setLyricsChineseConversion: (conversion: ChineseConversion) => Promise<void>;
  setNotchLyricsVisible: (visible: boolean) => Promise<void>;
  setLyricsDisplayPreferences: <Mode extends Exclude<LyricsStyleMode, "desktop">>(mode: Mode, preferences: LyricsDisplayPreferences[Mode]) => Promise<void>;
  setLyricsBaseAppearance: (appearance: LyricsBaseAppearance) => Promise<void>;
  setLyricsStyleInheritance: (mode: LyricsStyleMode, inheritance: LyricsModeStyleInheritance) => Promise<void>;
  resetLyricsBaseAppearance: () => Promise<void>;
  loaded: boolean;
  syncConfig: (config: AppConfig) => void;
};

const AppConfigContext = createContext<AppConfigContextValue | null>(null);

export function AppConfigProvider({
  children,
  windowType = "main",
}: {
  children: React.ReactNode;
  windowType?: "main" | "quick-lyrics" | "overlay" | "unlock-handle" | "lyrics-status-bar" | "lyrics-list" | "lyrics-notch";
}) {
  const [config, setConfig] = useState(defaultConfig);
  const [loaded, setLoaded] = useState(!isTauriRuntime());
  const [resolvedTheme, setResolvedTheme] = useState<"light" | "dark">("dark");
  const configRef = useRef(config);
  configRef.current = config;
  const notchPreferencesWriteRef = useRef({
    queue: Promise.resolve() as Promise<void>,
    version: 0,
    pending: null as LyricsDisplayPreferences["notch"] | null,
    confirmed: null as LyricsDisplayPreferences["notch"] | null,
  });

  useEffect(() => {
    document.documentElement.dataset.window = windowType;
    if (!isTauriRuntime()) return;
    void api.getAppConfig().then((value) => {
      setConfig(applyPendingNotchPreferences(value, notchPreferencesWriteRef.current.pending));
      setLoaded(true);
    }).catch(() => setLoaded(false));
    return createTauriListenerCleanup(
      listen<AppConfig>("config://changed", ({ payload }) => {
        setConfig(applyPendingNotchPreferences(
          payload,
          notchPreferencesWriteRef.current.pending,
        ));
      }),
    );
  }, [windowType]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const resolved = config.app.theme === "system"
        ? (media.matches ? "dark" : "light")
        : config.app.theme;
      document.documentElement.dataset.theme = config.app.theme;
      document.documentElement.dataset.resolvedTheme = resolved;
      document.documentElement.classList.toggle("light", resolved === "light");
      document.documentElement.classList.toggle("dark", resolved === "dark");
      document.documentElement.style.colorScheme = resolved;
      setResolvedTheme(resolved);
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [config.app.theme]);

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

  const value = useMemo<AppConfigContextValue>(() => ({
    config,
    loaded,
    resolvedTheme,
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

  return <AppConfigContext.Provider value={value}>{children}</AppConfigContext.Provider>;
}

export function useAppConfig() {
  const value = useContext(AppConfigContext);
  if (!value) throw new Error("useAppConfig must be used within AppConfigProvider");
  return value;
}
