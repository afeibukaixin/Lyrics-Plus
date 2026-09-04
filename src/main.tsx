import React, { useEffect } from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router/dom";
import router from "./router";
import Overlay from "./features/overlay/Overlay";
import UnlockHandle from "./features/overlay/UnlockHandle";
import QuickLyricsWindow from "./features/lyrics/QuickLyricsWindow";
import LyricsListWindow from "./features/lyrics/LyricsListWindow";
import NotchLyricsWindow from "./features/lyrics/NotchLyricsWindow";
import StatusBarLyricsWindow from "./features/lyrics/StatusBarLyricsWindow";
import { AppConfigProvider, useAppConfig } from "./features/config/AppConfigProvider";
import { DebugLogProvider } from "./features/debug/DebugLogProvider";
import { AppI18nProvider } from "./features/i18n/I18nProvider";
import { LegalNoticeGate } from "./features/legal/LegalNoticeGate";
import { UpdateProvider } from "./features/update/UpdateProvider";
import { updatePreview } from "./features/update/updateService";
import { TooltipProvider } from "./components/ui/tooltip";
import { Toaster } from "./components/ui/sonner";
import { api, isTauriRuntime } from "./shared/api";

import "./tailwind.css";
import "./styles.scss";

function AppToaster() {
  const { resolvedTheme } = useAppConfig();
  return <Toaster theme={resolvedTheme} />;
}

const view = new URLSearchParams(window.location.search).get("view");
const isUnlockHandleView = view === "unlock-handle" || view === "lyrics-list-unlock-handle";
const windowType = view === "overlay" || view === "unlock-handle" || view === "lyrics-list-unlock-handle" || view === "quick-lyrics" || view === "lyrics-status-bar" || view === "lyrics-list" || view === "lyrics-notch"
  ? view
  : "main";

function currentUiVersion() {
  const isUiProtocol = window.location.protocol === "lyrics-plus-ui:"
    || (window.location.protocol === "http:" && window.location.hostname === "lyrics-plus-ui.localhost");
  if (!isUiProtocol) return "embedded";
  const [version] = window.location.pathname.split("/").filter(Boolean);
  return version || "embedded";
}

function UiReadyReporter() {
  useEffect(() => {
    if (!isTauriRuntime()) return;
    void api.reportUiReady(currentUiVersion()).catch(() => {
      // 页面启动时上报失败不应阻断歌词窗口；Rust 会在超时后执行回滚。
    });
  }, []);
  return null;
}

const content = view === "overlay" ? (
  <Overlay />
) : isUnlockHandleView ? (
  <UnlockHandle />
) : view === "quick-lyrics" ? (
  <DebugLogProvider><QuickLyricsWindow /></DebugLogProvider>
) : view === "lyrics-list" ? (
  <LyricsListWindow />
) : view === "lyrics-status-bar" ? (
  <StatusBarLyricsWindow />
) : view === "lyrics-notch" ? (
  <NotchLyricsWindow />
) : updatePreview ? (
  <DebugLogProvider><UpdateProvider><RouterProvider router={router} /></UpdateProvider></DebugLogProvider>
) : (
  <DebugLogProvider><LegalNoticeGate><UpdateProvider><RouterProvider router={router} /></UpdateProvider></LegalNoticeGate></DebugLogProvider>
);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppConfigProvider windowType={windowType}>
      <AppI18nProvider><TooltipProvider delayDuration={400}>{content}<AppToaster /><UiReadyReporter /></TooltipProvider></AppI18nProvider>
    </AppConfigProvider>
  </React.StrictMode>
);
