import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { LockOpen } from "lucide-react";
import { api } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
import { IconButton } from "@/components/ui/icon-button";
import styles from "./UnlockHandle.module.scss";

export default function UnlockHandle() {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const [hovered, setHovered] = useState(false);

  useEffect(() => {
    document.documentElement.dataset.window = "unlock-handle";
    return createTauriListenerCleanup(
      listen<boolean>("unlock-handle://hover", ({ payload }) => setHovered(payload)),
    );
  }, []);

  return (
    <IconButton
      className={styles.handle}
      data-hover={hovered}
      disabled={busy}
      label={t("settings.app.unlockOverlay")}
      tooltip={t("settings.app.unlockOverlay")}
      variant="ghost"
      size="icon-sm"
      onPointerEnter={() => setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onClick={async () => {
        setBusy(true);
        try {
          await api.setOverlayLocked(false);
        } finally {
          setBusy(false);
        }
      }}
    >
      <LockOpen aria-hidden="true" />
    </IconButton>
  );
}
