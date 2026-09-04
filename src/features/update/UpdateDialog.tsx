import { X } from "lucide-react";
import appIcon from "../../../src-tauri/icons/128x128.png";
import { Button } from "@/components/ui/button";
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { IconButton } from "@/components/ui/icon-button";
import { Progress } from "@/components/ui/progress";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Spinner } from "@/components/ui/spinner";
import type { UpdateDialogProps } from "./useUpdateController";

function formatBytes(bytes: number, language: string) {
  const value = bytes / 1024 / 1024;
  return `${new Intl.NumberFormat(language, { maximumFractionDigits: value < 10 ? 1 : 0 }).format(value)} MB`;
}

export function UpdateDialog({
  open,
  currentVersion,
  availableVersion,
  releaseNotes,
  downloadedBytes,
  totalBytes,
  error,
  status,
  progressPercentage,
  language,
  t,
  openUpdateDialog,
  dismissDialog,
  installUpdate,
  restartToUpdate,
  retryUpdate,
}: UpdateDialogProps) {
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
    <Dialog open={open} disablePointerDismissal onOpenChange={(nextOpen) => { if (nextOpen) openUpdateDialog(); else dismissDialog(); }}>
      <DialogContent className="grid max-h-[min(640px,calc(100vh-48px))] w-[min(520px,calc(100vw-48px))] grid-rows-[auto_auto_minmax(0,1fr)_auto_auto] gap-0 overflow-hidden bg-card p-0" showClose={false}>
        <DialogClose render={<IconButton className="absolute right-3 top-3 z-10 text-muted-foreground hover:text-foreground" label={t("common.actions.close")} variant="ghost" size="icon-sm" />}><X /></DialogClose>
        <DialogHeader className="flex flex-row items-center gap-3.5 border-b border-border px-[22px] pb-4 pr-12 pt-5">
          <div className="size-12 shrink-0 rounded-xl border border-border bg-muted p-[3px]"><img className="block size-full rounded-[9px]" alt="" src={appIcon} /></div>
          <div className="min-w-0">
            <DialogTitle id="update-dialog-title">{title}</DialogTitle>
            <DialogDescription className="mt-1 leading-relaxed" id="update-dialog-description" role={status === "error" ? "alert" : undefined}>{description}</DialogDescription>
          </div>
        </DialogHeader>

        <div className="mx-[22px] mb-3 mt-3.5 flex items-center gap-2 rounded-lg bg-muted px-2.5 py-2 text-xs tabular-nums">
          <span>{t("settings.about.version", { version: currentVersion })}</span>
          <b className="text-muted-foreground" aria-hidden="true">→</b>
          <strong className="text-primary">{t("settings.about.version", { version: availableVersion ?? "—" })}</strong>
        </div>

        <section className="mx-[22px] grid min-h-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden border-t border-border" aria-labelledby="update-notes-title">
          <h2 className="m-0 py-2.5 text-sm font-medium" id="update-notes-title">{t("settings.about.releaseNotes")}</h2>
          <ScrollArea className="h-full min-h-0 max-h-[220px]"><div className="whitespace-pre-wrap pb-3 pr-3 text-sm leading-relaxed text-muted-foreground">{releaseNotes || t("settings.about.noReleaseNotes")}</div></ScrollArea>
        </section>

        {(status === "downloading" || status === "installing") && (
          <section className="grid gap-2 px-6 pt-4" aria-live="polite">
            <div className="flex items-center justify-between gap-3 text-xs">
              <span className="inline-flex items-center gap-2"><Spinner />{t(`settings.about.status.${status}`, { version: availableVersion ?? "" })}</span>
              <strong className="text-primary tabular-nums">{progressPercentage === null ? "" : `${progressPercentage}%`}</strong>
            </div>
            <Progress
              aria-label={t("settings.about.downloadProgress")}
              className={progressPercentage === null ? "animate-pulse" : undefined}
              value={progressPercentage ?? 100}
            />
            {status === "downloading" && downloadedBytes > 0 && (
              <small className="text-right text-xs text-muted-foreground tabular-nums">
                {totalBytes
                  ? t("settings.about.downloadedSize", { downloaded: formatBytes(downloadedBytes, language), total: formatBytes(totalBytes, language) })
                  : t("settings.about.downloadedUnknownSize", { downloaded: formatBytes(downloadedBytes, language) })}
              </small>
            )}
          </section>
        )}

        {error && status !== "error" && <Alert variant="destructive" className="mx-6 mt-3"><AlertDescription>{error}</AlertDescription></Alert>}

        <DialogFooter className="flex justify-end gap-2 border-t border-border px-[22px] pb-[18px] pt-3.5">
          {status === "available" && (
            <>
              <Button className="min-w-28" variant="secondary" type="button" onClick={dismissDialog}>{t("common.actions.cancel")}</Button>
              <Button className="min-w-28" type="button" onClick={() => void installUpdate()}>{t("settings.about.installNow")}</Button>
            </>
          )}
          {active && <Button className="min-w-28" disabled type="button">{t(`settings.about.status.${status}`, { version: availableVersion ?? "" })}</Button>}
          {status === "ready" && (
            <>
              <Button className="min-w-28" variant="secondary" type="button" onClick={dismissDialog}>{t("settings.about.restartLater")}</Button>
              <Button className="min-w-28" type="button" onClick={() => void restartToUpdate()}>{t("settings.about.restartNow")}</Button>
            </>
          )}
          {status === "error" && (
            <>
              <Button className="min-w-28" variant="secondary" type="button" onClick={dismissDialog}>{t("common.actions.close")}</Button>
              <Button className="min-w-28" type="button" onClick={() => void retryUpdate()}>{t("settings.about.retryUpdate")}</Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
