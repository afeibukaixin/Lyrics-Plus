import type { UnlistenFn } from "@tauri-apps/api/event";

type MaybeAsyncUnlistenFn = () => void | Promise<void>;

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
