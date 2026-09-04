import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { api, isTauriRuntime, messageOf } from "../../../shared/api";
import { createTauriListenerCleanup } from "../../../shared/tauriEvent";
import type {
  OverlaySettings,
  OverlayStyle,
  OverlayAppearance,
  ProviderCredentialView,
  ProviderSettingsView,
  ProviderStatus,
  SettingsSection,
} from "../../../shared/types";
import { defaultOverlayStyle } from "../../../shared/types";
import { rememberSettingsPath } from "../../../router/settingsRoute";
import type { ProviderDragState } from "./SettingsContext";

type UseSettingsDataOptions = {
  locationPathname: string;
  appearance: OverlayAppearance;
  providerStatuses: ProviderStatus[];
};

export function useSettingsData({
  appearance,
  locationPathname,
  providerStatuses,
}: UseSettingsDataOptions) {
  const fileInput = useRef<HTMLInputElement>(null);
  const providerRows = useRef(new Map<string, HTMLDivElement>());
  const [overlaySettings, setOverlaySettings] = useState<OverlaySettings>({ visible: true, locked: false });
  const [style, setStyle] = useState<OverlayStyle>(defaultOverlayStyle);
  const [providerView, setProviderView] = useState<ProviderSettingsView | null>(null);
  const [providerCredentials, setProviderCredentials] = useState<ProviderCredentialView | null>(null);
  const [testingProvider, setTestingProvider] = useState<string | null>(null);
  const [resettingSection, setResettingSection] = useState<SettingsSection | null>(null);
  const [confirmingReset, setConfirmingReset] = useState<SettingsSection | null>(null);
  const [providerDrag, setProviderDrag] = useState<ProviderDragState | null>(null);
  const [savingProviderOrder, setSavingProviderOrder] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => rememberSettingsPath(locationPathname), [locationPathname]);

  useEffect(() => {
    document.documentElement.dataset.window = "main";
    if (!isTauriRuntime()) return;
    void api.getOverlaySettings().then(setOverlaySettings).catch((value) => setError(messageOf(value)));
    void api.getOverlayStyle().then(setStyle).catch((value) => setError(messageOf(value)));
    void api.getProviderSettings().then(setProviderView).catch((value) => setError(messageOf(value)));
    void api.getProviderCredentials().then(setProviderCredentials).catch((value) => setError(messageOf(value)));
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
    setStyle((current) => ({ ...current, ...appearance }));
  }, [appearance]);

  useEffect(() => {
    if (!notice) return;
    toast.success(notice);
    setNotice(null);
  }, [notice]);

  useEffect(() => {
    if (!error) return;
    toast.error(error);
    setError(null);
  }, [error]);

  useEffect(() => {
    if (providerStatuses.length === 0) return;
    setProviderView((current) => current ? { ...current, statuses: providerStatuses } : current);
  }, [providerStatuses]);

  return {
    confirmingReset,
    error,
    fileInput,
    notice,
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
  };
}
