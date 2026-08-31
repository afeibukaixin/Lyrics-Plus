import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { LockOpen } from "lucide-react";
import { api } from "../../shared/api";
import { reportFrontendError } from "../../shared/debugLog";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import { IconButton } from "@/components/ui/icon-button";
import styles from "./UnlockHandle.module.scss";

export default function UnlockHandle() {
  const { t } = useTranslation();
  const params = new URLSearchParams(window.location.search);
  const target = params.get("target") === "list" || params.get("view") === "lyrics-list-unlock-handle" ? "list" : "overlay";
  const hoverEvent = target === "list" ? "lyrics-list-unlock-handle://hover" : "unlock-handle://hover";
  const unlockLabel = target === "list" ? t("lyricsList.toolbar.unlock") : t("settings.app.unlockOverlay");
  const [busy, setBusy] = useState(false);
  const [hovered, setHovered] = useState(false);

  useEffect(() => {
    document.documentElement.dataset.window = target === "list" ? "lyrics-list-unlock-handle" : "unlock-handle";
    return createTauriListenerCleanup(
      listen<boolean>(hoverEvent, ({ payload }) => setHovered(payload)),
    );
  }, [hoverEvent, target]);

  return (
    <IconButton
      className={styles.handle}
      data-hover={hovered}
      disabled={busy}
      label={unlockLabel}
      tooltip={unlockLabel}
      variant="ghost"
      size="icon-sm"
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onClick={async () => {
        setBusy(true);
        try {
          if (target === "list") {
            await api.setListLyricsLocked(false);
          } else {
            await api.setOverlayLocked(false);
          }
        } catch (error) {
          reportFrontendError("Failed to unlock lyrics window", error);
        } finally {
          setBusy(false);
        }
      }}
    >
      <LockOpen aria-hidden="true" />
    </IconButton>
  );
}
