import { createInstance } from "i18next";
import { initReactI18next } from "react-i18next";
import type { LanguagePreference, SupportedLanguage } from "../../shared/types";
import { matchSupportedLanguage, supportedLanguages } from "./languages";
import { translationResources } from "./resources";

export const DEFAULT_LANGUAGE: SupportedLanguage = "zh-CN";

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
  preference: LanguagePreference,
  systemLanguage = detectSystemLanguage(),
): SupportedLanguage {
  return preference === "system" ? systemLanguage : preference;
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
