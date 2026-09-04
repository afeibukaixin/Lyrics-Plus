import { createContext } from "react";
import type { AppConfig } from "../../../shared/types";

import type { ConfigActions } from "./actions";

export type AppConfigWindowType =
  | "main"
  | "quick-lyrics"
  | "overlay"
  | "unlock-handle"
  | "lyrics-list-unlock-handle"
  | "lyrics-status-bar"
  | "lyrics-list"
  | "lyrics-notch";

export type AppConfigContextValue = ConfigActions & {
  config: AppConfig;
  resolvedTheme: "light" | "dark";
  loaded: boolean;
};

export const AppConfigContext = createContext<AppConfigContextValue | null>(null);
