import { useEffect } from "react";
import { useLocation } from "react-router";
import { useTranslation } from "react-i18next";

import { useLyrics } from "../../features/lyrics/useLyrics";
import { usePlayback } from "../../features/player/usePlayback";
import { useAppConfig } from "../../features/config/AppConfigProvider";
import { useUpdates } from "../../features/update/UpdateProvider";
import { messageOf } from "../../shared/api";

import { SettingsShell } from "./shell/SettingsShell";
import {
  buildSettingsNavigation,
  getThemeToggle,
  getUpdateIndicator,
} from "./shell/navigation";
import { useSettingsData } from "./shared/useSettingsData";
import {
  type SettingsOutletContext,
} from "./shared/SettingsContext";
import { providerDragTransform } from "./lyrics/providerDrag";
import { createProviderActions } from "./lyrics/providerActions";
import { createSettingsOperations } from "./shared/operations";

export { useSettingsContext } from "./shared/SettingsContext";
export type { ProviderDragState, SettingsOutletContext } from "./shared/SettingsContext";

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
    setMenuBarIconHidden,
    setSilentStartup,
    setLyricsWindowsShowOnAllSpaces,
    setOverlayHideWhenNotPlaying,
    setStatusBarLyricsEnabled,
    setListLyricsVisible,
    setListLyricsOptions,
    setListLyricsLocked,
    setNotchLyricsVisible,
    setLyricsDisplayPreferences,
    setLyricsBaseAppearance,
    setLyricsStyleInheritance,
    resetLyricsBaseAppearance,
    syncConfig,
  } = useAppConfig();
  const playback = usePlayback({ trackPosition: false });
  const lyrics = useLyrics(playback.snapshot, playback.positionMs, playback.active);
  const {
    confirmingReset,
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

  const {
    beginProviderDrag,
    clearMusixmatchToken,
    continueProviderDrag,
    finishProviderDrag,
    saveMusixmatchToken,
    saveProviderSettings,
    testAllProviders,
    testProviders,
    toggleProvider,
  } = createProviderActions({
    lyrics,
    providerDrag,
    providerRows,
    providerView,
    savingProviderOrder,
    setError,
    setProviderCredentials,
    setProviderDrag,
    setProviderView,
    setSavingProviderOrder,
    setTestingProvider,
    testingProvider,
    t,
  });

  const {
    confirmResetSection,
    handleFile,
    resetOverlayBounds,
    resetSection,
    setLocked,
    setVisible,
    syncAppliedConfig,
    updateStyle,
  } = createSettingsOperations({
    confirmingReset,
    fileInput,
    lyrics,
    playback,
    setConfirmingReset,
    setError,
    setNotice,
    setOverlaySettings,
    setProviderView,
    setResettingSection,
    setStyle,
    style,
    syncConfig,
    t,
  });

  const context: SettingsOutletContext = {
    config,
    setTheme,
    setLanguage,
    setGlobalShortcuts,
    setSystemMediaFilterMode,
    setSystemMediaApplications,
    setPlayerFollowerApplication,
    setDockIconHidden,
    setMenuBarIconHidden,
    setSilentStartup,
    setLyricsWindowsShowOnAllSpaces,
    setOverlayHideWhenNotPlaying,
    setStatusBarLyricsEnabled,
    setListLyricsVisible,
    setListLyricsOptions,
    setListLyricsLocked,
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
  const { primaryNavigation, advancedNavigation } = buildSettingsNavigation(t, playerHasWarning);
  const themeToggle = getThemeToggle(t, config.app.theme);
  const updateIndicator = getUpdateIndicator(t, updateStatus, progressPercentage);

  return (
    <SettingsShell
      advancedNavigation={advancedNavigation}
      confirmingReset={confirmingReset}
      context={context}
      locationPathname={location.pathname}
      onConfirmReset={() => void confirmResetSection()}
      onOpenResetChange={(open) => { if (!open && !resettingSection) setConfirmingReset(null); }}
      onThemeToggle={() => void setTheme(themeToggle.nextTheme).catch((value) => setError(messageOf(value)))}
      openUpdateDialog={openUpdateDialog}
      primaryNavigation={primaryNavigation}
      progressPercentage={progressPercentage}
      resettingSection={resettingSection}
      t={t}
      themeToggleIcon={themeToggle.icon}
      themeToggleLabel={themeToggle.label}
      updateIndicator={updateIndicator}
      updateStatus={updateStatus}
    />
  );
}
