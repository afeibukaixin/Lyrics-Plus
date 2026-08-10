import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { useTranslation } from "react-i18next";
import { useAppConfig } from "../config/AppConfigProvider";
import { isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";

export type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "installing" | "latest" | "error";

type UpdateContextValue = {
  currentVersion: string;
  availableVersion: string | null;
  error: string | null;
  status: UpdateStatus;
  checkForUpdates: () => Promise<void>;
};

const UpdateContext = createContext<UpdateContextValue | null>(null);

export function UpdateProvider({ children }: { children: React.ReactNode }) {
  const { config, loaded } = useAppConfig();
  const { t } = useTranslation();
  const [currentVersion, setCurrentVersion] = useState("—");
  const [availableVersion, setAvailableVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const busy = useRef(false);
  const autoChecked = useRef(false);
  const canUpdate = import.meta.env.PROD && isTauriRuntime();

  useEffect(() => {
    if (!isTauriRuntime()) return;
    void getVersion().then(setCurrentVersion).catch((value) => {
      reportFrontendError("Failed to read app version", value);
    });
  }, []);

  const runCheck = useCallback(async (manual: boolean) => {
    if (busy.current) return;
    if (!canUpdate) {
      if (manual) {
        setStatus("error");
        setError(t("settings.about.installedOnly"));
      }
      return;
    }

    busy.current = true;
    let installationStarted = false;
    setError(null);
    setAvailableVersion(null);
    setStatus("checking");
    try {
      const update = await check({ timeout: 15_000 });
      if (!update) {
        setStatus("latest");
        return;
      }

      setAvailableVersion(update.version);
      setStatus("available");
      const accepted = await ask(
        t("settings.about.updatePrompt", {
          version: update.version,
          notes: update.body ? `\n\n${update.body}` : "",
        }),
        {
          title: t("settings.about.updateAvailable"),
          kind: "info",
          okLabel: t("settings.about.installNow"),
          cancelLabel: t("common.actions.cancel"),
        },
      );
      if (!accepted) {
        await update.close();
        return;
      }

      setStatus("downloading");
      installationStarted = true;
      await update.downloadAndInstall((event) => {
        if (event.event === "Finished") setStatus("installing");
      });
      await relaunch();
    } catch (value) {
      reportFrontendError("Update check or installation failed", value);
      const visible = manual || installationStarted;
      setStatus(visible ? "error" : "idle");
      if (visible) setError(t("settings.about.updateError"));
      if (!manual && installationStarted) {
        void message(t("settings.about.updateError"), {
          title: t("settings.about.updateFailed"),
          kind: "error",
        }).catch((dialogError) => reportFrontendError("Failed to show update error", dialogError));
      }
    } finally {
      busy.current = false;
    }
  }, [canUpdate, t]);

  useEffect(() => {
    if (!loaded || autoChecked.current || !config.app.autoCheckUpdates) return;
    autoChecked.current = true;
    void runCheck(false);
  }, [config.app.autoCheckUpdates, loaded, runCheck]);

  const value = useMemo<UpdateContextValue>(() => ({
    currentVersion,
    availableVersion,
    error,
    status,
    checkForUpdates: () => runCheck(true),
  }), [availableVersion, currentVersion, error, runCheck, status]);

  return <UpdateContext.Provider value={value}>{children}</UpdateContext.Provider>;
}

export function useUpdates() {
  const value = useContext(UpdateContext);
  if (!value) throw new Error("useUpdates must be used within UpdateProvider");
  return value;
}
