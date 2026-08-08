export const languageRegistry = {
  "zh-CN": {
    labelKey: "common.language.zhCN",
    matches: (language: string) => language === "zh" || language.startsWith("zh-"),
  },
  "en-US": {
    labelKey: "common.language.enUS",
    matches: (language: string) => language === "en" || language.startsWith("en-"),
  },
} as const;

export type SupportedLanguage = keyof typeof languageRegistry;

export const supportedLanguages = Object.keys(languageRegistry) as SupportedLanguage[];

export function matchSupportedLanguage(language: string): SupportedLanguage | null {
  const normalized = language.toLowerCase();
  return supportedLanguages.find((code) => languageRegistry[code].matches(normalized)) ?? null;
}
