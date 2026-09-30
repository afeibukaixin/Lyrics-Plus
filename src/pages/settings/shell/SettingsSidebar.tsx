import type { TFunction } from "i18next";
import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { NavLink } from "react-router";
import { MessageCircle, Music2, type LucideIcon } from "lucide-react";
import { api, messageOf } from "../../../shared/api";
import type { PlaybackSnapshot } from "../../../shared/types";
import systemMediaControlCenter from "../../../assets/system-media-control-center.png";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import styles from "../settings.module.scss";
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
  setError: (message: string) => void;
};

const REMOTE_CONFIG_URL = "https://raw.githubusercontent.com/afeibukaixin/Lyrics-Plus/main/remote-config.json";

type QqGroupConfig = {
  number: string;
  joinUrl: string;
};

function parseQqGroupConfig(value: unknown): QqGroupConfig | null {
  if (typeof value !== "object" || value === null || !("qqGroup" in value)) return null;
  const qqGroup = value.qqGroup;
  if (typeof qqGroup !== "object" || qqGroup === null) return null;
  if (!("number" in qqGroup) || !("joinUrl" in qqGroup)) return null;

  const number = qqGroup.number;
  const joinUrl = qqGroup.joinUrl;
  if (typeof number !== "string" || !/^\d+$/.test(number)) return null;
  if (typeof joinUrl !== "string") return null;

  try {
    const url = new URL(joinUrl);
    if (url.protocol !== "https:" || url.hostname !== "qm.qq.com") return null;
  } catch {
    return null;
  }

  return { number, joinUrl };
}

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

function currentPlayerBundleId(snapshot: PlaybackSnapshot): string | null {
  if (!snapshot.isRunning || snapshot.errorCode !== null) return null;
  switch (snapshot.player) {
    case "apple_music": return "com.apple.Music";
    case "spotify": return "com.spotify.client";
    case "system": return snapshot.sourceAppBundleId?.trim() || null;
    default: return null;
  }
}

export function SettingsSidebar({
  t,
  locationPathname,
  playbackSnapshot,
  primaryNavigation,
  advancedNavigation,
  setError,
}: SettingsSidebarProps) {
  const playerName = currentPlayerName(playbackSnapshot, t);
  const playerBundleId = currentPlayerBundleId(playbackSnapshot);
  const noCurrentPlayer = !playbackSnapshot.isRunning || playbackSnapshot.errorCode !== null || playbackSnapshot.player === null;
  const [applicationIcons, setApplicationIcons] = useState<Record<string, string>>({});
  const [qqGroup, setQqGroup] = useState<QqGroupConfig | null>(null);
  const playerIcon = playerBundleId ? applicationIcons[playerBundleId] : null;

  useEffect(() => {
    const controller = new AbortController();
    let active = true;

    void fetch(REMOTE_CONFIG_URL, { cache: "no-store", signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) return null;
        return parseQqGroupConfig(await response.json() as unknown);
      })
      .then((value) => {
        if (active) setQqGroup(value);
      })
      .catch(() => {
        if (active) setQqGroup(null);
      });

    return () => {
      active = false;
      controller.abort();
    };
  }, []);

  useEffect(() => {
    if (!playerBundleId) return;

    let active = true;
    void api.getApplicationIcons([playerBundleId])
      .then((icons) => {
        if (active) setApplicationIcons((current) => ({ ...current, ...icons }));
      })
      .catch(() => {});
    return () => { active = false; };
  }, [playerBundleId]);

  const openQqGroup = () => {
    if (qqGroup) void openUrl(qqGroup.joinUrl).catch((error) => setError(messageOf(error)));
  };

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
        {qqGroup && <SidebarGroup className="p-0">
          <SidebarGroupLabel>{t("settings.about.community")}</SidebarGroupLabel>
          <SidebarGroupContent>
            <a
              href={qqGroup.joinUrl}
              className="flex h-8 min-w-0 items-center rounded-md px-2 text-sidebar-foreground underline underline-offset-4 hover:text-sidebar-accent-foreground focus-visible:outline-2 focus-visible:outline-sidebar-ring group-data-[collapsible=icon]:hidden"
              aria-label={`${t("settings.about.joinQqGroup")}：${t("settings.about.qqGroup", { number: qqGroup.number })}`}
              onClick={(event) => { event.preventDefault(); openQqGroup(); }}
            >
              <span className="truncate">{t("settings.about.qqGroup", { number: qqGroup.number })}</span>
            </a>
            <SidebarMenu className="hidden group-data-[collapsible=icon]:flex">
              <SidebarMenuItem>
                <SidebarMenuButton type="button" aria-label={t("settings.about.community")} tooltip={t("settings.about.community")} onClick={openQqGroup}>
                  <MessageCircle aria-hidden="true" />
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>}
        <SidebarGroup className="p-0 group-data-[collapsible=icon]:hidden">
          <SidebarGroupLabel>{t("settings.shell.currentPlayer")}</SidebarGroupLabel>
          <SidebarGroupContent>
            {noCurrentPlayer ? (
              <Tooltip>
                <TooltipTrigger delay={0} render={<button type="button" className={styles.noPlayerTrigger} />}>
                  <Music2 className="size-5 shrink-0" aria-hidden="true" />
                  <span className="min-w-0 truncate">{playerName}</span>
                </TooltipTrigger>
                <TooltipContent side="right" align="end" sideOffset={10} className={styles.noPlayerTooltip}>
                  <img src={systemMediaControlCenter} alt={t("settings.shell.systemMediaHelpImageAlt")} className={styles.noPlayerScreenshot} width={664} height={993} decoding="async" />
                  <p>{t("settings.shell.systemMediaHelp")}</p>
                </TooltipContent>
              </Tooltip>
            ) : (
              <div className="flex h-8 min-w-0 items-center gap-2 px-2">
                {playerIcon ? <img className="size-5 shrink-0 object-contain" src={playerIcon} alt="" /> : <Music2 className="size-5 shrink-0" aria-hidden="true" />}
                <span className="min-w-0 truncate" title={playerName}>{playerName}</span>
              </div>
            )}
          </SidebarGroupContent>
        </SidebarGroup>
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
