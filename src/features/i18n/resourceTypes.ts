import type { zhCN } from "./locales/zh-CN";

type TranslationShape<Value> = {
  [Key in keyof Value]: Value[Key] extends string ? string : TranslationShape<Value[Key]>;
};

export type AppTranslationResource = TranslationShape<typeof zhCN>;

