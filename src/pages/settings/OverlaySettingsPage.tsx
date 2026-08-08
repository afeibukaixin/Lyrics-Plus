import { secondaryDisplayFlags, secondaryDisplayFromFlags, type OverlayStyle } from "../../shared/types";
import { useTranslation } from "react-i18next";
import { messageOf } from "../../shared/api";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { ColorRow, RangeRow, SelectRow, SettingsCard, SettingsHeading, ToggleRow } from "./components";

type OverlayColorValues = Pick<
  OverlayStyle,
  "activeColor" | "inactiveColor" | "translationColor" | "romanizationColor"
>;

type OverlayColorPreset = {
  id: "violet" | "ocean" | "mint" | "sunset" | "sakura" | "contrast";
  colors: OverlayColorValues;
};

const overlayColorPresets: OverlayColorPreset[] = [
  { id: "violet", colors: { activeColor: "#c4b5fd", inactiveColor: "#c8d2df", translationColor: "#cbd5e1", romanizationColor: "#aab7c8" } },
  { id: "ocean", colors: { activeColor: "#38bdf8", inactiveColor: "#dbeafe", translationColor: "#bae6fd", romanizationColor: "#93c5fd" } },
  { id: "mint", colors: { activeColor: "#5eead4", inactiveColor: "#d1fae5", translationColor: "#99f6e4", romanizationColor: "#a7f3d0" } },
  { id: "sunset", colors: { activeColor: "#fbbf24", inactiveColor: "#ffedd5", translationColor: "#fde68a", romanizationColor: "#fdba74" } },
  { id: "sakura", colors: { activeColor: "#fda4af", inactiveColor: "#fce7f3", translationColor: "#fbcfe8", romanizationColor: "#fecdd3" } },
  { id: "contrast", colors: { activeColor: "#ffffff", inactiveColor: "#cbd5e1", translationColor: "#e2e8f0", romanizationColor: "#94a3b8" } },
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
  const { t } = useTranslation();
  const {
    config,
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
    setOverlayHideWhenNotPlaying,
    resetSection,
    resetOverlayBounds,
  } = useSettingsContext();

  const secondaryFlags = secondaryDisplayFlags(style.secondaryDisplay);
  const lyricCapabilities = lyrics.document
    ? [
        lyrics.document.tracks.translation ? t("common.feature.hasTranslation") : t("common.feature.noTranslation"),
        lyrics.document.tracks.romanization ? t("common.feature.hasRomanization") : t("common.feature.noRomanization"),
        lyrics.document.tracks.original.lines.some((line) => line.words?.length) ? t("common.feature.hasWordTiming") : t("common.feature.noWordTiming"),
      ].join(" · ")
    : t("settings.common.capabilitiesHint");
  const secondaryLayoutHint = style.layout === "double"
    ? secondaryFlags.translation && secondaryFlags.romanization
      ? t("settings.overlay.doubleHint", { capabilities: lyricCapabilities })
      : lyricCapabilities
    : t("settings.overlay.singleHint", { capabilities: lyricCapabilities });
  const alignmentAvailable = style.layout === "double";
  const alignmentDescription = style.layout === "double" && style.orientation === "vertical"
    ? t("settings.overlay.alignmentVertical")
    : style.layout === "double"
      ? t("settings.overlay.alignmentHorizontal")
      : t("settings.overlay.alignmentFixed");
  const activeColorPreset = overlayColorPresets.find((preset) => matchesColorPreset(style, preset));

  const applyColorPreset = async (preset: OverlayColorPreset) => {
    setError(null);
    setNotice(null);
    const name = t(`settings.overlay.presets.${preset.id}`);
    if (await updateStyle(preset.colors)) setNotice(t("settings.overlay.colorApplied", { name }));
  };

  return (
    <>
      <SettingsHeading title={t("settings.overlay.title")} description={t("settings.overlay.description")} onReset={() => void resetSection("overlay")} resetting={resettingSection === "overlay"} confirming={confirmingReset === "overlay"} />
      <SettingsCard title={t("settings.overlay.state")}>
        <ToggleRow label={t("settings.overlay.show")} description={t("settings.overlay.showHint")} value={overlaySettings.visible} onChange={setVisible} />
        <ToggleRow
          label={t("settings.overlay.autoHide")}
          description={t("settings.overlay.autoHideHint")}
          value={config.overlay.hideWhenNotPlaying}
          onChange={(hidden) => setOverlayHideWhenNotPlaying(hidden).catch((value) => setError(messageOf(value)))}
        />
        <ToggleRow label={t("settings.overlay.lock")} description={t("settings.overlay.lockHint")} value={overlaySettings.locked} onChange={setLocked} />
        <div className={styles.buttonRow}><button onClick={() => void resetOverlayBounds()}>{t("settings.overlay.resetPosition")}</button></div>
      </SettingsCard>
      <SettingsCard title={t("settings.overlay.colors")} trailing={<span className={styles.colorPresetStatus}>{t("settings.overlay.currentColor", { name: activeColorPreset ? t(`settings.overlay.presets.${activeColorPreset.id}`) : t("settings.overlay.custom") })}</span>}>
        <div className={styles.colorPresetGrid}>
          {overlayColorPresets.map((preset) => {
            const active = preset.id === activeColorPreset?.id;
            return (
              <button type="button" className={styles.colorPresetButton} data-active={active} aria-label={t("settings.overlay.applyColor", { name: t(`settings.overlay.presets.${preset.id}`) })} aria-pressed={active} key={preset.id} onClick={() => void applyColorPreset(preset)}>
                <span className={styles.colorPresetPreview} aria-hidden="true">{overlayColorKeys.map((key) => <i key={key} style={{ background: preset.colors[key] }} />)}</span>
                <strong>{t(`settings.overlay.presets.${preset.id}`)}</strong>
              </button>
            );
          })}
        </div>
      </SettingsCard>
      <SettingsCard title={t("settings.overlay.textEffects")}>
        <RangeRow label={t("settings.overlay.fontSize")} value={style.fontSize} min={16} max={72} suffix="px" onChange={(fontSize) => void updateStyle({ fontSize })} />
        <RangeRow label={t("settings.overlay.opacity")} value={style.opacity} min={0.2} max={1} step={0.05} suffix="%" displayValue={Math.round(style.opacity * 100)} onChange={(opacity) => void updateStyle({ opacity })} />
        <ColorRow label={t("settings.overlay.activeColor")} value={style.activeColor} onChange={(activeColor) => void updateStyle({ activeColor })} />
        <ColorRow label={t("settings.overlay.inactiveColor")} value={style.inactiveColor} onChange={(inactiveColor) => void updateStyle({ inactiveColor })} />
        <SelectRow label={t("settings.overlay.karaoke")} value={style.karaokeStyle} onChange={(karaokeStyle) => void updateStyle({ karaokeStyle: karaokeStyle as OverlayStyle["karaokeStyle"] })} options={[["sweep", t("settings.overlay.karaokeSweep")], ["bounce", t("settings.overlay.karaokeBounce")], ["highlight", t("settings.overlay.karaokeHighlight")]]} />
      </SettingsCard>
      <SettingsCard title={t("settings.overlay.backgroundLayout")}>
        <SelectRow label={t("settings.overlay.backgroundMode")} value={style.backgroundMode} onChange={(backgroundMode) => void updateStyle({ backgroundMode: backgroundMode as OverlayStyle["backgroundMode"] })} options={[["solid", t("settings.overlay.solid")], ["transparent", t("settings.overlay.transparent")]]} />
        {style.backgroundMode === "solid" && (
          <>
            <RangeRow label={t("settings.overlay.backgroundOpacity")} value={style.backgroundOpacity} min={0} max={1} step={0.05} suffix="%" displayValue={Math.round(style.backgroundOpacity * 100)} onChange={(backgroundOpacity) => void updateStyle({ backgroundOpacity })} />
            <ColorRow label={t("settings.overlay.backgroundColor")} value={style.solidColor} onChange={(solidColor) => void updateStyle({ solidColor })} />
            <ToggleRow label={t("settings.overlay.glass")} description={t("settings.overlay.glassHint")} value={style.background === "glass"} onChange={(enabled) => updateStyle({ background: enabled ? "glass" : "solid" })} />
            {style.background === "glass" && <RangeRow label={t("settings.overlay.blur")} value={style.backgroundBlur} min={0} max={40} suffix="%" displayValue={Math.round(style.backgroundBlur / 40 * 100)} onChange={(backgroundBlur) => void updateStyle({ backgroundBlur })} />}
          </>
        )}
        <SelectRow label={t("settings.overlay.lyricLayout")} value={style.layout} onChange={(value) => void updateStyle({ layout: value as OverlayStyle["layout"] })} options={[["single", t("overlay.layout.single")], ["double", t("overlay.layout.double")]]} />
        <SelectRow label={t("settings.overlay.textDirection")} value={style.orientation} onChange={(value) => void updateStyle({ orientation: value as OverlayStyle["orientation"] })} options={[["horizontal", t("overlay.orientation.horizontal")], ["vertical", t("overlay.orientation.vertical")]]} />
        <SelectRow label={t("settings.overlay.alignment")} description={alignmentDescription} disabled={!alignmentAvailable} value={alignmentAvailable ? style.alignment : "center"} onChange={(alignment) => void updateStyle({ alignment: alignment as OverlayStyle["alignment"] })} options={[["center", t("settings.overlay.centered")], ["distributed", t("settings.overlay.distributed")]]} />
        <SelectRow label={t("settings.overlay.longLyrics")} value={style.longText} onChange={(longText) => void updateStyle({ longText: longText as OverlayStyle["longText"] })} options={[["shrink", t("settings.overlay.shrink")], ["wrap", t("settings.overlay.wrap")], ["marquee", t("settings.overlay.marquee")]]} />
      </SettingsCard>
      <SettingsCard title={t("settings.overlay.secondary")}>
        <RangeRow label={t("settings.overlay.secondarySize")} value={style.secondaryFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(style.secondaryFontScale * 100)} onChange={(secondaryFontScale) => void updateStyle({ secondaryFontScale })} />
        <ToggleRow label={t("settings.overlay.showTranslation")} description={secondaryLayoutHint} value={secondaryFlags.translation} onChange={(translation) => updateStyle({ secondaryDisplay: secondaryDisplayFromFlags(translation, secondaryFlags.romanization) })} />
        <RangeRow label={t("settings.overlay.translationSize")} value={style.translationFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(style.translationFontScale * 100)} onChange={(translationFontScale) => void updateStyle({ translationFontScale })} />
        <ColorRow label={t("settings.overlay.translationColor")} value={style.translationColor} onChange={(translationColor) => void updateStyle({ translationColor })} />
        <ToggleRow label={t("settings.overlay.showRomanization")} description={secondaryLayoutHint} value={secondaryFlags.romanization} onChange={(romanization) => updateStyle({ secondaryDisplay: secondaryDisplayFromFlags(secondaryFlags.translation, romanization) })} />
        <RangeRow label={t("settings.overlay.romanizationSize")} value={style.romanizationFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(style.romanizationFontScale * 100)} onChange={(romanizationFontScale) => void updateStyle({ romanizationFontScale })} />
        <ColorRow label={t("settings.overlay.romanizationColor")} value={style.romanizationColor} onChange={(romanizationColor) => void updateStyle({ romanizationColor })} />
        <ToggleRow label={t("settings.overlay.autoCenter")} description={t("settings.overlay.autoCenterHint")} value={style.autoCenterWithTranslationOrRomanization} onChange={(autoCenterWithTranslationOrRomanization) => updateStyle({ autoCenterWithTranslationOrRomanization })} />
      </SettingsCard>
    </>
  );
}
