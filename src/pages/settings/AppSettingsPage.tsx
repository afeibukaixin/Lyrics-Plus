import type { PlayerSelection } from "../../shared/types";
import { messageOf } from "../../shared/api";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { RangeRow, SettingsCard, SettingsHeading, ToggleRow } from "./components";

const playerOptions: Array<{ value: PlayerSelection; label: string }> = [
  { value: "auto", label: "自动选择" },
  { value: "apple_music", label: "Apple Music" },
  { value: "spotify", label: "Spotify" },
];

export default function AppSettingsPage() {
  const {
    config,
    setUiFontScale,
    setDockIconHidden,
    playback,
    lyrics,
    resettingSection,
    confirmingReset,
    setError,
    resetSection,
  } = useSettingsContext();

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
        <div className={styles.shortcutRow}><span>显示 / 隐藏桌面歌词</span><kbd>⌘ ⇧ L</kbd></div>
        <div className={styles.shortcutRow}><span>解锁桌面歌词</span><kbd>⌘ ⇧ U</kbd></div>
        <div className={styles.shortcutRow}><span>复位并显示桌面歌词</span><kbd>⌘ ⇧ 0</kbd></div>
      </SettingsCard>
      <SettingsCard title="诊断"><div className={styles.diagnostics} data-error={Boolean(playback.commandError || playback.snapshot.error || lyrics.error)}><i /><span>{diagnostics}</span></div></SettingsCard>
    </>
  );
}
