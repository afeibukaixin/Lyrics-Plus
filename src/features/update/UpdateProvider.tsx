import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useTranslation } from "react-i18next";
import { LoaderCircle } from "lucide-react";
import { isTauriRuntime } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import { useAppConfig } from "../config/AppConfigProvider";
import { useAppLanguage } from "../i18n/I18nProvider";
import appIcon from "../../../src-tauri/icons/128x128.png";
import styles from "./UpdateProvider.module.scss";

export type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "installing" | "ready" | "latest" | "error";

type UpdateContextValue = {
  currentVersion: string;
  availableVersion: string | null;
  error: string | null;
  status: UpdateStatus;
  checkForUpdates: () => Promise<void>;
  restartToUpdate: () => Promise<void>;
};

const UpdateContext = createContext<UpdateContextValue | null>(null);

function formatBytes(bytes: number, language: string) {
  const value = bytes / 1024 / 1024;
  return `${new Intl.NumberFormat(language, { maximumFractionDigits: value < 10 ? 1 : 0 }).format(value)} MB`;
}

export function UpdateProvider({ children }: { children: React.ReactNode }) {
  const { config, loaded } = useAppConfig();
  const { language } = useAppLanguage();
  const { t } = useTranslation();
  const dialogRef = useRef<HTMLDialogElement>(null);
  const updateRef = useRef<Update | null>(null);
  const busy = useRef(false);
  const autoChecked = useRef(false);
  const [currentVersion, setCurrentVersion] = useState("—");
  const [availableVersion, setAvailableVersion] = useState<string | null>(null);
  const [releaseNotes, setReleaseNotes] = useState("");
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [totalBytes, setTotalBytes] = useState<number | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const canUpdate = import.meta.env.PROD && isTauriRuntime();

  useEffect(() => {
    if (!isTauriRuntime()) return;
    void getVersion().then(setCurrentVersion).catch((value) => {
      reportFrontendError("Failed to read app version", value);
    });
  }, []);

  useEffect(() => {
    if (!import.meta.env.DEV || new URLSearchParams(window.location.search).get("update-preview") !== "1") return;
    setCurrentVersion("1.1.0");
    setAvailableVersion("1.2.0");
    setReleaseNotes("• 全新的应用内更新界面\n• 显示真实下载进度与文件大小\n• 安装完成后可选择立即或稍后重启\n• 优化多语言提示与键盘操作");
    setStatus("available");
    setDialogOpen(true);
  }, []);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (dialogOpen && !dialog.open) dialog.showModal();
    if (!dialogOpen && dialog.open) dialog.close();
  }, [dialogOpen]);

  const releasePendingUpdate = useCallback(async () => {
    const update = updateRef.current;
    updateRef.current = null;
    if (!update) return;
    await update.close().catch((value) => reportFrontendError("Failed to close updater resource", value));
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
    setError(null);
    setAvailableVersion(null);
    setStatus("checking");
    try {
      const update = await check({ timeout: 15_000 });
      if (!update) {
        setStatus("latest");
        return;
      }

      updateRef.current = update;
      setAvailableVersion(update.version);
      setReleaseNotes(update.body?.trim() ?? "");
      setDownloadedBytes(0);
      setTotalBytes(null);
      setStatus("available");
      setDialogOpen(true);
    } catch (value) {
      reportFrontendError("Update check failed", value);
      setStatus(manual ? "error" : "idle");
      if (manual) setError(t("settings.about.updateError"));
    } finally {
      busy.current = false;
    }
  }, [canUpdate, t]);

  const installUpdate = useCallback(async () => {
    const update = updateRef.current;
    if (!update || busy.current) return;
    busy.current = true;
    setError(null);
    setDownloadedBytes(0);
    setTotalBytes(null);
    setStatus("downloading");
    let contentLength: number | null = null;
    try {
      await update.downloadAndInstall((event) => {
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
      await releasePendingUpdate();
      setStatus("ready");
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
    try {
      await relaunch();
    } catch (value) {
      reportFrontendError("Failed to relaunch after update", value);
      setError(t("settings.about.restartError"));
    }
  }, [t]);

  const dismissDialog = useCallback(() => {
    if (status === "downloading" || status === "installing") return;
    setDialogOpen(false);
    if (status !== "ready") void releasePendingUpdate();
  }, [releasePendingUpdate, status]);

  const retryUpdate = useCallback(async () => {
    setDialogOpen(false);
    await releasePendingUpdate();
    await runCheck(true);
  }, [releasePendingUpdate, runCheck]);

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
    restartToUpdate,
  }), [availableVersion, currentVersion, error, restartToUpdate, runCheck, status]);

  const percentage = totalBytes && totalBytes > 0
    ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
    : null;
  const title = status === "downloading"
    ? t("settings.about.downloadingTitle")
    : status === "installing"
      ? t("settings.about.installingTitle")
      : status === "ready"
        ? t("settings.about.restartTitle")
        : status === "error"
          ? t("settings.about.updateFailed")
          : t("settings.about.updateAvailable");
  const description = status === "downloading"
    ? t("settings.about.downloadingHint")
    : status === "installing"
      ? t("settings.about.installingHint")
      : status === "ready"
        ? t("settings.about.restartHint", { version: availableVersion ?? "" })
        : status === "error"
          ? t("settings.about.updateError")
          : t("settings.about.updateHint", { version: availableVersion ?? "" });
  const active = status === "downloading" || status === "installing";

  return (
    <UpdateContext.Provider value={value}>
      {children}
      <dialog
        aria-describedby="update-dialog-description"
        aria-labelledby="update-dialog-title"
        className={styles.dialog}
        onCancel={(event) => {
          event.preventDefault();
          if (!active) dismissDialog();
        }}
        ref={dialogRef}
      >
        <header className={styles.hero}>
          <div className={styles.icon}><img alt="" src={appIcon} /></div>
          <div className={styles.heading}>
            <span>Lyrics Plus</span>
            <h1 id="update-dialog-title">{title}</h1>
            <p id="update-dialog-description" role={status === "error" ? "alert" : undefined}>{description}</p>
          </div>
        </header>

        <div className={styles.versionRow}>
          <span>{t("settings.about.version", { version: currentVersion })}</span>
          <b aria-hidden="true">→</b>
          <strong>{t("settings.about.version", { version: availableVersion ?? "—" })}</strong>
        </div>

        <section className={styles.notes} aria-labelledby="update-notes-title">
          <h2 id="update-notes-title">{t("settings.about.releaseNotes")}</h2>
          <div>{releaseNotes || t("settings.about.noReleaseNotes")}</div>
        </section>

        {(status === "downloading" || status === "installing") && (
          <section className={styles.progress} aria-live="polite">
            <div>
              <span><LoaderCircle className="animate-spin" />{t(`settings.about.status.${status}`, { version: availableVersion ?? "" })}</span>
              <strong>{percentage === null ? "" : `${percentage}%`}</strong>
            </div>
            <progress
              aria-label={t("settings.about.downloadProgress")}
              max={totalBytes ?? undefined}
              value={totalBytes ? Math.min(downloadedBytes, totalBytes) : undefined}
            />
            {status === "downloading" && downloadedBytes > 0 && (
              <small>
                {totalBytes
                  ? t("settings.about.downloadedSize", { downloaded: formatBytes(downloadedBytes, language), total: formatBytes(totalBytes, language) })
                  : t("settings.about.downloadedUnknownSize", { downloaded: formatBytes(downloadedBytes, language) })}
              </small>
            )}
          </section>
        )}

        {error && status !== "error" && <p className={styles.error} role="alert">{error}</p>}

        <footer className={styles.footer}>
          {status === "available" && (
            <>
              <button className={styles.secondary} type="button" onClick={dismissDialog}>{t("common.actions.cancel")}</button>
              <button className={styles.primary} type="button" onClick={() => void installUpdate()}>{t("settings.about.installNow")}</button>
            </>
          )}
          {active && <button className={styles.primary} disabled type="button">{t(`settings.about.status.${status}`, { version: availableVersion ?? "" })}</button>}
          {status === "ready" && (
            <>
              <button className={styles.secondary} type="button" onClick={dismissDialog}>{t("settings.about.restartLater")}</button>
              <button className={styles.primary} type="button" onClick={() => void restartToUpdate()}>{t("settings.about.restartNow")}</button>
            </>
          )}
          {status === "error" && (
            <>
              <button className={styles.secondary} type="button" onClick={dismissDialog}>{t("common.actions.close")}</button>
              <button className={styles.primary} type="button" onClick={() => void retryUpdate()}>{t("settings.about.retryUpdate")}</button>
            </>
          )}
        </footer>
      </dialog>
    </UpdateContext.Provider>
  );
}

export function useUpdates() {
  const value = useContext(UpdateContext);
  if (!value) throw new Error("useUpdates must be used within UpdateProvider");
  return value;
}
