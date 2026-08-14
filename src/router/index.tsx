import { createHashRouter, Navigate } from "react-router";
import IndexLayout from "../layout";
import Settings from "../pages/settings";
import StyleSettingsPage from "../pages/settings/StyleSettingsPage";
import DisplaySettingsPage from "../pages/settings/DisplaySettingsPage";
import LyricsSettingsPage from "../pages/settings/LyricsSettingsPage";
import AppSettingsPage from "../pages/settings/AppSettingsPage";
import DebugSettingsPage from "../pages/settings/DebugSettingsPage";
import ConfigSettingsPage from "../pages/settings/ConfigSettingsPage";
import AboutSettingsPage from "../pages/settings/AboutSettingsPage";
import { lastSettingsSection } from "./settingsRoute";

function SettingsIndexRedirect() {
  return <Navigate to={lastSettingsSection()} replace />;
}

const router = createHashRouter([
  {
    path: "/",
    element: <IndexLayout />,
    children: [
      {
        path: "/",
        element: <Navigate to="/settings" replace />,
      },
      {
        path: "/settings",
        element: <Settings />,
        children: [
          { index: true, element: <SettingsIndexRedirect /> },
          { path: "style", element: <StyleSettingsPage /> },
          { path: "display", element: <DisplaySettingsPage /> },
          { path: "lyrics", element: <LyricsSettingsPage /> },
          { path: "player", element: <AppSettingsPage /> },
          { path: "application", element: <AppSettingsPage /> },
          { path: "debug", element: <DebugSettingsPage /> },
          { path: "config", element: <ConfigSettingsPage /> },
          { path: "about", element: <AboutSettingsPage /> },
        ],
      },
      {
        path: "*",
        element: <Navigate to="/settings" replace />,
      },
    ],
  },
]);

export default router;
