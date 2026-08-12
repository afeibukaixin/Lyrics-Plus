import { createHashRouter, Navigate } from "react-router";
import Index from "../pages";
import IndexLayout from "../layout";
import Overlay from "../features/overlay/Overlay";
import Library from "../pages/library";
import Settings from "../pages/settings";
import OverlaySettingsPage from "../pages/settings/OverlaySettingsPage";
import LyricsSettingsPage from "../pages/settings/LyricsSettingsPage";
import ArtworkSettingsPage from "../pages/settings/ArtworkSettingsPage";
import AppSettingsPage from "../pages/settings/AppSettingsPage";
import DebugSettingsPage from "../pages/settings/DebugSettingsPage";
import ConfigSettingsPage from "../pages/settings/ConfigSettingsPage";
import AboutSettingsPage from "../pages/settings/AboutSettingsPage";

const router = createHashRouter([
  {
    path: "/",
    element: <IndexLayout />,
    children: [
      {
        path: "/",
        element: <Index />,
      },
      {
        path: "/overlay",
        element: <Overlay />,
      },
      {
        path: "/library",
        element: <Library />,
      },
      {
        path: "/settings",
        element: <Settings />,
        children: [
          { index: true, element: <Navigate to="overlay" replace /> },
          { path: "overlay", element: <OverlaySettingsPage /> },
          { path: "lyrics", element: <LyricsSettingsPage /> },
          { path: "artwork", element: <ArtworkSettingsPage /> },
          { path: "app", element: <AppSettingsPage /> },
          { path: "debug", element: <DebugSettingsPage /> },
          { path: "config", element: <ConfigSettingsPage /> },
          { path: "about", element: <AboutSettingsPage /> },
        ],
      },
    ],
  },
]);

export default router;
