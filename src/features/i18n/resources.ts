import type { SupportedLanguage } from "../../shared/types";
import { enUS } from "./locales/en-US";
import { jaJP } from "./locales/ja-JP";
import { koKR } from "./locales/ko-KR";
import { zhCN } from "./locales/zh-CN";
import { zhHK } from "./locales/zh-HK";
import { zhTW } from "./locales/zh-TW";
import type { AppTranslationResource } from "./resourceTypes";

export type { AppTranslationResource } from "./resourceTypes";

export const translationResources: Record<SupportedLanguage, { translation: AppTranslationResource }> = {
  "zh-CN": { translation: zhCN },
  "zh-HK": { translation: zhHK },
  "zh-TW": { translation: zhTW },
  "en-US": { translation: enUS },
  "ja-JP": { translation: jaJP },
  "ko-KR": { translation: koKR },
};
