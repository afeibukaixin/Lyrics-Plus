import { createContext, useContext, useEffect, useMemo, useState } from "react";
import { I18nextProvider } from "react-i18next";
import { useAppConfig } from "../config/AppConfigProvider";
import type { LanguagePreference, SupportedLanguage } from "../../shared/types";
import { appI18n, detectSystemLanguage, nativeLanguageFor, normalizeLanguagePreference, resolveLanguage } from "./i18n";
import { api, isTauriRuntime } from "../../shared/api";

type LanguageContextValue = {
  language: SupportedLanguage;
  preference: LanguagePreference;
  setLanguage: (preference: LanguagePreference) => Promise<void>;
};

const LanguageContext = createContext<LanguageContextValue | null>(null);

export function AppI18nProvider({ children }: { children: React.ReactNode }) {
  const { config, setLanguage } = useAppConfig();
  const [systemLanguage, setSystemLanguage] = useState(detectSystemLanguage);
  const preference = normalizeLanguagePreference(config.app.language);
  const language = resolveLanguage(preference, systemLanguage);

  useEffect(() => {
    const handleLanguageChange = () => setSystemLanguage(detectSystemLanguage());
    window.addEventListener("languagechange", handleLanguageChange);
    return () => window.removeEventListener("languagechange", handleLanguageChange);
  }, []);

  useEffect(() => {
    document.documentElement.lang = language;
    const view = new URLSearchParams(window.location.search).get("view");
    const titleKey = view === "quick-lyrics"
      ? "window.quickLyrics"
      : view === "overlay"
        ? "window.overlay"
        : view === "unlock-handle"
          ? "window.unlockHandle"
          : "window.main";
    void appI18n.changeLanguage(language).then(() => {
      document.title = appI18n.t(titleKey);
    });
    if (isTauriRuntime()) void api.setNativeLanguage(nativeLanguageFor(language)).catch(() => undefined);
  }, [language]);

  const value = useMemo<LanguageContextValue>(() => ({
    language,
    preference,
    setLanguage,
  }), [language, preference, setLanguage]);

  return (
    <LanguageContext.Provider value={value}>
      <I18nextProvider i18n={appI18n}>{children}</I18nextProvider>
    </LanguageContext.Provider>
  );
}

export function useAppLanguage() {
  const value = useContext(LanguageContext);
  if (!value) throw new Error("useAppLanguage must be used inside AppI18nProvider");
  return value;
}
