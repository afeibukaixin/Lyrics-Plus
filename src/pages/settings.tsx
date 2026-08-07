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
import { Link, NavLink, Outlet, useOutletContext } from "react-router";
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
  setUiFontScale: ReturnType<typeof useAppConfig>["setUiFontScale"];
  setDockIconHidden: ReturnType<typeof useAppConfig>["setDockIconHidden"];
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
  saveProviderSettings: (settings: ProviderSettings) => Promise<void>;
  beginProviderDrag: (providerId: string, sourceIndex: number, event: ReactPointerEvent<HTMLButtonElement>) => void;
  continueProviderDrag: (event: ReactPointerEvent<HTMLButtonElement>) => void;
  finishProviderDrag: (event: ReactPointerEvent<HTMLButtonElement>) => void;
  setProviderDrag: Dispatch<SetStateAction<ProviderDragState | null>>;
  providerDragTransform: (index: number) => string | undefined;
  toggleProvider: (id: string) => void;
  testProvider: (providerId: string) => Promise<void>;
  handleFile: (file?: File) => Promise<void>;
  resetSection: (target: SettingsSection) => Promise<void>;
  resetOverlayBounds: () => Promise<void>;
  syncAppliedConfig: (imported: Parameters<ReturnType<typeof useAppConfig>["syncConfig"]>[0], appearanceOnly: boolean) => Promise<void>;
};

export default function Settings() {
  const { config, setUiFontScale, setDockIconHidden, syncConfig } = useAppConfig();
  const playback = usePlayback();
  const lyrics = useLyrics(playback.snapshot, playback.positionMs, false);
  const fileInput = useRef<HTMLInputElement>(null);
  const resetConfirmTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const providerRows = useRef(new Map<string, HTMLDivElement>());
  const [overlaySettings, setOverlaySettings] = useState<OverlaySettings>({ visible: true, locked: false, passthrough: false });
  const [style, setStyle] = useState<OverlayStyle>(defaultOverlayStyle);
  const [providerView, setProviderView] = useState<ProviderSettingsView | null>(null);
  const [testingProvider, setTestingProvider] = useState<string | null>(null);
  const [resettingSection, setResettingSection] = useState<SettingsSection | null>(null);
  const [confirmingReset, setConfirmingReset] = useState<SettingsSection | null>(null);
  const [providerDrag, setProviderDrag] = useState<ProviderDragState | null>(null);
  const [savingProviderOrder, setSavingProviderOrder] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

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

  useEffect(() => () => {
    if (resetConfirmTimer.current !== null) clearTimeout(resetConfirmTimer.current);
  }, []);

  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(null), 3200);
    return () => clearTimeout(timer);
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
    const next = { ...style, ...patch };
    setStyle(next);
    try {
      setStyle(await api.setOverlayStyle(next));
      return true;
    } catch (value) {
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
      setOverlaySettings((current) => ({ ...current, locked, passthrough: locked }));
    } catch (value) { setError(messageOf(value)); }
  };

  const saveProviderSettings = async (settings: ProviderSettings) => {
    try { setProviderView(await api.setProviderSettings(settings)); } catch (value) { setError(messageOf(value)); }
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

  const testProvider = async (providerId: string) => {
    setTestingProvider(providerId);
    try {
      const status = await api.testProvider(providerId);
      setProviderView((current) => current ? { ...current, statuses: current.statuses.map((item) => item.providerId === providerId ? status : item) } : current);
    } catch (value) { setError(messageOf(value)); } finally { setTestingProvider(null); }
  };

  const handleFile = async (file?: File) => {
    if (!file) return;
    await lyrics.importRaw(await file.text());
    if (fileInput.current) fileInput.current.value = "";
  };

  const resetSection = async (target: SettingsSection) => {
    const names: Record<SettingsSection, string> = {
      overlay: "桌面歌词",
      lyrics: "歌词与搜索",
      app: "应用",
    };
    if (confirmingReset !== target) {
      if (resetConfirmTimer.current !== null) clearTimeout(resetConfirmTimer.current);
      setConfirmingReset(target);
      setNotice(`再次点击“恢复默认”以确认恢复${names[target]}；歌词库和歌曲关联不会删除。`);
      resetConfirmTimer.current = setTimeout(() => {
        resetConfirmTimer.current = null;
        setConfirmingReset(null);
      }, 4000);
      return;
    }
    if (resetConfirmTimer.current !== null) clearTimeout(resetConfirmTimer.current);
    resetConfirmTimer.current = null;
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
      setNotice(`${names[target]}已恢复默认。`);
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
      setNotice("桌面歌词已复位并重新显示。");
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
    setUiFontScale,
    setDockIconHidden,
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
    testProvider,
    handleFile,
    resetSection,
    resetOverlayBounds,
    syncAppliedConfig,
  };

  return (
    <main className={styles.shell}>
      <header className={styles.header}>
        <div><Link to="/">← 返回首页</Link><h1>设置</h1></div>
        <Link className={styles.libraryLink} to="/library">歌词库</Link>
      </header>

      <div className={styles.settingsLayout}>
        <nav className={styles.sidebar} aria-label="设置分类">
          <NavLink to="/settings/overlay"><span>◫</span><div><strong>桌面歌词</strong><small>外观、布局与浮窗</small></div></NavLink>
          <NavLink to="/settings/lyrics"><span>≋</span><div><strong>歌词与搜索</strong><small>来源、关联与偏移</small></div></NavLink>
          <NavLink to="/settings/app"><span>⚙</span><div><strong>应用</strong><small>播放器与快捷键</small></div></NavLink>
          <NavLink to="/settings/debug"><span>⌁</span><div><strong>调试日志</strong><small>实时日志与筛选</small></div></NavLink>
          <NavLink to="/settings/config"><span>{"{}"}</span><div><strong>配置</strong><small>JSONC 编辑与分享</small></div></NavLink>
        </nav>

        <div className={styles.content}><Outlet context={context} /></div>
      </div>
      {(error || notice) && <div className={styles.toast} data-error={Boolean(error)}>{error ?? notice}<button aria-label="关闭" onClick={() => { setError(null); setNotice(null); }}>×</button></div>}
    </main>
  );
}

export function useSettingsContext() {
  return useOutletContext<SettingsOutletContext>();
}
