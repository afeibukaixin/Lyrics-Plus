import {
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
  type SetStateAction,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { NavLink, Outlet, useLocation, useOutletContext } from "react-router";
import { useTranslation } from "react-i18next";
import { Bug, FileJson, Info, MonitorUp, Music2, Palette, Settings2, SlidersHorizontal, TriangleAlert, X } from "lucide-react";
import { Alert } from "@/components/ui/alert";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "@/components/ui/alert-dialog";
import { IconButton } from "@/components/ui/icon-button";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarInset,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { toast } from "sonner";
import { useLyrics } from "../features/lyrics/useLyrics";
import { usePlayback } from "../features/player/usePlayback";
import { useAppConfig } from "../features/config/AppConfigProvider";
import { api, isTauriRuntime, messageOf } from "../shared/api";
import { createTauriListenerCleanup } from "../shared/tauriEvent";
import {
  defaultOverlayStyle,
  type OverlaySettings,
  type OverlayStyle,
  type ProviderSettings,
  type ProviderSettingsView,
  type SettingsSection,
} from "../shared/types";
import styles from "./settings.module.scss";
import { rememberSettingsPath } from "../router/settingsRoute";

type ProviderDragState = {
  providerId: string;
  pointerId: number;
  sourceIndex: number;
  targetIndex: number;
  startY: number;
  currentY: number;
  positions: Array<{ top: number; center: number }>;
};

export type SettingsOutletContext = {
  config: ReturnType<typeof useAppConfig>["config"];
  setTheme: ReturnType<typeof useAppConfig>["setTheme"];
  setLanguage: ReturnType<typeof useAppConfig>["setLanguage"];
  setGlobalShortcuts: ReturnType<typeof useAppConfig>["setGlobalShortcuts"];
  setSystemMediaFilterMode: ReturnType<typeof useAppConfig>["setSystemMediaFilterMode"];
  setSystemMediaApplications: ReturnType<typeof useAppConfig>["setSystemMediaApplications"];
  setPlayerFollowerApplication: ReturnType<typeof useAppConfig>["setPlayerFollowerApplication"];
  setDockIconHidden: ReturnType<typeof useAppConfig>["setDockIconHidden"];
  setSilentStartup: ReturnType<typeof useAppConfig>["setSilentStartup"];
  setOverlayHideWhenNotPlaying: ReturnType<typeof useAppConfig>["setOverlayHideWhenNotPlaying"];
  playback: ReturnType<typeof usePlayback>;
  lyrics: ReturnType<typeof useLyrics>;
  fileInput: RefObject<HTMLInputElement | null>;
  providerRows: RefObject<Map<string, HTMLDivElement>>;
  overlaySettings: OverlaySettings;
  style: OverlayStyle;
  providerView: ProviderSettingsView | null;
  testingProvider: string | null;
  resettingSection: SettingsSection | null;
  confirmingReset: SettingsSection | null;
  providerDrag: ProviderDragState | null;
  savingProviderOrder: boolean;
  setError: Dispatch<SetStateAction<string | null>>;
  setNotice: Dispatch<SetStateAction<string | null>>;
  updateStyle: (patch: Partial<OverlayStyle>) => Promise<boolean>;
  setVisible: (visible: boolean) => Promise<void>;
  setLocked: (locked: boolean) => Promise<void>;
  saveProviderSettings: (settings: ProviderSettings) => Promise<boolean>;
  beginProviderDrag: (providerId: string, sourceIndex: number, event: ReactPointerEvent<HTMLButtonElement>) => void;
  continueProviderDrag: (event: ReactPointerEvent<HTMLButtonElement>) => void;
  finishProviderDrag: (event: ReactPointerEvent<HTMLButtonElement>) => void;
  setProviderDrag: Dispatch<SetStateAction<ProviderDragState | null>>;
  providerDragTransform: (index: number) => string | undefined;
  toggleProvider: (id: string) => void;
  testProviders: (providerIds: string[]) => Promise<void>;
  handleFile: (file?: File) => Promise<void>;
  resetSection: (target: SettingsSection) => Promise<void>;
  resetOverlayBounds: () => Promise<void>;
  syncAppliedConfig: (imported: Parameters<ReturnType<typeof useAppConfig>["syncConfig"]>[0], appearanceOnly: boolean) => Promise<void>;
};

export default function Settings() {
  const { t } = useTranslation();
  const location = useLocation();
  const {
    config,
    setTheme,
    setLanguage,
    setGlobalShortcuts,
    setSystemMediaFilterMode,
    setSystemMediaApplications,
    setPlayerFollowerApplication,
    setDockIconHidden,
    setSilentStartup,
    setOverlayHideWhenNotPlaying,
    syncConfig,
  } = useAppConfig();
  const playback = usePlayback();
  const lyrics = useLyrics(playback.snapshot, playback.positionMs, false);
  const fileInput = useRef<HTMLInputElement>(null);
  const providerRows = useRef(new Map<string, HTMLDivElement>());
  const [overlaySettings, setOverlaySettings] = useState<OverlaySettings>({ visible: true, locked: false });
  const [style, setStyle] = useState<OverlayStyle>(defaultOverlayStyle);
  const [providerView, setProviderView] = useState<ProviderSettingsView | null>(null);
  const [testingProvider, setTestingProvider] = useState<string | null>(null);
  const [resettingSection, setResettingSection] = useState<SettingsSection | null>(null);
  const [confirmingReset, setConfirmingReset] = useState<SettingsSection | null>(null);
  const [providerDrag, setProviderDrag] = useState<ProviderDragState | null>(null);
  const [savingProviderOrder, setSavingProviderOrder] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => rememberSettingsPath(location.pathname), [location.pathname]);

  useEffect(() => {
    document.documentElement.dataset.window = "main";
    if (!isTauriRuntime()) return;
    void api.getOverlaySettings().then(setOverlaySettings).catch((value) => setError(messageOf(value)));
    void api.getOverlayStyle().then(setStyle).catch((value) => setError(messageOf(value)));
    void api.getProviderSettings().then(setProviderView).catch((value) => setError(messageOf(value)));
    const cleanupSettingsListener = createTauriListenerCleanup(
      listen<OverlaySettings>("overlay://settings", ({ payload }) => setOverlaySettings(payload)),
    );
    const cleanupStyleListener = createTauriListenerCleanup(
      listen<OverlayStyle>("overlay://style", ({ payload }) => setStyle(payload)),
    );
    return () => {
      cleanupSettingsListener();
      cleanupStyleListener();
    };
  }, []);

  useEffect(() => {
    if (!notice) return;
    toast.success(notice);
    setNotice(null);
  }, [notice]);

  useEffect(() => {
    if (!providerDrag) return;
    const cancelDrag = (event: KeyboardEvent) => {
      if (event.key === "Escape") setProviderDrag(null);
    };
    window.addEventListener("keydown", cancelDrag);
    return () => window.removeEventListener("keydown", cancelDrag);
  }, [providerDrag]);

  useEffect(() => {
    if (lyrics.providerStatuses.length === 0) return;
    setProviderView((current) => current ? { ...current, statuses: lyrics.providerStatuses } : current);
  }, [lyrics.providerStatuses]);

  const updateStyle = async (patch: Partial<OverlayStyle>) => {
    const previous = style;
    const next = { ...style, ...patch };
    setStyle(next);
    try {
      setStyle(await api.setOverlayStyle(next));
      return true;
    } catch (value) {
      setStyle(previous);
      setError(messageOf(value));
      return false;
    }
  };

  const setVisible = async (visible: boolean) => {
    try {
      await api.setOverlayVisible(visible);
      setOverlaySettings((current) => ({ ...current, visible }));
    } catch (value) { setError(messageOf(value)); }
  };

  const setLocked = async (locked: boolean) => {
    try {
      await api.setOverlayLocked(locked);
      setOverlaySettings((current) => ({ ...current, locked }));
    } catch (value) { setError(messageOf(value)); }
  };

  const saveProviderSettings = async (settings: ProviderSettings) => {
    try {
      setProviderView(await api.setProviderSettings(settings));
      return true;
    } catch (value) {
      setError(messageOf(value));
      return false;
    }
  };

  const moveProvider = async (sourceId: string, targetId: string) => {
    if (!providerView || sourceId === targetId || savingProviderOrder) return;
    const previous = providerView;
    const providers = [...previous.settings.providers];
    const sourceIndex = providers.findIndex((provider) => provider.id === sourceId);
    const targetIndex = providers.findIndex((provider) => provider.id === targetId);
    if (sourceIndex < 0 || targetIndex < 0) return;
    const [source] = providers.splice(sourceIndex, 1);
    providers.splice(targetIndex, 0, source);
    const settings = { ...previous.settings, providers };
    setProviderView({ ...previous, settings });
    setSavingProviderOrder(true);
    try {
      setProviderView(await api.setProviderSettings(settings));
    } catch (value) {
      setProviderView((current) => ({ ...previous, statuses: current?.statuses ?? previous.statuses }));
      setError(messageOf(value));
    } finally {
      setSavingProviderOrder(false);
    }
  };

  const beginProviderDrag = (providerId: string, sourceIndex: number, event: ReactPointerEvent<HTMLButtonElement>) => {
    if (!providerView || providerDrag || savingProviderOrder || !event.isPrimary) return;
    if (event.pointerType === "mouse" && event.button !== 0) return;
    const positions = providerView.settings.providers.map((provider) => {
      const bounds = providerRows.current.get(provider.id)?.getBoundingClientRect();
      return bounds ? { top: bounds.top, center: bounds.top + bounds.height / 2 } : null;
    });
    if (positions.some((position) => position === null)) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    setProviderDrag({
      providerId,
      pointerId: event.pointerId,
      sourceIndex,
      targetIndex: sourceIndex,
      startY: event.clientY,
      currentY: event.clientY,
      positions: positions as ProviderDragState["positions"],
    });
  };

  const continueProviderDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (!providerDrag || providerDrag.pointerId !== event.pointerId) return;
    event.preventDefault();
    const currentY = event.clientY;
    const draggedCenter = providerDrag.positions[providerDrag.sourceIndex].center + currentY - providerDrag.startY;
    let targetIndex = providerDrag.sourceIndex;
    if (draggedCenter > providerDrag.positions[providerDrag.sourceIndex].center) {
      for (let index = providerDrag.sourceIndex + 1; index < providerDrag.positions.length; index += 1) {
        if (draggedCenter > providerDrag.positions[index].center) targetIndex = index;
      }
    } else {
      for (let index = providerDrag.sourceIndex - 1; index >= 0; index -= 1) {
        if (draggedCenter < providerDrag.positions[index].center) targetIndex = index;
      }
    }
    setProviderDrag((current) => current && current.pointerId === event.pointerId
      ? { ...current, currentY, targetIndex }
      : current);
  };

  const finishProviderDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (!providerDrag || providerDrag.pointerId !== event.pointerId) return;
    const { providerId, sourceIndex, targetIndex } = providerDrag;
    const targetId = providerView?.settings.providers[targetIndex]?.id;
    setProviderDrag(null);
    if (targetId && sourceIndex !== targetIndex) void moveProvider(providerId, targetId);
  };

  const providerDragTransform = (index: number) => {
    if (!providerDrag) return undefined;
    if (index === providerDrag.sourceIndex) {
      return `translate3d(0, ${providerDrag.currentY - providerDrag.startY}px, 0) scale(1.015)`;
    }
    if (providerDrag.targetIndex > providerDrag.sourceIndex && index > providerDrag.sourceIndex && index <= providerDrag.targetIndex) {
      return `translate3d(0, ${providerDrag.positions[index - 1].top - providerDrag.positions[index].top}px, 0)`;
    }
    if (providerDrag.targetIndex < providerDrag.sourceIndex && index >= providerDrag.targetIndex && index < providerDrag.sourceIndex) {
      return `translate3d(0, ${providerDrag.positions[index + 1].top - providerDrag.positions[index].top}px, 0)`;
    }
    return undefined;
  };

  const toggleProvider = (id: string) => {
    if (!providerView) return;
    void saveProviderSettings({
      ...providerView.settings,
      providers: providerView.settings.providers.map((provider) => provider.id === id ? { ...provider, enabled: !provider.enabled } : provider),
    });
  };

  const testProviders = async (providerIds: string[]) => {
    if (testingProvider || providerIds.length === 0) return;
    setTestingProvider(providerIds.length === 1 ? providerIds[0] : "*");
    try {
      const statuses = await Promise.all(providerIds.map(api.testProvider));
      setProviderView((current) => current ? { ...current, statuses: current.statuses.map((item) => statuses.find((status) => status.providerId === item.providerId) ?? item) } : current);
    } catch (value) { setError(messageOf(value)); } finally { setTestingProvider(null); }
  };

  const handleFile = async (file?: File) => {
    if (!file) return;
    await lyrics.importRaw(await file.text());
    if (fileInput.current) fileInput.current.value = "";
  };

  const resetSection = async (target: SettingsSection) => {
    setConfirmingReset(target);
  };

  const confirmResetSection = async () => {
    const target = confirmingReset;
    if (!target) return;
    const names: Record<SettingsSection, string> = {
      style: t("settings.shell.nav.style"),
      display: t("settings.shell.nav.display"),
      lyrics: t("settings.shell.nav.lyrics"),
      player: t("settings.shell.nav.player"),
      application: t("settings.shell.nav.application"),
      about: t("settings.shell.nav.about"),
    };
    setConfirmingReset(null);
    setResettingSection(target);
    setError(null);
    setNotice(null);
    try {
      const result = await api.resetSettingsSection(target);
      setOverlaySettings(result.overlaySettings);
      setStyle(result.overlayStyle);
      setProviderView(result.providerView);
      playback.syncSelection(result.playerSelection);
      setNotice(t("settings.shell.resetDone", { section: names[target] }));
    } catch (value) {
      setError(messageOf(value));
    } finally {
      setResettingSection(null);
    }
  };

  const resetOverlayBounds = async () => {
    setError(null);
    setNotice(null);
    try {
      const resetStyle = await api.resetOverlayBounds();
      setStyle(resetStyle);
      setOverlaySettings((current) => ({ ...current, visible: true }));
      setNotice(t("settings.shell.positionReset"));
    } catch (value) {
      setError(messageOf(value));
    }
  };

  const syncAppliedConfig = async (imported: Parameters<typeof syncConfig>[0], appearanceOnly: boolean) => {
    syncConfig(imported);
    setStyle(await api.getOverlayStyle());
    if (!appearanceOnly) {
      setOverlaySettings(await api.getOverlaySettings());
      setProviderView(await api.getProviderSettings());
      playback.syncSelection(imported.app.playerSelection);
    }
  };

  const context: SettingsOutletContext = {
    config,
    setTheme,
    setLanguage,
    setGlobalShortcuts,
    setSystemMediaFilterMode,
    setSystemMediaApplications,
    setPlayerFollowerApplication,
    setDockIconHidden,
    setSilentStartup,
    setOverlayHideWhenNotPlaying,
    playback,
    lyrics,
    fileInput,
    providerRows,
    overlaySettings,
    style,
    providerView,
    testingProvider,
    resettingSection,
    confirmingReset,
    providerDrag,
    savingProviderOrder,
    setError,
    setNotice,
    updateStyle,
    setVisible,
    setLocked,
    saveProviderSettings,
    beginProviderDrag,
    continueProviderDrag,
    finishProviderDrag,
    setProviderDrag,
    providerDragTransform,
    toggleProvider,
    testProviders,
    handleFile,
    resetSection,
    resetOverlayBounds,
    syncAppliedConfig,
  };

  const playerHasWarning = Boolean(playback.configError || playback.snapshotLoadError)
    || (Boolean(playback.snapshot.errorCode)
      && !["waiting", "no_unique_player", "source_not_allowed"].includes(playback.snapshot.errorCode ?? ""));

  const primaryNavigation = [
    { to: "/settings/style", label: t("settings.shell.nav.style"), icon: Palette },
    { to: "/settings/display", label: t("settings.shell.nav.display"), icon: SlidersHorizontal },
    { to: "/settings/lyrics", label: t("settings.shell.nav.lyrics"), icon: Music2 },
    { to: "/settings/player", label: t("settings.shell.nav.player"), icon: MonitorUp, warning: playerHasWarning },
    { to: "/settings/application", label: t("settings.shell.nav.application"), icon: Settings2 },
    { to: "/settings/about", label: t("settings.shell.nav.about"), icon: Info },
  ];
  const advancedNavigation = [
    { to: "/settings/debug", label: t("settings.shell.nav.debug"), icon: Bug },
    { to: "/settings/config", label: t("settings.shell.nav.config"), icon: FileJson },
  ];

  return (
    <SidebarProvider className={styles.shell} style={{ "--sidebar-width": "11.5rem", "--sidebar-width-icon": "3.5rem" } as React.CSSProperties}>
      <Sidebar collapsible="icon" aria-label={t("settings.shell.navigation")}>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                {primaryNavigation.map((item) => {
                  const Icon = item.icon;
                  return (
                    <SidebarMenuItem key={item.to}>
                      <SidebarMenuButton render={<NavLink to={item.to} />} isActive={location.pathname === item.to} tooltip={item.label}>
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
              <SidebarMenu>
                {advancedNavigation.map((item) => {
                  const Icon = item.icon;
                  return <SidebarMenuItem key={item.to}><SidebarMenuButton render={<NavLink to={item.to} />} isActive={location.pathname === item.to} tooltip={item.label}><Icon aria-hidden="true" /><span>{item.label}</span></SidebarMenuButton></SidebarMenuItem>;
                })}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarFooter>
      </Sidebar>

      <SidebarInset className={styles.settingsLayout}>
        <div className={styles.sidebarTriggerRow}><SidebarTrigger aria-label={t("settings.shell.navigation")} /></div>
        <div className={styles.content}>
          {error && <Alert className={styles.inlineError}><span>{error}</span><IconButton label={t("settings.shell.closeToast")} variant="ghost" size="icon-sm" onClick={() => setError(null)}><X /></IconButton></Alert>}
          <Outlet context={context} />
        </div>
      </SidebarInset>
      <AlertDialog open={confirmingReset !== null} onOpenChange={(open) => { if (!open && !resettingSection) setConfirmingReset(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader><AlertDialogTitle>{t("settings.shell.resetTitle")}</AlertDialogTitle><AlertDialogDescription>{t("settings.shell.resetConfirm")}</AlertDialogDescription></AlertDialogHeader>
          <AlertDialogFooter><AlertDialogCancel>{t("common.actions.cancel")}</AlertDialogCancel><AlertDialogAction disabled={resettingSection !== null} onClick={() => void confirmResetSection()}>{t("common.actions.resetDefault")}</AlertDialogAction></AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SidebarProvider>
  );
}

export function useSettingsContext() {
  return useOutletContext<SettingsOutletContext>();
}
