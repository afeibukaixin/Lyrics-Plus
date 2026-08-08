import {
  languageRegistry,
  matchSupportedLanguage,
  supportedLanguages,
  type SupportedLanguage,
} from "../../shared/languages";
import type { AppTranslationResource } from "./resourceTypes";

type LanguageLabel = Exclude<keyof AppTranslationResource["common"]["language"], "system"> & string;

type LocalizedLanguageDefinition = {
  labelKey: `common.language.${LanguageLabel}`;
  matches: (language: string) => boolean;
};

const localizedLanguageRegistry: Record<SupportedLanguage, LocalizedLanguageDefinition> =
  languageRegistry;

export const languageOptions = supportedLanguages.map((code) => ({
  code,
  labelKey: localizedLanguageRegistry[code].labelKey,
}));

export { matchSupportedLanguage, supportedLanguages };
