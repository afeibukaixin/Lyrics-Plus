import React from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "react-router/dom";
import router from "./router";
import Overlay from "./features/overlay/Overlay";
import UnlockHandle from "./features/overlay/UnlockHandle";
import QuickLyricsWindow from "./features/lyrics/QuickLyricsWindow";
import { AppConfigProvider } from "./features/config/AppConfigProvider";
import { DebugLogProvider } from "./features/debug/DebugLogProvider";

import "virtual:uno.css";
import "./styles.scss";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {new URLSearchParams(window.location.search).get("view") === "overlay" ? (
      <Overlay />
    ) : new URLSearchParams(window.location.search).get("view") === "unlock-handle" ? (
      <UnlockHandle />
    ) : new URLSearchParams(window.location.search).get("view") === "quick-lyrics" ? (
      <AppConfigProvider windowType="quick-lyrics">
        <DebugLogProvider><QuickLyricsWindow /></DebugLogProvider>
      </AppConfigProvider>
    ) : (
      <AppConfigProvider>
        <DebugLogProvider><RouterProvider router={router} /></DebugLogProvider>
      </AppConfigProvider>
    )}
  </React.StrictMode>
);
