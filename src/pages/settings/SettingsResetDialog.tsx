import type { TFunction } from "i18next";
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

type SettingsResetDialogProps = {
  t: TFunction;
  open: boolean;
  resetting: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
};

export function SettingsResetDialog({
  t,
  open,
  resetting,
  onOpenChange,
  onConfirm,
}: SettingsResetDialogProps) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader><AlertDialogTitle>{t("settings.shell.resetTitle")}</AlertDialogTitle><AlertDialogDescription>{t("settings.shell.resetConfirm")}</AlertDialogDescription></AlertDialogHeader>
        <AlertDialogFooter><AlertDialogCancel>{t("common.actions.cancel")}</AlertDialogCancel><AlertDialogAction disabled={resetting} onClick={onConfirm}>{t("common.actions.resetDefault")}</AlertDialogAction></AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
