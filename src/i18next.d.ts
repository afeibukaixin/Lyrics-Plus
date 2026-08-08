import "i18next";
import type { AppTranslationResource } from "./features/i18n/resources";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    resources: {
      translation: AppTranslationResource;
    };
  }
}

