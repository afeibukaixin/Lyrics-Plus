import { createInstance } from "i18next";
import { initReactI18next } from "react-i18next";
import type { LanguagePreference, NativeLanguage, SupportedLanguage } from "../../shared/types";
import { matchSupportedLanguage, supportedLanguages } from "./languages";
import { translationResources } from "./resources";

export const DEFAULT_LANGUAGE: SupportedLanguage = "en-US";

export function detectSystemLanguage(languages?: readonly string[]): SupportedLanguage {
  const candidates = languages
    ?? (typeof navigator === "undefined"
      ? []
      : [...navigator.languages, navigator.language]);
  for (const candidate of candidates) {
    const supported = matchSupportedLanguage(candidate);
    if (supported) return supported;
  }
  return DEFAULT_LANGUAGE;
}

export function resolveLanguage(
  preference: string,
  systemLanguage = detectSystemLanguage(),
): SupportedLanguage {
  if (preference === "system") return systemLanguage;
  return matchSupportedLanguage(preference) ?? DEFAULT_LANGUAGE;
}

export function normalizeLanguagePreference(preference: string): LanguagePreference {
  if (preference === "system") return "system";
  return matchSupportedLanguage(preference) ?? DEFAULT_LANGUAGE;
}

export function nativeLanguageFor(language: SupportedLanguage): NativeLanguage {
  return language === "zh-CN" ? "zh-CN" : "en-US";
}

export const appI18n = createInstance();

void appI18n.use(initReactI18next).init({
  resources: translationResources,
  lng: detectSystemLanguage(),
  supportedLngs: supportedLanguages,
  fallbackLng: DEFAULT_LANGUAGE,
  load: "currentOnly",
  initAsync: false,
  returnEmptyString: false,
  interpolation: { escapeValue: false },
  parseMissingKeyHandler: (key) => `[${key}]`,
  react: { useSuspense: false },
});
