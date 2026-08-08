import {
  languageRegistry,
  matchSupportedLanguage,
  supportedLanguages,
} from "../../shared/languages";

export const languageOptions = supportedLanguages.map((code) => ({
  code,
  label: languageRegistry[code].nativeLabel,
}));

export { matchSupportedLanguage, supportedLanguages };
