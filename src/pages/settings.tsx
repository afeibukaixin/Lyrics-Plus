import {
  useEffect,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { Outlet, useLocation } from "react-router";
import { useTranslation } from "react-i18next";
import { Bug, CircleAlert, Download, FileJson, Info, LoaderCircle, Monitor, MonitorUp, Moon, Music2, Palette, RotateCw, Settings2, Sun, X } from "lucide-react";
import { Alert } from "@/components/ui/alert";
import { IconButton } from "@/components/ui/icon-button";
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";
import { useLyrics } from "../features/lyrics/useLyrics";
import { usePlayback } from "../features/player/usePlayback";
import { useAppConfig } from "../features/config/AppConfigProvider";
import { useUpdates } from "../features/update/UpdateProvider";
import { api, messageOf } from "../shared/api";
import {
  type OverlayStyle,
  type ProviderSettings,
  type MusixmatchTokenType,
  type SettingsSection,
  type ThemePreference,
} from "../shared/types";
import styles from "./settings.module.scss";
import { UpdateProgressRing } from "./settings/UpdateProgressRing";
import { SettingsResetDialog } from "./settings/SettingsResetDialog";
import { SettingsSidebar, type SettingsNavigationItem } from "./settings/SettingsSidebar";
import { useSettingsData } from "./settings/useSettingsData";
import {
  type ProviderDragState,
  type SettingsOutletContext,
} from "./settings/SettingsContext";
import {
  continueProviderDrag as updateProviderDrag,
  providerDragTransform,
} from "./settings/providerDrag";

const themeCycle: readonly ThemePreference[] = ["dark", "light", "system"];

export { useSettingsContext } from "./settings/SettingsContext";
export type { ProviderDragState, SettingsOutletContext } from "./settings/SettingsContext";


export default function Settings() {
  const { t } = useTranslation();
  const location = useLocation();
  const { openUpdateDialog, progressPercentage, status: updateStatus } = useUpdates();
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
    setStatusBarLyricsEnabled,
    setListLyricsVisible,
    setListLyricsOptions,
    setNotchLyricsVisible,
    setLyricsDisplayPreferences,
    setLyricsBaseAppearance,
    setLyricsStyleInheritance,
    resetLyricsBaseAppearance,
    syncConfig,
  } = useAppConfig();
  const playback = usePlayback();
  const lyrics = useLyrics(playback.snapshot, playback.positionMs);
  const {
    confirmingReset,
    error,
    fileInput,
    overlaySettings,
    providerCredentials,
    providerDrag,
    providerRows,
    providerView,
    resettingSection,
    savingProviderOrder,
    setConfirmingReset,
    setError,
    setNotice,
    setOverlaySettings,
    setProviderCredentials,
    setProviderDrag,
    setProviderView,
    setResettingSection,
    setSavingProviderOrder,
    setTestingProvider,
    setStyle,
    style,
    testingProvider,
  } = useSettingsData({
    appearance: config.overlay.appearance,
    locationPathname: location.pathname,
    providerStatuses: lyrics.providerStatuses,
  });

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

  const saveMusixmatchToken = async (tokenType: MusixmatchTokenType, token: string) => {
    try {
      const update = await api.setMusixmatchToken(tokenType, token);
      setProviderCredentials(update.credentials);
      setProviderView(update.providerView);
      return true;
    } catch (value) {
      setError(messageOf(value));
      return false;
    }
  };

  const clearMusixmatchToken = async () => {
    try {
      const update = await api.clearMusixmatchToken();
      setProviderCredentials(update.credentials);
      setProviderView(update.providerView);
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
    setProviderDrag((current) => current ? updateProviderDrag(current, event.pointerId, currentY) : current);
  };

  const finishProviderDrag = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (!providerDrag || providerDrag.pointerId !== event.pointerId) return;
    const { providerId, sourceIndex, targetIndex } = providerDrag;
    const targetId = providerView?.settings.providers[targetIndex]?.id;
    setProviderDrag(null);
    if (targetId && sourceIndex !== targetIndex) void moveProvider(providerId, targetId);
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

  const testAllProviders = async () => {
    if (testingProvider || !lyrics.trackKey) return;
    setTestingProvider("*");
    try {
      const response = await lyrics.search();
      if (!response) return;
      setProviderView((current) => current ? {
        ...current,
        statuses: current.statuses.map((item) => {
          const provider = current.settings.providers.find((candidate) => candidate.id === item.providerId);
          if (!provider?.enabled) {
            return { ...item, health: "unknown" as const, message: t("settings.lyrics.notParticipated"), checkedAtMs: null };
          }
          return response.providerStatuses.find((status) => status.providerId === item.providerId) ?? item;
        }),
      } : current);
      if (response.error) setError(response.error);
    } catch (value) {
      setError(messageOf(value));
    } finally {
      setTestingProvider(null);
    }
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
    setStatusBarLyricsEnabled,
    setListLyricsVisible,
    setListLyricsOptions,
    setNotchLyricsVisible,
    setLyricsDisplayPreferences,
    setLyricsBaseAppearance,
    setLyricsStyleInheritance,
    resetLyricsBaseAppearance,
    playback,
    lyrics,
    fileInput,
    providerRows,
    overlaySettings,
    style,
    providerView,
    providerCredentials,
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
    saveMusixmatchToken,
    clearMusixmatchToken,
    beginProviderDrag,
    continueProviderDrag,
    finishProviderDrag,
    setProviderDrag,
    providerDragTransform: (index) => providerDragTransform(providerDrag, index),
    toggleProvider,
    testProviders,
    testAllProviders,
    handleFile,
    resetSection,
    resetOverlayBounds,
    syncAppliedConfig,
  };

  const playerHasWarning = Boolean(playback.configError || playback.snapshotLoadError)
    || (Boolean(playback.snapshot.errorCode)
      && !["waiting", "no_unique_player", "source_not_allowed"].includes(playback.snapshot.errorCode ?? ""));

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
  const currentThemeIndex = themeCycle.indexOf(config.app.theme);
  const nextTheme = themeCycle[(currentThemeIndex + 1) % themeCycle.length];
  const themeToggleLabelKey = ({
    light: "settings.theme.switchToLight",
    dark: "settings.theme.switchToDark",
    system: "settings.theme.switchToSystem",
  } as const)[nextTheme];
  const themeToggleLabel = t(themeToggleLabelKey);
  const ThemeToggleIcon = config.app.theme === "light" ? Sun : config.app.theme === "dark" ? Moon : Monitor;
  const updateIndicator = updateStatus === "downloading"
    ? { icon: Download, label: t("settings.about.updateCard.downloading") }
    : updateStatus === "installing"
      ? { icon: LoaderCircle, label: t("settings.about.updateCard.installing") }
      : updateStatus === "ready"
        ? { icon: RotateCw, label: t("settings.about.updateCard.ready") }
        : updateStatus === "error"
          ? { icon: CircleAlert, label: t("settings.about.updateCard.error") }
          : null;
  const UpdateIndicatorIcon = updateIndicator?.icon;
  const updateIndicatorAction = t(updateStatus === "ready" ? "settings.about.updateCard.restart" : "settings.about.updateCard.open");
  const updateIndicatorLabel = updateIndicator
    ? [
      updateIndicator.label,
      updateStatus === "downloading" && progressPercentage !== null ? `${progressPercentage}%` : null,
      updateIndicatorAction,
    ].filter(Boolean).join(" · ")
    : "";

  return (
    <SidebarProvider className={styles.shell} style={{ "--sidebar-width": "11.5rem", "--sidebar-width-icon": "3.5rem" } as React.CSSProperties}>
      <SettingsSidebar
        advancedNavigation={advancedNavigation}
        locationPathname={location.pathname}
        primaryNavigation={primaryNavigation}
        t={t}
      />

      <SidebarInset className={styles.settingsLayout}>
        <div className={styles.sidebarTriggerRow}>
          <SidebarTrigger aria-label={t("settings.shell.navigation")} size="icon" />
          <IconButton label={themeToggleLabel} tooltip={themeToggleLabel} variant="ghost" size="icon" onClick={() => void setTheme(nextTheme).catch((value) => setError(messageOf(value)))}>
            <ThemeToggleIcon />
          </IconButton>
          {updateIndicator && UpdateIndicatorIcon ? (
            <IconButton
              className={styles.updateStatusButton}
              data-progress={updateStatus === "downloading" && progressPercentage !== null ? "true" : undefined}
              data-status={updateStatus}
              label={updateIndicatorLabel}
              tooltip={updateIndicatorLabel}
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
          {error && <Alert className={styles.inlineError}><span>{error}</span><IconButton label={t("settings.shell.closeToast")} variant="ghost" size="icon-sm" onClick={() => setError(null)}><X /></IconButton></Alert>}
          <Outlet context={context} />
        </div>
      </SidebarInset>
      <SettingsResetDialog
        onConfirm={() => void confirmResetSection()}
        onOpenChange={(open) => { if (!open && !resettingSection) setConfirmingReset(null); }}
        open={confirmingReset !== null}
        resetting={resettingSection !== null}
        t={t}
      />
    </SidebarProvider>
  );
}
