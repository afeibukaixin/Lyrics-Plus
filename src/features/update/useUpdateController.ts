import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";
import type { Update } from "@tauri-apps/plugin-updater";
import { toast } from "sonner";
import { api, isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import { useAppConfig } from "../config/AppConfigProvider";
import { useAppLanguage } from "../i18n/I18nProvider";
import {
  checkForUpdate,
  closeUpdate,
  downloadAndInstall,
  relaunchApplication,
  readCurrentVersion,
  uiUpdatePreviewReleaseNotes,
  updatePreview,
  updatePreviewMode,
  updatePreviewReleaseNotes,
  waitForPreviewStep,
} from "./updateService";

export type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "installing" | "ready" | "latest" | "error";
export type UpdateKind = "application" | "interface" | null;

export type UpdateContextValue = {
  currentVersion: string;
  availableVersion: string | null;
  error: string | null;
  status: UpdateStatus;
  updateKind: UpdateKind;
  progressPercentage: number | null;
  checkForUpdates: () => Promise<void>;
  openUpdateDialog: () => void;
  restartToUpdate: () => Promise<void>;
};

export type UpdateDialogProps = {
  open: boolean;
  currentVersion: string;
  availableVersion: string | null;
  releaseNotes: string;
  downloadedBytes: number;
  totalBytes: number | null;
  error: string | null;
  status: UpdateStatus;
  updateKind: UpdateKind;
  progressPercentage: number | null;
  language: string;
  t: TFunction;
  openUpdateDialog: () => void;
  dismissDialog: () => void;
  installUpdate: () => Promise<void>;
  restartToUpdate: () => Promise<void>;
  retryUpdate: () => Promise<void>;
};

export type UpdateController = {
  value: UpdateContextValue;
  dialog: UpdateDialogProps;
};

export function useUpdateController(): UpdateController {
  const { config, loaded } = useAppConfig();
  const { language } = useAppLanguage();
  const { t } = useTranslation();
  const updateRef = useRef<Update | null>(null);
  const busy = useRef(false);
  const autoChecked = useRef(false);
  const dialogOpenRef = useRef(false);
  const mountedRef = useRef(true);
  const [currentVersion, setCurrentVersion] = useState("—");
  const [availableVersion, setAvailableVersion] = useState<string | null>(null);
  const [releaseNotes, setReleaseNotes] = useState("");
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [totalBytes, setTotalBytes] = useState<number | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [updateKind, setUpdateKind] = useState<UpdateKind>(null);
  const canUpdate = import.meta.env.PROD && isTauriRuntime();

  useEffect(() => {
    if (updatePreview || !isTauriRuntime()) return;
    void readCurrentVersion().then(setCurrentVersion).catch((value) => {
      reportFrontendError("Failed to read app version", value);
    });
  }, []);

  useEffect(() => {
    if (!updatePreview) return;
    const isUiPreview = updatePreviewMode === "ui-ready";
    setCurrentVersion(isUiPreview ? "2.2.10" : "2.0.0");
    setAvailableVersion(isUiPreview ? "2.2.10-ui.1" : "2.1.0");
    setUpdateKind(isUiPreview ? "interface" : "application");
    setReleaseNotes(isUiPreview ? uiUpdatePreviewReleaseNotes : updatePreviewReleaseNotes);
    setError(null);
    setDownloadedBytes(0);
    setTotalBytes(null);

    if (updatePreviewMode === "downloading") {
      setTotalBytes(100 * 1024 * 1024);
      setDownloadedBytes(52 * 1024 * 1024);
      setStatus("downloading");
    } else if (updatePreviewMode === "downloading-unknown") {
      setDownloadedBytes(32 * 1024 * 1024);
      setStatus("downloading");
    } else if (updatePreviewMode === "installing") {
      setTotalBytes(100 * 1024 * 1024);
      setDownloadedBytes(100 * 1024 * 1024);
      setStatus("installing");
    } else if (updatePreviewMode === "ready" || isUiPreview) {
      setStatus("ready");
    } else if (updatePreviewMode === "error") {
      setError(t("settings.about.updateError"));
      setStatus("error");
    } else {
      setStatus("available");
    }

    dialogOpenRef.current = true;
    setDialogOpen(true);
  }, [t]);

  useEffect(() => {
    dialogOpenRef.current = dialogOpen;
  }, [dialogOpen]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const releasePendingUpdate = useCallback(async () => {
    const update = updateRef.current;
    updateRef.current = null;
    if (!update) return;
    await closeUpdate(update).catch((value) => reportFrontendError("Failed to close updater resource", value));
  }, []);

  useEffect(() => () => { void releasePendingUpdate(); }, [releasePendingUpdate]);

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
    await releasePendingUpdate();
    setError(null);
    setAvailableVersion(null);
    setReleaseNotes("");
    setUpdateKind(null);
    setStatus("checking");
    let applicationError: unknown = null;
    let interfaceError: unknown = null;
    try {
      try {
        const update = await checkForUpdate();
        if (update) {
          updateRef.current = update;
          setUpdateKind("application");
          setAvailableVersion(update.version);
          setReleaseNotes(update.body?.trim() ?? "");
          setDownloadedBytes(0);
          setTotalBytes(null);
          setStatus("available");
          dialogOpenRef.current = true;
          setDialogOpen(true);
          return;
        }
      } catch (value) {
        applicationError = value;
        reportFrontendError("Application update check failed", value);
      }

      try {
        const uiState = await api.checkAndPrepareUiUpdate();
        if (uiState.preparedVersion) {
          setUpdateKind("interface");
          setAvailableVersion(uiState.preparedVersion);
          setReleaseNotes(uiState.preparedReleaseNotes ?? "");
          setDownloadedBytes(0);
          setTotalBytes(null);
          setStatus("ready");
          dialogOpenRef.current = true;
          setDialogOpen(true);
          return;
        }
      } catch (value) {
        reportFrontendError("Interface update check failed", value);
        interfaceError = value;
      }

      if (manual && (applicationError || interfaceError)) {
        setStatus("error");
        setError(t("settings.about.updateError"));
      } else {
        setStatus(manual ? "latest" : "idle");
      }
    } catch (value) {
      reportFrontendError("Update check failed", value);
      setStatus(manual ? "error" : "idle");
      if (manual) setError(t("settings.about.updateError"));
    } finally {
      busy.current = false;
    }
  }, [canUpdate, releasePendingUpdate, t]);

  const installUpdate = useCallback(async () => {
    const update = updateRef.current;
    if ((!update && !updatePreview) || busy.current) return;
    busy.current = true;
    setError(null);
    setDownloadedBytes(0);
    setTotalBytes(null);
    setStatus("downloading");
    let contentLength: number | null = null;
    try {
      if (updatePreview) {
        contentLength = 100 * 1024 * 1024;
        setTotalBytes(contentLength);
        for (const downloaded of [8, 19, 34, 52, 71, 86, 100]) {
          await waitForPreviewStep(420);
          if (!mountedRef.current) return;
          setDownloadedBytes(downloaded * 1024 * 1024);
        }
        setStatus("installing");
        await waitForPreviewStep(1_200);
        if (!mountedRef.current) return;
      } else {
        await downloadAndInstall(update!, (event) => {
          if (event.event === "Started") {
            contentLength = event.data.contentLength ?? null;
            setTotalBytes(contentLength);
          } else if (event.event === "Progress") {
            setDownloadedBytes((current) => current + event.data.chunkLength);
          } else {
            if (contentLength) setDownloadedBytes(contentLength);
            setStatus("installing");
          }
        });
      }
      await releasePendingUpdate();
      setStatus("ready");
      if (!dialogOpenRef.current) toast.success(t("settings.about.updateReadyToast"));
    } catch (value) {
      reportFrontendError("Update download or installation failed", value);
      await releasePendingUpdate();
      setStatus("error");
      setError(t("settings.about.updateError"));
    } finally {
      busy.current = false;
    }
  }, [releasePendingUpdate, t]);

  const restartToUpdate = useCallback(async () => {
    setError(null);
    // 预览复用真实弹窗，但不能触发原生刷新或应用重启。
    if (updatePreview) {
      dialogOpenRef.current = false;
      setDialogOpen(false);
      return;
    }
    try {
      if (updateKind === "interface") {
        await api.applyPreparedUiUpdate();
        return;
      }
      await relaunchApplication();
    } catch (value) {
      reportFrontendError(
        updateKind === "interface" ? "Failed to apply interface update" : "Failed to relaunch after update",
        value,
      );
      setError(t(updateKind === "interface" ? "settings.about.interfaceApplyError" : "settings.about.restartError"));
    }
  }, [t, updateKind]);

  const dismissDialog = useCallback(() => {
    dialogOpenRef.current = false;
    setDialogOpen(false);
    if (!["downloading", "installing", "ready", "error"].includes(status)) void releasePendingUpdate();
  }, [releasePendingUpdate, status]);

  const openUpdateDialog = useCallback(() => {
    dialogOpenRef.current = true;
    setDialogOpen(true);
  }, []);

  const retryUpdate = useCallback(async () => {
    setDialogOpen(false);
    await releasePendingUpdate();
    await runCheck(true);
  }, [releasePendingUpdate, runCheck]);

  useEffect(() => {
    if (updatePreview || !loaded || autoChecked.current || !config.app.autoCheckUpdates) return;
    autoChecked.current = true;
    void runCheck(false);
  }, [config.app.autoCheckUpdates, loaded, runCheck]);

  const percentage = totalBytes && totalBytes > 0
    ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
    : null;
  const value = useMemo<UpdateContextValue>(() => ({
    currentVersion,
    availableVersion,
    error,
    status,
    updateKind,
    progressPercentage: percentage,
    checkForUpdates: () => runCheck(true),
    openUpdateDialog,
    restartToUpdate,
  }), [availableVersion, currentVersion, error, openUpdateDialog, percentage, restartToUpdate, runCheck, status, updateKind]);

  return {
    value,
    dialog: {
      open: dialogOpen,
      currentVersion,
      availableVersion,
      releaseNotes,
      downloadedBytes,
      totalBytes,
      error,
      status,
      updateKind,
      progressPercentage: percentage,
      language,
      t,
      openUpdateDialog,
      dismissDialog,
      installUpdate,
      restartToUpdate,
      retryUpdate,
    },
  };
}
