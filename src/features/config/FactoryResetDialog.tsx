import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import type { TFunction } from "i18next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Input } from "@/components/ui/input";

export const FACTORY_RESET_CONFIRMATION = "RESET LYRICS PLUS";

type Props = {
  open: boolean;
  confirmation: string;
  resetting: boolean;
  error: string | null;
  t: TFunction;
  onConfirmationChange: (value: string) => void;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
};

export function FactoryResetDialog({
  open,
  confirmation,
  resetting,
  error,
  t,
  onConfirmationChange,
  onOpenChange,
  onConfirm,
}: Props) {
  const confirmed = confirmation === FACTORY_RESET_CONFIRMATION;

  return (
    <AlertDialog open={open} onOpenChange={(nextOpen) => { if (!resetting) onOpenChange(nextOpen); }}>
      <AlertDialogContent className="max-w-lg">
        <AlertDialogHeader>
          <AlertDialogTitle>{t("settings.config.factoryResetTitle")}</AlertDialogTitle>
          <AlertDialogDescription>
            <span className="block">{t("settings.config.factoryResetDescription")}</span>
            <span className="mt-3 block font-medium text-destructive">{t("settings.config.factoryResetWarning")}</span>
          </AlertDialogDescription>
        </AlertDialogHeader>

        <div className="grid gap-3 text-sm">
          <div>
            <p className="mb-1 font-medium">{t("settings.config.factoryResetDeletes")}</p>
            <ul className="list-disc space-y-1 pl-5 text-muted-foreground">
              <li>{t("settings.config.factoryResetDeleteConfig")}</li>
              <li>{t("settings.config.factoryResetDeleteDatabase")}</li>
              <li>{t("settings.config.factoryResetDeleteCredentials")}</li>
              <li>{t("settings.config.factoryResetDeleteDownloads")}</li>
            </ul>
          </div>
          <div>
            <p className="mb-1 font-medium">{t("settings.config.factoryResetKeeps")}</p>
            <p className="text-muted-foreground">{t("settings.config.factoryResetKeepFiles")}</p>
          </div>
        </div>

        {error ? (
          <Alert variant="destructive">
            <AlertTitle>{t("settings.config.factoryResetFailed")}</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        <label className="grid gap-2 text-sm font-medium" htmlFor="factory-reset-confirmation">
          {t("settings.config.factoryResetInputLabel", { phrase: FACTORY_RESET_CONFIRMATION })}
          <Input
            id="factory-reset-confirmation"
            autoComplete="off"
            disabled={resetting}
            onChange={(event) => onConfirmationChange(event.currentTarget.value)}
            spellCheck={false}
            value={confirmation}
          />
        </label>

        <AlertDialogFooter>
          <AlertDialogCancel disabled={resetting}>{t("common.actions.cancel")}</AlertDialogCancel>
          <AlertDialogAction
            disabled={!confirmed || resetting}
            onClick={onConfirm}
            variant="destructive"
          >
            {resetting ? t("settings.config.factoryResetting") : t("settings.config.factoryResetAction")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
