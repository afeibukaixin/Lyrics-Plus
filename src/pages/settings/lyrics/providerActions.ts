import type { TFunction } from "i18next";
import type {
  Dispatch,
  PointerEvent as ReactPointerEvent,
  RefObject,
  SetStateAction,
} from "react";

import { api, messageOf } from "../../../shared/api";
import type {
  LyricsSearchIntent,
  MusixmatchTokenType,
  ProviderCredentialView,
  ProviderSettings,
  ProviderSettingsView,
  SearchResponse,
} from "../../../shared/types";

import type { ProviderDragState } from "../shared/SettingsContext";
import { continueProviderDrag as updateProviderDrag } from "./providerDrag";

type LyricsProviderActions = {
  trackKey: string | null;
  search: (intent?: LyricsSearchIntent) => Promise<SearchResponse | null>;
};

type ProviderActionsOptions = {
  lyrics: LyricsProviderActions;
  providerDrag: ProviderDragState | null;
  providerRows: RefObject<Map<string, HTMLDivElement>>;
  providerView: ProviderSettingsView | null;
  savingProviderOrder: boolean;
  setError: Dispatch<SetStateAction<string | null>>;
  setProviderCredentials: Dispatch<SetStateAction<ProviderCredentialView | null>>;
  setProviderDrag: Dispatch<SetStateAction<ProviderDragState | null>>;
  setProviderView: Dispatch<SetStateAction<ProviderSettingsView | null>>;
  setSavingProviderOrder: Dispatch<SetStateAction<boolean>>;
  setTestingProvider: Dispatch<SetStateAction<string | null>>;
  testingProvider: string | null;
  t: TFunction;
};

export function createProviderActions({
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
}: ProviderActionsOptions) {
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
      const response = await lyrics.search("manual");
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

  return {
    beginProviderDrag,
    clearMusixmatchToken,
    continueProviderDrag,
    finishProviderDrag,
    saveMusixmatchToken,
    saveProviderSettings,
    testAllProviders,
    testProviders,
    toggleProvider,
  };
}
