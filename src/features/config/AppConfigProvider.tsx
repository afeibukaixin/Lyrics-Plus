import { useContext, useEffect, useMemo, useRef, useState } from "react";

import { isTauriRuntime } from "../../shared/api";
import { useConfigActions } from "./provider/actions";
import {
  AppConfigContext,
  type AppConfigContextValue,
  type AppConfigWindowType,
} from "./provider/context";
import { defaultConfig } from "./provider/defaults";
import {
  useConfigSubscription,
  type NotchPreferencesWriteState,
} from "./provider/subscription";
import { useResolvedTheme } from "./provider/theme";

export function AppConfigProvider({
  children,
  windowType = "main",
}: {
  children: React.ReactNode;
  windowType?: AppConfigWindowType;
}) {
  const [config, setConfig] = useState(defaultConfig);
  const [loaded, setLoaded] = useState(!isTauriRuntime());
  const configRef = useRef(config);
  configRef.current = config;
  const notchPreferencesWriteRef = useRef<NotchPreferencesWriteState>({
    queue: Promise.resolve(),
    version: 0,
    pending: null,
    confirmed: null,
  });

  useConfigSubscription(windowType, setConfig, setLoaded, notchPreferencesWriteRef);
  // Keep the UI-only font override on each WebView root; null restores the stylesheet default.
  useEffect(() => {
    const root = document.documentElement;
    if (config.app.uiFontFamily === null) {
      root.style.removeProperty("--font-family-sans");
    } else {
      root.style.setProperty("--font-family-sans", config.app.uiFontFamily);
    }
    return () => {
      root.style.removeProperty("--font-family-sans");
    };
  }, [config.app.uiFontFamily]);
  const resolvedTheme = useResolvedTheme(config.app.theme);
  const actions = useConfigActions(
    config,
    loaded,
    resolvedTheme,
    setConfig,
    configRef,
    notchPreferencesWriteRef,
  );

  const value = useMemo<AppConfigContextValue>(() => ({
    config,
    loaded,
    resolvedTheme,
    ...actions,
  }), [actions, config, loaded, resolvedTheme]);

  return <AppConfigContext.Provider value={value}>{children}</AppConfigContext.Provider>;
}

export function useAppConfig() {
  const value = useContext(AppConfigContext);
  if (!value) throw new Error("useAppConfig must be used within AppConfigProvider");
  return value;
}
