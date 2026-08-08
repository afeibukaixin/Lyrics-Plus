export const languageRegistry = {
  "zh-CN": {
    nativeLabel: "简体中文",
    matches: (language: string) => language === "zh"
      || language === "zh-cn"
      || language.startsWith("zh-cn-")
      || language === "zh-sg"
      || language.startsWith("zh-sg-")
      || language.startsWith("zh-hans"),
  },
  "zh-TW": {
    nativeLabel: "繁體中文",
    matches: (language: string) => language === "zh-tw"
      || language.startsWith("zh-tw-")
      || language === "zh-hk"
      || language.startsWith("zh-hk-")
      || language === "zh-mo"
      || language.startsWith("zh-mo-")
      || language.startsWith("zh-hant"),
  },
  "en-US": {
    nativeLabel: "English",
    matches: (language: string) => language === "en" || language.startsWith("en-"),
  },
} as const;

export type SupportedLanguage = keyof typeof languageRegistry;

export const supportedLanguages = Object.keys(languageRegistry) as SupportedLanguage[];

export function matchSupportedLanguage(language: string): SupportedLanguage | null {
  const normalized = language.trim().replace(/_/g, "-").toLowerCase();
  return supportedLanguages.find((code) => languageRegistry[code].matches(normalized)) ?? null;
}
