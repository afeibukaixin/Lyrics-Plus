import { useEffect, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import { listen } from "@tauri-apps/api/event";

import { api, isTauriRuntime } from "../../../shared/api";
import type { AppConfig, LyricsDisplayPreferences } from "../../../shared/types";
import { createTauriListenerCleanup } from "../../../shared/tauriEvent";

import { applyPendingNotchPreferences } from "./inheritance";
import type { AppConfigWindowType } from "./context";

export type NotchPreferencesWriteState = {
  queue: Promise<void>;
  version: number;
  pending: LyricsDisplayPreferences["notch"] | null;
  confirmed: LyricsDisplayPreferences["notch"] | null;
};

export function useConfigSubscription(
  windowType: AppConfigWindowType,
  setConfig: Dispatch<SetStateAction<AppConfig>>,
  setLoaded: Dispatch<SetStateAction<boolean>>,
  notchPreferencesWriteRef: MutableRefObject<NotchPreferencesWriteState>,
) {
  useEffect(() => {
    document.documentElement.dataset.window = windowType;
    if (!isTauriRuntime()) return;
    void api.getAppConfig().then((value) => {
      setConfig(applyPendingNotchPreferences(value, notchPreferencesWriteRef.current.pending));
      setLoaded(true);
    }).catch(() => setLoaded(false));
    return createTauriListenerCleanup(
      listen<AppConfig>("config://changed", ({ payload }) => {
        setConfig(applyPendingNotchPreferences(
          payload,
          notchPreferencesWriteRef.current.pending,
        ));
      }),
    );
  }, [windowType]);
}
