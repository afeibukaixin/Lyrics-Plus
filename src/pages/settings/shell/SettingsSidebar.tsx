import type { TFunction } from "i18next";
import { NavLink } from "react-router";
import { TriangleAlert } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
export type SettingsNavigationItem = {
  to: string;
  label: string;
  icon: LucideIcon;
  warning?: boolean;
};

type SettingsSidebarProps = {
  t: TFunction;
  locationPathname: string;
  primaryNavigation: SettingsNavigationItem[];
  advancedNavigation: SettingsNavigationItem[];
};

export function SettingsSidebar({
  t,
  locationPathname,
  primaryNavigation,
  advancedNavigation,
}: SettingsSidebarProps) {
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
                    <SidebarMenuButton render={<NavLink to={item.to} />} isActive={locationPathname === item.to} tooltip={item.label}>
                      <Icon aria-hidden="true" /><span>{item.label}</span>
                    </SidebarMenuButton>
                    {item.warning && <SidebarMenuBadge><TriangleAlert role="img" aria-label={t("settings.player.attentionStatus")} className="text-warning" /></SidebarMenuBadge>}
                  </SidebarMenuItem>
                );
              })}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
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
