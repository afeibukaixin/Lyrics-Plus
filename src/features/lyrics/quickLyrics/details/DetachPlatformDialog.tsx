import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent,
  AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { Field, FieldGroup } from "@/components/ui/field";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import type { SongManagerViewProps } from "./useSongManager";

export function DetachPlatformDialog({ recordingAction, setRecordingAction, context, t,
  splitLyricsMode, setDetachLyricsMode, canInheritCurrentLyrics, busyAction,
  handleRecordingAction }: SongManagerViewProps) {
  return (    <AlertDialog open={recordingAction !== null} onOpenChange={(isOpen) => { if (!isOpen) setRecordingAction(null); }}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t("quickLyrics.details.detachPlatformTrackTitle")}</AlertDialogTitle>
          <AlertDialogDescription>{t("quickLyrics.details.detachPlatformTrackDescription", {
            platform: recordingAction?.observation.platform,
            count: Math.max(0, (context?.recording.observations.length ?? 1) - 1),
          })}</AlertDialogDescription>
        </AlertDialogHeader>
        <FieldGroup><Field>
          <ToggleGroup orientation="vertical" variant="outline" value={[splitLyricsMode]}
            disabled={busyAction !== null}
            onValueChange={(values) => {
              if (values[0] === "unbound" || values[0] === "inheritCurrent") setDetachLyricsMode(values[0]);
            }}
            aria-label={t("quickLyrics.details.currentLyrics")}>
            <ToggleGroupItem value="unbound">{t("quickLyrics.details.splitLyricsUnbound")}</ToggleGroupItem>
            <ToggleGroupItem value="inheritCurrent" disabled={!canInheritCurrentLyrics}>{t("quickLyrics.details.splitLyricsInherit")}</ToggleGroupItem>
          </ToggleGroup>
        </Field></FieldGroup>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={busyAction !== null}>{t("common.actions.cancel")}</AlertDialogCancel>
          <AlertDialogAction disabled={busyAction !== null} onClick={handleRecordingAction}>{t("quickLyrics.details.detachPlatformTrack")}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>

);
}
