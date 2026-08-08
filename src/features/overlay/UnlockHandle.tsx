import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { UiIcon } from "../../components/UiIcon";
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
      <UiIcon name="lockOpen" />
    </button>
  );
}
