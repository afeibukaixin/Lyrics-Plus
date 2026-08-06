import { secondaryDisplayFlags, secondaryDisplayFromFlags, type OverlayStyle } from "../../shared/types";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { ColorRow, RangeRow, SelectRow, SettingsCard, SettingsHeading, ToggleRow } from "./components";

type OverlayColorValues = Pick<
  OverlayStyle,
  "activeColor" | "inactiveColor" | "translationColor" | "romanizationColor"
>;

type OverlayColorPreset = {
  id: string;
  name: string;
  colors: OverlayColorValues;
};

const overlayColorPresets: OverlayColorPreset[] = [
  { id: "violet", name: "紫罗兰", colors: { activeColor: "#c4b5fd", inactiveColor: "#c8d2df", translationColor: "#cbd5e1", romanizationColor: "#aab7c8" } },
  { id: "ocean", name: "海洋蓝", colors: { activeColor: "#38bdf8", inactiveColor: "#dbeafe", translationColor: "#bae6fd", romanizationColor: "#93c5fd" } },
  { id: "mint", name: "薄荷青", colors: { activeColor: "#5eead4", inactiveColor: "#d1fae5", translationColor: "#99f6e4", romanizationColor: "#a7f3d0" } },
  { id: "sunset", name: "日落橙", colors: { activeColor: "#fbbf24", inactiveColor: "#ffedd5", translationColor: "#fde68a", romanizationColor: "#fdba74" } },
  { id: "sakura", name: "樱花粉", colors: { activeColor: "#fda4af", inactiveColor: "#fce7f3", translationColor: "#fbcfe8", romanizationColor: "#fecdd3" } },
  { id: "contrast", name: "黑白高对比", colors: { activeColor: "#ffffff", inactiveColor: "#cbd5e1", translationColor: "#e2e8f0", romanizationColor: "#94a3b8" } },
];

const overlayColorKeys: Array<keyof OverlayColorValues> = [
  "activeColor",
  "inactiveColor",
  "translationColor",
  "romanizationColor",
];

function matchesColorPreset(style: OverlayStyle, preset: OverlayColorPreset) {
  return overlayColorKeys.every((key) => style[key].trim().toLowerCase() === preset.colors[key].toLowerCase());
}

export default function OverlaySettingsPage() {
  const {
    overlaySettings,
    style,
    lyrics,
    resettingSection,
    confirmingReset,
    setError,
    setNotice,
    updateStyle,
    setVisible,
    setLocked,
    resetSection,
    resetOverlayBounds,
  } = useSettingsContext();

  const secondaryFlags = secondaryDisplayFlags(style.secondaryDisplay);
  const lyricCapabilities = lyrics.document
    ? [
        lyrics.document.tracks.translation ? "有翻译" : "无翻译",
        lyrics.document.tracks.romanization ? "有音译" : "无音译",
        lyrics.document.tracks.original.lines.some((line) => line.words?.length) ? "有逐字时间轴" : "无逐字时间轴",
      ].join(" · ")
    : "关联歌词后会显示翻译、音译和逐字时间轴的可用状态";
  const secondaryLayoutHint = style.layout === "double"
    ? secondaryFlags.translation && secondaryFlags.romanization
      ? `同时开启时优先显示翻译，无翻译时显示音译 · ${lyricCapabilities}`
      : lyricCapabilities
    : `当前布局不显示副歌词 · ${lyricCapabilities}`;
  const alignmentAvailable = style.layout === "double";
  const alignmentDescription = style.layout === "double" && style.orientation === "vertical"
    ? "主副分居会将右侧主歌词靠上、左侧副歌词靠下"
    : style.layout === "double"
      ? "主副分居会将主歌词靠左、副歌词靠右"
      : "当前布局固定居中";
  const activeColorPreset = overlayColorPresets.find((preset) => matchesColorPreset(style, preset));

  const applyColorPreset = async (preset: OverlayColorPreset) => {
    setError(null);
    setNotice(null);
    if (await updateStyle(preset.colors)) setNotice(`已应用「${preset.name}」配色。`);
  };

  return (
    <>
      <SettingsHeading title="桌面歌词" description="横排宽度、竖排高度由边缘拖动设定；解锁后拖动空白区域可移动浮窗。" onReset={() => void resetSection("overlay")} resetting={resettingSection === "overlay"} confirming={confirmingReset === "overlay"} />
      <SettingsCard title="浮窗状态">
        <ToggleRow label="显示桌面歌词" description="在所有桌面空间置顶显示" value={overlaySettings.visible} onChange={setVisible} />
        <ToggleRow label="锁定并鼠标穿透" description="锁定后点击会穿透到下方窗口" value={overlaySettings.locked} onChange={setLocked} />
        <div className={styles.buttonRow}><button onClick={() => void resetOverlayBounds()}>复位并显示桌面歌词</button></div>
      </SettingsCard>
      <SettingsCard title="快捷配色" trailing={<span className={styles.colorPresetStatus}>当前：{activeColorPreset?.name ?? "自定义"}</span>}>
        <div className={styles.colorPresetGrid}>
          {overlayColorPresets.map((preset) => {
            const active = preset.id === activeColorPreset?.id;
            return (
              <button type="button" className={styles.colorPresetButton} data-active={active} aria-label={`应用${preset.name}配色`} aria-pressed={active} key={preset.id} onClick={() => void applyColorPreset(preset)}>
                <span className={styles.colorPresetPreview} aria-hidden="true">{overlayColorKeys.map((key) => <i key={key} style={{ background: preset.colors[key] }} />)}</span>
                <strong>{preset.name}</strong>
              </button>
            );
          })}
        </div>
      </SettingsCard>
      <SettingsCard title="文字与效果">
        <RangeRow label="字号" value={style.fontSize} min={16} max={72} suffix="px" onChange={(fontSize) => void updateStyle({ fontSize })} />
        <RangeRow label="透明度" value={style.opacity} min={0.2} max={1} step={0.05} suffix="%" displayValue={Math.round(style.opacity * 100)} onChange={(opacity) => void updateStyle({ opacity })} />
        <ColorRow label="高亮颜色" value={style.activeColor} onChange={(activeColor) => void updateStyle({ activeColor })} />
        <ColorRow label="未唱颜色" value={style.inactiveColor} onChange={(inactiveColor) => void updateStyle({ inactiveColor })} />
        <SelectRow label="卡拉 OK 效果" value={style.karaokeStyle} onChange={(karaokeStyle) => void updateStyle({ karaokeStyle: karaokeStyle as OverlayStyle["karaokeStyle"] })} options={[["sweep", "逐词扫光"], ["bounce", "逐词弹跳"], ["highlight", "纯高亮"]]} />
      </SettingsCard>
      <SettingsCard title="背景与排版">
        <SelectRow label="背景" value={style.background} onChange={(background) => void updateStyle({ background: background as OverlayStyle["background"] })} options={[["glass", "毛玻璃"], ["transparent", "纯透明"], ["solid", "纯色"]]} />
        {style.background === "solid" && <ColorRow label="背景颜色" value={style.solidColor} onChange={(solidColor) => void updateStyle({ solidColor })} />}
        <SelectRow label="歌词布局" value={style.layout} onChange={(value) => void updateStyle({ layout: value as OverlayStyle["layout"] })} options={[["single", "单歌词"], ["double", "双歌词"]]} />
        <SelectRow label="文字方向" value={style.orientation} onChange={(value) => void updateStyle({ orientation: value as OverlayStyle["orientation"] })} options={[["horizontal", "横排"], ["vertical", "竖排"]]} />
        <SelectRow label="歌词对齐" description={alignmentDescription} disabled={!alignmentAvailable} value={alignmentAvailable ? style.alignment : "center"} onChange={(alignment) => void updateStyle({ alignment: alignment as OverlayStyle["alignment"] })} options={[["center", "居中"], ["distributed", "主副分居"]]} />
        <SelectRow label="长歌词" value={style.longText} onChange={(longText) => void updateStyle({ longText: longText as OverlayStyle["longText"] })} options={[["shrink", "智能缩放"], ["wrap", "自动换行"], ["marquee", "超出时滚动"]]} />
      </SettingsCard>
      <SettingsCard title="翻译与音译">
        <ToggleRow label="显示翻译" description={secondaryLayoutHint} value={secondaryFlags.translation} onChange={(translation) => updateStyle({ secondaryDisplay: secondaryDisplayFromFlags(translation, secondaryFlags.romanization) })} />
        <RangeRow label="翻译字号" value={style.translationFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(style.translationFontScale * 100)} onChange={(translationFontScale) => void updateStyle({ translationFontScale })} />
        <ColorRow label="翻译颜色" value={style.translationColor} onChange={(translationColor) => void updateStyle({ translationColor })} />
        <ToggleRow label="显示音译" description={secondaryLayoutHint} value={secondaryFlags.romanization} onChange={(romanization) => updateStyle({ secondaryDisplay: secondaryDisplayFromFlags(secondaryFlags.translation, romanization) })} />
        <RangeRow label="音译字号" value={style.romanizationFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(style.romanizationFontScale * 100)} onChange={(romanizationFontScale) => void updateStyle({ romanizationFontScale })} />
        <ColorRow label="音译颜色" value={style.romanizationColor} onChange={(romanizationColor) => void updateStyle({ romanizationColor })} />
        <ToggleRow label="显示翻译或音译时自动居中" description="仅在当前行实际显示翻译或音译时居中；回退显示下一句时仍使用歌词对齐设置" value={style.autoCenterWithTranslationOrRomanization} onChange={(autoCenterWithTranslationOrRomanization) => updateStyle({ autoCenterWithTranslationOrRomanization })} />
      </SettingsCard>
    </>
  );
}
