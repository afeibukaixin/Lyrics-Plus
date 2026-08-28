import { useEffect, useRef, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { isTauriRuntime } from "../../shared/api";
import { createTauriListenerCleanup } from "../../shared/tauriEvent";

type SurfaceRuntimeState = "active" | "dormant";

/**
 * 以 Rust 窗口生命周期为唯一状态源，供高频任务决定是否继续运行。
 * macOS 切换 Space 会短暂改变 document.visibilityState，不代表窗口进入休眠。
 */
export function useSurfaceActivity() {
  const [active, setActive] = useState(() => !isTauriRuntime());
  const runtimeEventObservedRef = useRef(false);

  useEffect(() => {
    let disposed = false;

    if (!isTauriRuntime()) {
      return () => {
        disposed = true;
      };
    }

    const currentWebviewWindow = getCurrentWebviewWindow();
    void currentWebviewWindow.isVisible().then((visible) => {
      if (disposed || runtimeEventObservedRef.current) return;
      setActive(visible);
    }).catch(() => undefined);
    const cleanupRuntimeListener = createTauriListenerCleanup(
      currentWebviewWindow.listen<SurfaceRuntimeState>("surface://runtime-state", ({ payload }) => {
        if (disposed) return;
        runtimeEventObservedRef.current = true;
        setActive(payload === "active");
      }),
    );

    return () => {
      disposed = true;
      cleanupRuntimeListener();
    };
  }, []);

  return active;
}
