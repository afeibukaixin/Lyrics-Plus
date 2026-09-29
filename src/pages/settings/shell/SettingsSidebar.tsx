import type { TFunction } from "i18next";
import { NavLink } from "react-router";
import type { LucideIcon } from "lucide-react";
import type { PlaybackSnapshot } from "../../../shared/types";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
export type SettingsNavigationItem = {
  to: string;
  label: string;
  icon: LucideIcon;
};

type SettingsSidebarProps = {
  t: TFunction;
  locationPathname: string;
  playbackSnapshot: PlaybackSnapshot;
  primaryNavigation: SettingsNavigationItem[];
  advancedNavigation: SettingsNavigationItem[];
};

function currentPlayerName(snapshot: PlaybackSnapshot, t: TFunction): string {
  if (!snapshot.isRunning || snapshot.errorCode !== null) {
    return t("settings.shell.noCurrentPlayer");
  }
  switch (snapshot.player) {
    case "apple_music": return "Apple Music";
    case "spotify": return "Spotify";
    case "system": return snapshot.sourceAppName?.trim() || t("settings.shell.systemMedia");
    default: return t("settings.shell.noCurrentPlayer");
  }
}

export function SettingsSidebar({
  t,
  locationPathname,
  playbackSnapshot,
  primaryNavigation,
  advancedNavigation,
}: SettingsSidebarProps) {
  const playerName = currentPlayerName(playbackSnapshot, t);

  return (
    <Sidebar collapsible="icon" aria-label={t("settings.shell.navigation")}>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu className="gap-1">
              {primaryNavigation.map((item) => {
                const Icon = item.icon;
                return (
                  <SidebarMenuItem key={item.to}>
                    <SidebarMenuButton render={<NavLink to={item.to} />} isActive={locationPathname === item.to || locationPathname.startsWith(`${item.to.split("/songs")[0]}/`)} tooltip={item.label}>
                      <Icon aria-hidden="true" /><span>{item.label}</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                );
              })}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
        <div className="min-w-0 px-2 pb-2 group-data-[collapsible=icon]:hidden">
          <p className="text-xs text-sidebar-foreground/70">{t("settings.shell.currentPlayer")}</p>
          <p className="truncate text-sm" title={playerName}>{playerName}</p>
        </div>
        <SidebarGroup className="p-0">
          <SidebarGroupLabel>{t("settings.shell.advanced")}</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu className="gap-1">
              {advancedNavigation.map((item) => {
                const Icon = item.icon;
                return <SidebarMenuItem key={item.to}><SidebarMenuButton render={<NavLink to={item.to} />} isActive={locationPathname === item.to} tooltip={item.label}><Icon aria-hidden="true" /><span>{item.label}</span></SidebarMenuButton></SidebarMenuItem>;
              })}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarFooter>
    </Sidebar>
  );
}
