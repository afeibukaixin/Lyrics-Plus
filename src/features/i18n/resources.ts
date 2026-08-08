import type { SupportedLanguage } from "../../shared/types";
import { enUS } from "./locales/en-US";
import { zhCN } from "./locales/zh-CN";
import { zhTW } from "./locales/zh-TW";
import type { AppTranslationResource } from "./resourceTypes";

export type { AppTranslationResource } from "./resourceTypes";

export const translationResources: Record<SupportedLanguage, { translation: AppTranslationResource }> = {
  "zh-CN": { translation: zhCN },
  "zh-TW": { translation: zhTW },
  "en-US": { translation: enUS },
};
