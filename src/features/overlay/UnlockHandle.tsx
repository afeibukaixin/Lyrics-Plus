import { useEffect, useState } from "react";
import { api } from "../../shared/api";
import styles from "./UnlockHandle.module.scss";

export default function UnlockHandle() {
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    document.documentElement.dataset.window = "unlock-handle";
  }, []);

  return (
    <button
      className={styles.handle}
      disabled={busy}
      title="解锁桌面歌词"
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
