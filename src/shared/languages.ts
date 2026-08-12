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
  "zh-HK": {
    nativeLabel: "繁體中文（香港）",
    matches: (language: string) => language === "zh-hk"
      || language.startsWith("zh-hk-")
      || language === "zh-hant-hk"
      || language.startsWith("zh-hant-hk-")
      || language === "zh-mo"
      || language.startsWith("zh-mo-")
      || language === "zh-hant-mo"
      || language.startsWith("zh-hant-mo-"),
  },
  "zh-TW": {
    nativeLabel: "繁體中文（台灣）",
    matches: (language: string) => language === "zh-tw"
      || language.startsWith("zh-tw-")
      || language.startsWith("zh-hant"),
  },
  "en-US": {
    nativeLabel: "English",
    matches: (language: string) => language === "en" || language.startsWith("en-"),
  },
} as const;

export type SupportedLanguage = keyof typeof languageRegistry;
export type ItunesCountry = "CN" | "TW" | "HK" | "US";

export const supportedLanguages = Object.keys(languageRegistry) as SupportedLanguage[];

export function matchSupportedLanguage(language: string): SupportedLanguage | null {
  const normalized = language.trim().replace(/_/g, "-").toLowerCase();
  return supportedLanguages.find((code) => languageRegistry[code].matches(normalized)) ?? null;
}

export function itunesCountryForLanguage(language: SupportedLanguage): ItunesCountry {
  return language === "zh-CN" ? "CN" : language === "zh-TW" ? "TW" : language === "zh-HK" ? "HK" : "US";
}
