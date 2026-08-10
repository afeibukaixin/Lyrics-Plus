import { createContext, useContext, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauriRuntime } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import { defaultGlobalShortcuts, defaultOverlayStyle, type AppConfig, type GlobalShortcutSettings, type LanguagePreference } from "../../shared/types";

const defaultOverlayAppearance = (({
  horizontalMaxWidth: _horizontalMaxWidth,
  verticalMaxHeight: _verticalMaxHeight,
  ...appearance
}: typeof defaultOverlayStyle) => appearance)(defaultOverlayStyle);

const defaultConfig: AppConfig = {
  schemaVersion: 15,
  app: { uiFontScale: 100, language: "system", playerSelection: "auto", hideDockIcon: false, autoCheckUpdates: true, shortcuts: defaultGlobalShortcuts },
  lyrics: {
    providers: {
      mode: "smart",
      autoApplyThreshold: 60,
      providers: [
        { id: "lrclib", enabled: true },
        { id: "kugou", enabled: false },
        { id: "qqmusic", enabled: false },
        { id: "netease", enabled: false },
      ],
    },
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
  setUiFontScale: (scale: number) => Promise<void>;
  setLanguage: (language: LanguagePreference) => Promise<void>;
  setGlobalShortcuts: (shortcuts: GlobalShortcutSettings) => Promise<void>;
  setDockIconHidden: (hidden: boolean) => Promise<void>;
  setAutoCheckUpdates: (enabled: boolean) => Promise<void>;
  setOverlayHideWhenNotPlaying: (hidden: boolean) => Promise<void>;
  loaded: boolean;
  syncConfig: (config: AppConfig) => void;
};

const AppConfigContext = createContext<AppConfigContextValue | null>(null);

export function AppConfigProvider({
  children,
  windowType = "main",
}: {
  children: React.ReactNode;
  windowType?: "main" | "quick-lyrics" | "overlay" | "unlock-handle";
}) {
  const [config, setConfig] = useState(defaultConfig);
  const [loaded, setLoaded] = useState(!isTauriRuntime());

  useEffect(() => {
    document.documentElement.dataset.window = windowType;
    if (!isTauriRuntime()) return;
    void api.getAppConfig().then((value) => {
      setConfig(value);
      setLoaded(true);
    }).catch(() => setLoaded(false));
    return createTauriListenerCleanup(
      listen<AppConfig>("config://changed", ({ payload }) => setConfig(payload)),
    );
  }, [windowType]);

  useEffect(() => {
    document.documentElement.style.fontSize = `${19.2 * config.app.uiFontScale / 100}px`;
  }, [config.app.uiFontScale]);

  const value = useMemo<AppConfigContextValue>(() => ({
    config,
    loaded,
    setUiFontScale: async (scale) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, app: { ...current.app, uiFontScale: scale } }));
        return;
      }
      setConfig(await api.setUiFontScale(scale));
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
    setAutoCheckUpdates: async (enabled) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, app: { ...current.app, autoCheckUpdates: enabled } }));
        return;
      }
      setConfig(await api.setAutoCheckUpdates(enabled));
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
    syncConfig: setConfig,
  }), [config, loaded]);

  return <AppConfigContext.Provider value={value}>{children}</AppConfigContext.Provider>;
}

export function useAppConfig() {
  const value = useContext(AppConfigContext);
  if (!value) throw new Error("useAppConfig must be used within AppConfigProvider");
  return value;
}
