import type { TFunction } from "i18next";
import type {
  Dispatch,
  RefObject,
  SetStateAction,
} from "react";

import { api, messageOf } from "../../../shared/api";
import type {
  AppConfig,
  OverlaySettings,
  OverlayStyle,
  PlayerSelection,
  ProviderSettingsView,
  SettingsSection,
} from "../../../shared/types";

type SettingsOperationsOptions = {
  confirmingReset: SettingsSection | null;
  fileInput: RefObject<HTMLInputElement | null>;
  lyrics: {
    importRaw: (raw: string) => Promise<void>;
  };
  playback: {
    syncSelection: (selection: PlayerSelection) => void;
  };
  setConfirmingReset: Dispatch<SetStateAction<SettingsSection | null>>;
  setError: Dispatch<SetStateAction<string | null>>;
  setNotice: Dispatch<SetStateAction<string | null>>;
  setOverlaySettings: Dispatch<SetStateAction<OverlaySettings>>;
  setProviderView: Dispatch<SetStateAction<ProviderSettingsView | null>>;
  setResettingSection: Dispatch<SetStateAction<SettingsSection | null>>;
  setStyle: Dispatch<SetStateAction<OverlayStyle>>;
  style: OverlayStyle;
  syncConfig: (config: AppConfig) => void;
  t: TFunction;
};

export function createSettingsOperations({
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
}: SettingsOperationsOptions) {
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

  const syncAppliedConfig = async (imported: AppConfig, appearanceOnly: boolean) => {
    syncConfig(imported);
    setStyle(await api.getOverlayStyle());
    if (!appearanceOnly) {
      setOverlaySettings(await api.getOverlaySettings());
      setProviderView(await api.getProviderSettings());
      playback.syncSelection(imported.app.playerSelection);
    }
  };

  return {
    confirmResetSection,
    handleFile,
    resetOverlayBounds,
    resetSection,
    setLocked,
    setVisible,
    syncAppliedConfig,
    updateStyle,
  };
}
