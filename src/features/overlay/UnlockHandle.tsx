import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { api } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";
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
    <button
      className={styles.handle}
      data-hover={hovered}
      disabled={busy}
      aria-label={t("settings.app.unlockOverlay")}
      title={t("settings.app.unlockOverlay")}
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
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <rect x="5" y="10" width="14" height="10" rx="2" />
        <path d="M8 10V7a4 4 0 0 1 7.6-1.7" />
        <path d="M12 14v2" />
      </svg>
    </button>
  );
}
