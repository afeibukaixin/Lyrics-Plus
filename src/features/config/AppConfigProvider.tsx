import { createContext, useContext, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, isTauriRuntime } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import { defaultGlobalShortcuts, defaultOverlayStyle, type AppConfig, type GlobalShortcutSettings } from "../../shared/types";

const defaultConfig: AppConfig = {
  schemaVersion: 9,
  app: { uiFontScale: 100, playerSelection: "auto", hideDockIcon: false, shortcuts: defaultGlobalShortcuts },
  lyrics: {
    providers: {
      mode: "smart",
      autoApplyThreshold: 60,
      providers: [
        { id: "lrclib", enabled: true },
        { id: "kugou", enabled: true },
        { id: "qqmusic", enabled: true },
        { id: "netease", enabled: true },
      ],
    },
  },
  overlay: {
    visible: true,
    locked: false,
    appearance: {
      fontSize: defaultOverlayStyle.fontSize,
      activeColor: defaultOverlayStyle.activeColor,
      inactiveColor: defaultOverlayStyle.inactiveColor,
      opacity: defaultOverlayStyle.opacity,
      backgroundOpacity: defaultOverlayStyle.backgroundOpacity,
      background: defaultOverlayStyle.background,
      solidColor: defaultOverlayStyle.solidColor,
      layout: defaultOverlayStyle.layout,
      orientation: defaultOverlayStyle.orientation,
      alignment: defaultOverlayStyle.alignment,
      longText: defaultOverlayStyle.longText,
      secondaryDisplay: defaultOverlayStyle.secondaryDisplay,
      autoCenterWithTranslationOrRomanization: defaultOverlayStyle.autoCenterWithTranslationOrRomanization,
      karaokeStyle: defaultOverlayStyle.karaokeStyle,
      secondaryFontScale: defaultOverlayStyle.secondaryFontScale,
      translationFontScale: defaultOverlayStyle.translationFontScale,
      romanizationFontScale: defaultOverlayStyle.romanizationFontScale,
      translationColor: defaultOverlayStyle.translationColor,
      romanizationColor: defaultOverlayStyle.romanizationColor,
    },
  },
};

type AppConfigContextValue = {
  config: AppConfig;
  setUiFontScale: (scale: number) => Promise<void>;
  setGlobalShortcuts: (shortcuts: GlobalShortcutSettings) => Promise<void>;
  setDockIconHidden: (hidden: boolean) => Promise<void>;
  syncConfig: (config: AppConfig) => void;
};

const AppConfigContext = createContext<AppConfigContextValue | null>(null);

export function AppConfigProvider({
  children,
  windowType = "main",
}: {
  children: React.ReactNode;
  windowType?: "main" | "quick-lyrics";
}) {
  const [config, setConfig] = useState(defaultConfig);

  useEffect(() => {
    document.documentElement.dataset.window = windowType;
    if (!isTauriRuntime()) return;
    void api.getAppConfig().then(setConfig);
    return createTauriListenerCleanup(
      listen<AppConfig>("config://changed", ({ payload }) => setConfig(payload)),
    );
  }, [windowType]);

  useEffect(() => {
    document.documentElement.style.fontSize = `${19.2 * config.app.uiFontScale / 100}px`;
  }, [config.app.uiFontScale]);

  const value = useMemo<AppConfigContextValue>(() => ({
    config,
    setUiFontScale: async (scale) => {
      if (!isTauriRuntime()) {
        setConfig((current) => ({ ...current, app: { ...current.app, uiFontScale: scale } }));
        return;
      }
      setConfig(await api.setUiFontScale(scale));
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
    syncConfig: setConfig,
  }), [config]);

  return <AppConfigContext.Provider value={value}>{children}</AppConfigContext.Provider>;
}

export function useAppConfig() {
  const value = useContext(AppConfigContext);
  if (!value) throw new Error("useAppConfig 必须在 AppConfigProvider 内使用");
  return value;
}
