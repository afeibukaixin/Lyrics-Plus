import { emitTo, type UnlistenFn } from "@tauri-apps/api/event";

type MaybeAsyncUnlistenFn = () => void | Promise<void>;

export const NOTCH_WIDTH_PREVIEW_EVENT = "notch://width-preview";
export const NOTCH_VISIBILITY_TRANSITION_EVENT = "notch://visibility-transition";

export type NotchWidthPreviewPayload =
  | { phase: "update" | "commit"; width: number }
  | { phase: "cancel" };

export type NotchVisibilityTransitionPayload = { visible: boolean };

export function isTauriRuntime() {
  if (typeof window === "undefined") return false;
  const internals = (window as Window & {
    __TAURI_INTERNALS__?: { invoke?: unknown; transformCallback?: unknown };
  }).__TAURI_INTERNALS__;
  return typeof internals?.invoke === "function" && typeof internals.transformCallback === "function";
}

export function emitNotchWidthPreview(payload: NotchWidthPreviewPayload) {
  if (!isTauriRuntime()) return Promise.resolve();
  return emitTo("lyrics-notch", NOTCH_WIDTH_PREVIEW_EVENT, payload);
}

function runUnlisten(unlisten: UnlistenFn) {
  try {
    // Tauri 2 的类型仍声明返回 void，但运行时实现会返回 Promise。
    void Promise.resolve((unlisten as MaybeAsyncUnlistenFn)()).catch(() => {
      // Webview 销毁或热更新时，底层可能已先移除监听器。
    });
  } catch {
    // unregisterListener 也可能在 Promise 创建前同步抛错。
  }
}

export function createTauriListenerCleanup(listener: Promise<UnlistenFn>) {
  let cleanupRequested = false;
  let unlisten: UnlistenFn | undefined;

  void listener.then((registeredUnlisten) => {
    if (cleanupRequested) {
      runUnlisten(registeredUnlisten);
      return;
    }
    unlisten = registeredUnlisten;
  }).catch(() => {
    // 组件清理只做尽力而为；监听注册失败由对应功能的状态处理。
  });

  return () => {
    if (cleanupRequested) return;
    cleanupRequested = true;
    if (!unlisten) return;
    const registeredUnlisten = unlisten;
    unlisten = undefined;
    runUnlisten(registeredUnlisten);
  };
}

export function disposeTauriListener(unlisten: UnlistenFn | undefined) {
  if (unlisten) runUnlisten(unlisten);
}
