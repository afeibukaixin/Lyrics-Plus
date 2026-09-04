import type { TFunction } from "i18next";
import {
  Bug,
  CircleAlert,
  Download,
  FileJson,
  Info,
  LoaderCircle,
  Monitor,
  MonitorUp,
  Moon,
  Music2,
  Palette,
  RotateCw,
  Settings2,
  Sun,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import type { UpdateStatus } from "../../../features/update/UpdateProvider";
import type { ThemePreference } from "../../../shared/types";

import type { SettingsNavigationItem } from "./SettingsSidebar";

export const themeCycle: readonly ThemePreference[] = ["dark", "light", "system"];

export function buildSettingsNavigation(t: TFunction, playerHasWarning: boolean) {
  const primaryNavigation: SettingsNavigationItem[] = [
    { to: "/settings/style", label: t("settings.shell.nav.style"), icon: Palette },
    { to: "/settings/lyrics", label: t("settings.shell.nav.lyrics"), icon: Music2 },
    { to: "/settings/player", label: t("settings.shell.nav.player"), icon: MonitorUp, warning: playerHasWarning },
    { to: "/settings/application", label: t("settings.shell.nav.application"), icon: Settings2 },
    { to: "/settings/about", label: t("settings.shell.nav.about"), icon: Info },
  ];
  const advancedNavigation: SettingsNavigationItem[] = [
    { to: "/settings/debug", label: t("settings.shell.nav.debug"), icon: Bug },
    { to: "/settings/config", label: t("settings.shell.nav.config"), icon: FileJson },
  ];
  return { advancedNavigation, primaryNavigation };
}

export function getThemeToggle(t: TFunction, theme: ThemePreference) {
  const currentThemeIndex = themeCycle.indexOf(theme);
  const nextTheme = themeCycle[(currentThemeIndex + 1) % themeCycle.length];
  const themeToggleLabelKey = ({
    light: "settings.theme.switchToLight",
    dark: "settings.theme.switchToDark",
    system: "settings.theme.switchToSystem",
  } as const)[nextTheme];
  const ThemeToggleIcon = theme === "light" ? Sun : theme === "dark" ? Moon : Monitor;
  return {
    icon: ThemeToggleIcon,
    label: t(themeToggleLabelKey),
    nextTheme,
  };
}

export type SettingsUpdateIndicator = {
  icon: LucideIcon;
  text: string;
};

export function getUpdateIndicator(
  t: TFunction,
  status: UpdateStatus,
  progressPercentage: number | null,
): SettingsUpdateIndicator | null {
  const indicator = status === "downloading"
    ? { icon: Download, label: t("settings.about.updateCard.downloading") }
    : status === "installing"
      ? { icon: LoaderCircle, label: t("settings.about.updateCard.installing") }
      : status === "ready"
        ? { icon: RotateCw, label: t("settings.about.updateCard.ready") }
        : status === "error"
          ? { icon: CircleAlert, label: t("settings.about.updateCard.error") }
          : null;
  const action = t(status === "ready" ? "settings.about.updateCard.restart" : "settings.about.updateCard.open");
  if (!indicator) return null;
  const text = [
    indicator.label,
    status === "downloading" && progressPercentage !== null ? `${progressPercentage}%` : null,
    action,
  ].filter(Boolean).join(" · ");
  return { icon: indicator.icon, text };
}
