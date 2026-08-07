import { useState } from "react";
import { defaultGlobalShortcuts, type GlobalShortcutSettings, type PlayerSelection } from "../../shared/types";
import { messageOf } from "../../shared/api";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { RangeRow, SettingsCard, SettingsHeading, ToggleRow } from "./components";

const playerOptions: Array<{ value: PlayerSelection; label: string }> = [
  { value: "auto", label: "自动选择" },
  { value: "apple_music", label: "Apple Music" },
  { value: "spotify", label: "Spotify" },
];

type ShortcutAction = keyof GlobalShortcutSettings;

const shortcutLabels: Array<[ShortcutAction, string]> = [
  ["toggleOverlay", "显示 / 隐藏桌面歌词"],
  ["unlockOverlay", "解锁桌面歌词"],
  ["resetOverlay", "复位并显示桌面歌词"],
];

function shortcutDisplay(value: string) {
  const mac = navigator.userAgent.includes("Mac");
  const labels: Record<string, string> = {
    commandorcontrol: mac ? "⌘" : "Ctrl",
    commandorctrl: mac ? "⌘" : "Ctrl",
    super: mac ? "⌘" : "Super",
    control: "Ctrl",
    ctrl: "Ctrl",
    shift: "⇧",
    alt: mac ? "⌥" : "Alt",
    option: "⌥",
  };
  return value.split("+").map((token) => {
    const normalized = token.toLowerCase();
    if (labels[normalized]) return labels[normalized];
    return token.replace(/^Key/, "").replace(/^Digit/, "");
  }).join(" ");
}

function shortcutFromEvent(event: React.KeyboardEvent<HTMLButtonElement>) {
  if (["Meta", "Control", "Alt", "Shift"].includes(event.key)) return null;
  const modifiers = [
    event.metaKey ? "Super" : null,
    event.ctrlKey ? "Control" : null,
    event.altKey ? "Alt" : null,
    event.shiftKey ? "Shift" : null,
  ].filter((value): value is string => Boolean(value));
  if (modifiers.length === 0 || !event.code || event.code === "Unidentified") return null;
  return [...modifiers, event.code].join("+");
}

export default function AppSettingsPage() {
  const {
    config,
    setUiFontScale,
    setGlobalShortcuts,
    setDockIconHidden,
    playback,
    lyrics,
    resettingSection,
    confirmingReset,
    setError,
    resetSection,
  } = useSettingsContext();
  const [recording, setRecording] = useState<ShortcutAction | null>(null);
  const [savingShortcut, setSavingShortcut] = useState(false);

  const saveShortcut = async (action: ShortcutAction, value: string) => {
    setSavingShortcut(true);
    setError(null);
    try {
      await setGlobalShortcuts({ ...config.app.shortcuts, [action]: value });
      setRecording(null);
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setSavingShortcut(false);
    }
  };

  const diagnostics = playback.commandError
    ?? playback.snapshot.error
    ?? lyrics.error
    ?? (lyrics.document
      ? `歌词来源：${lyrics.document.metadata.source} · ${lyrics.document.tracks.original.lines.length} 行`
      : "当前没有已关联歌词");

  return (
    <>
      <SettingsHeading title="应用" description="选择播放器并管理主界面、菜单栏与快捷键。" onReset={() => void resetSection("app")} resetting={resettingSection === "app"} confirming={confirmingReset === "app"} />
      <SettingsCard title="播放器">
        <div className={styles.playerOptions}>{playerOptions.map((option) => <button key={option.value} data-active={playback.selection === option.value} onClick={() => playback.setSelection(option.value)}>{option.label}</button>)}</div>
      </SettingsCard>
      <SettingsCard title="主界面显示">
        <RangeRow label="主界面字号" value={config.app.uiFontScale} min={80} max={150} step={10} suffix="%" onChange={(scale) => void setUiFontScale(scale).catch((value) => setError(messageOf(value)))} />
        <p className={styles.cardHint}>只放大首页、设置和歌词库的文字；窗口尺寸、控件和桌面歌词不受影响。</p>
      </SettingsCard>
      <SettingsCard title="Dock 与菜单栏">
        <ToggleRow label="隐藏 Dock 图标和运行指示点" description="隐藏后仍可通过菜单栏图标打开 Lyrics Plus" value={config.app.hideDockIcon} onChange={(hidden) => setDockIconHidden(hidden).catch((value) => setError(messageOf(value)))} />
      </SettingsCard>
      <SettingsCard title="快捷键">
        <div className={styles.shortcutRow}><span>打开设置</span><kbd>⌘ ,</kbd></div>
        {shortcutLabels.map(([action, label]) => {
          const isRecording = recording === action;
          const isDefault = config.app.shortcuts[action] === defaultGlobalShortcuts[action];
          return (
            <div className={styles.shortcutRow} key={action}>
              <span>{label}</span>
              <div className={styles.shortcutControls}>
                <button
                  autoFocus={isRecording}
                  className={styles.shortcutRecorder}
                  data-recording={isRecording}
                  disabled={savingShortcut}
                  key={isRecording ? "recording" : "idle"}
                  onClick={() => setRecording(isRecording ? null : action)}
                  onKeyDown={(event) => {
                    if (!isRecording) return;
                    event.preventDefault();
                    event.stopPropagation();
                    if (event.key === "Escape") {
                      setRecording(null);
                      return;
                    }
                    const shortcut = shortcutFromEvent(event);
                    if (shortcut) void saveShortcut(action, shortcut);
                  }}
                >{isRecording ? "请按组合键 · Esc 取消" : shortcutDisplay(config.app.shortcuts[action])}</button>
                <button className={styles.shortcutReset} disabled={savingShortcut || isDefault} onClick={() => void saveShortcut(action, defaultGlobalShortcuts[action])}>恢复默认</button>
              </div>
            </div>
          );
        })}
      </SettingsCard>
      <SettingsCard title="诊断"><div className={styles.diagnostics} data-error={Boolean(playback.commandError || playback.snapshot.error || lyrics.error)}><i /><span>{diagnostics}</span></div></SettingsCard>
    </>
  );
}
