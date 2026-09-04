import type { TFunction } from "i18next";
import { Outlet } from "react-router";
import type { CSSProperties } from "react";
import type { LucideIcon } from "lucide-react";

import { IconButton } from "@/components/ui/icon-button";
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";

import type { UpdateStatus } from "../../../features/update/UpdateProvider";
import type { SettingsSection } from "../../../shared/types";

import { SettingsResetDialog } from "./SettingsResetDialog";
import { SettingsSidebar, type SettingsNavigationItem } from "./SettingsSidebar";
import type { SettingsOutletContext } from "../shared/SettingsContext";
import { UpdateProgressRing } from "./UpdateProgressRing";
import type { SettingsUpdateIndicator } from "./navigation";
import styles from "../settings.module.scss";

type SettingsShellProps = {
  advancedNavigation: SettingsNavigationItem[];
  confirmingReset: SettingsSection | null;
  context: SettingsOutletContext;
  locationPathname: string;
  onConfirmReset: () => void;
  onOpenResetChange: (open: boolean) => void;
  onThemeToggle: () => void;
  openUpdateDialog: () => void;
  primaryNavigation: SettingsNavigationItem[];
  progressPercentage: number | null;
  resettingSection: SettingsSection | null;
  t: TFunction;
  themeToggleIcon: LucideIcon;
  themeToggleLabel: string;
  updateIndicator: SettingsUpdateIndicator | null;
  updateStatus: UpdateStatus;
};

export function SettingsShell({
  advancedNavigation,
  confirmingReset,
  context,
  locationPathname,
  onConfirmReset,
  onOpenResetChange,
  onThemeToggle,
  openUpdateDialog,
  primaryNavigation,
  progressPercentage,
  resettingSection,
  t,
  themeToggleIcon: ThemeToggleIcon,
  themeToggleLabel,
  updateIndicator,
  updateStatus,
}: SettingsShellProps) {
  const UpdateIndicatorIcon = updateIndicator?.icon;

  return (
    <SidebarProvider className={styles.shell} style={{ "--sidebar-width": "11.5rem", "--sidebar-width-icon": "3.5rem" } as CSSProperties}>
      <SettingsSidebar
        advancedNavigation={advancedNavigation}
        locationPathname={locationPathname}
        primaryNavigation={primaryNavigation}
        t={t}
      />

      <SidebarInset className={styles.settingsLayout}>
        <div className={styles.sidebarTriggerRow}>
          <SidebarTrigger aria-label={t("settings.shell.navigation")} size="icon" />
          <IconButton label={themeToggleLabel} tooltip={themeToggleLabel} variant="ghost" size="icon" onClick={onThemeToggle}>
            <ThemeToggleIcon />
          </IconButton>
          {updateIndicator && UpdateIndicatorIcon ? (
            <IconButton
              className={styles.updateStatusButton}
              data-progress={updateStatus === "downloading" && progressPercentage !== null ? "true" : undefined}
              data-status={updateStatus}
              label={updateIndicator.text}
              tooltip={updateIndicator.text}
              variant="outline"
              size="icon"
              onClick={openUpdateDialog}
            >
              {updateStatus === "downloading" && progressPercentage !== null
                ? <UpdateProgressRing value={progressPercentage} />
                : <UpdateIndicatorIcon />}
            </IconButton>
          ) : null}
        </div>
        <div className={styles.content} data-settings-scroll-root>
          <Outlet context={context} />
        </div>
      </SidebarInset>
      <SettingsResetDialog
        onConfirm={onConfirmReset}
        onOpenChange={onOpenResetChange}
        open={confirmingReset !== null}
        resetting={resettingSection !== null}
        t={t}
      />
    </SidebarProvider>
  );
}
