import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router/dom";
import router from "./router";
import Overlay from "./features/overlay/Overlay";
import UnlockHandle from "./features/overlay/UnlockHandle";
import QuickLyricsWindow from "./features/lyrics/QuickLyricsWindow";
import { AppConfigProvider, useAppConfig } from "./features/config/AppConfigProvider";
import { DebugLogProvider } from "./features/debug/DebugLogProvider";
import { AppI18nProvider } from "./features/i18n/I18nProvider";
import { LegalNoticeGate } from "./features/legal/LegalNoticeGate";
import { UpdateProvider } from "./features/update/UpdateProvider";
import { TooltipProvider } from "./components/ui/tooltip";
import { Toaster } from "./components/ui/sonner";

import "./tailwind.css";
import "./styles.scss";

function AppToaster() {
  const { resolvedTheme } = useAppConfig();
  return <Toaster theme={resolvedTheme} />;
}

const view = new URLSearchParams(window.location.search).get("view");
const updatePreview = import.meta.env.DEV && new URLSearchParams(window.location.search).get("update-preview") === "1";
const windowType = view === "overlay" || view === "unlock-handle" || view === "quick-lyrics"
  ? view
  : "main";

const content = view === "overlay" ? (
  <Overlay />
) : view === "unlock-handle" ? (
  <UnlockHandle />
) : view === "quick-lyrics" ? (
  <DebugLogProvider><QuickLyricsWindow /></DebugLogProvider>
) : updatePreview ? (
  <DebugLogProvider><UpdateProvider><RouterProvider router={router} /></UpdateProvider></DebugLogProvider>
) : (
  <DebugLogProvider><LegalNoticeGate><UpdateProvider><RouterProvider router={router} /></UpdateProvider></LegalNoticeGate></DebugLogProvider>
);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppConfigProvider windowType={windowType}>
      <AppI18nProvider><TooltipProvider delayDuration={400}>{content}<AppToaster /></TooltipProvider></AppI18nProvider>
    </AppConfigProvider>
  </React.StrictMode>
);
