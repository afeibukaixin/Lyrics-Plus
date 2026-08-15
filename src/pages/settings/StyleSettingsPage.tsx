import { secondaryDisplayFlags, secondaryDisplayFromFlags, type OverlayStyle } from "../../shared/types";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown } from "lucide-react";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { ColorRow, PageHeader, RangeRow, SelectRow, SettingsSection, ToggleRow } from "./components";
import { Button } from "@/components/ui/button";

type OverlayColorValues = Pick<
  OverlayStyle,
  "activeColor" | "inactiveColor" | "translationColor" | "romanizationColor"
>;

type OverlayColorPreset = {
  id: "violet" | "ocean" | "mint" | "sunset" | "sakura" | "contrast" | "aurora" | "indigo" | "coral" | "lime" | "moonlight" | "neon";
  colors: OverlayColorValues;
};

const overlayColorPresets: OverlayColorPreset[] = [
  { id: "violet", colors: { activeColor: "#c4b5fd", inactiveColor: "#c8d2df", translationColor: "#cbd5e1", romanizationColor: "#aab7c8" } },
  { id: "ocean", colors: { activeColor: "#38bdf8", inactiveColor: "#dbeafe", translationColor: "#bae6fd", romanizationColor: "#93c5fd" } },
  { id: "mint", colors: { activeColor: "#5eead4", inactiveColor: "#d1fae5", translationColor: "#99f6e4", romanizationColor: "#a7f3d0" } },
  { id: "sunset", colors: { activeColor: "#fbbf24", inactiveColor: "#ffedd5", translationColor: "#fde68a", romanizationColor: "#fdba74" } },
  { id: "sakura", colors: { activeColor: "#fda4af", inactiveColor: "#fce7f3", translationColor: "#fbcfe8", romanizationColor: "#fecdd3" } },
  { id: "contrast", colors: { activeColor: "#ffffff", inactiveColor: "#cbd5e1", translationColor: "#e2e8f0", romanizationColor: "#94a3b8" } },
  { id: "aurora", colors: { activeColor: "#22d3ee", inactiveColor: "#ccfbf1", translationColor: "#a7f3d0", romanizationColor: "#c4b5fd" } },
  { id: "indigo", colors: { activeColor: "#818cf8", inactiveColor: "#e0e7ff", translationColor: "#c7d2fe", romanizationColor: "#a5b4fc" } },
  { id: "coral", colors: { activeColor: "#fb7185", inactiveColor: "#ffedd5", translationColor: "#fed7aa", romanizationColor: "#fecdd3" } },
  { id: "lime", colors: { activeColor: "#a3e635", inactiveColor: "#ecfccb", translationColor: "#d9f99d", romanizationColor: "#bef264" } },
  { id: "moonlight", colors: { activeColor: "#f8fafc", inactiveColor: "#dbeafe", translationColor: "#e0e7ff", romanizationColor: "#cbd5e1" } },
  { id: "neon", colors: { activeColor: "#e879f9", inactiveColor: "#cffafe", translationColor: "#67e8f9", romanizationColor: "#c4b5fd" } },
];

const featuredColorPresetCount = 6;

const overlayColorKeys: Array<keyof OverlayColorValues> = [
  "activeColor",
  "inactiveColor",
  "translationColor",
  "romanizationColor",
];

function matchesColorPreset(style: OverlayStyle, preset: OverlayColorPreset) {
  return overlayColorKeys.every((key) => style[key].trim().toLowerCase() === preset.colors[key].toLowerCase());
}

export default function StyleSettingsPage() {
  const { t } = useTranslation();
  const {
    style,
    resettingSection,
    confirmingReset,
    setError,
    setNotice,
    updateStyle,
    resetSection,
  } = useSettingsContext();
  const [colorPresetsExpanded, setColorPresetsExpanded] = useState(() =>
    overlayColorPresets.slice(featuredColorPresetCount).some((preset) => matchesColorPreset(style, preset)),
  );

  const secondaryFlags = secondaryDisplayFlags(style.secondaryDisplay);
  const alignmentAvailable = style.layout === "double";
  const activeColorPreset = overlayColorPresets.find((preset) => matchesColorPreset(style, preset));
  const visibleColorPresets = colorPresetsExpanded ? overlayColorPresets : overlayColorPresets.slice(0, featuredColorPresetCount);

  const applyColorPreset = async (preset: OverlayColorPreset) => {
    setError(null);
    setNotice(null);
    const name = t(`settings.overlay.presets.${preset.id}`);
    if (await updateStyle(preset.colors)) setNotice(t("settings.overlay.colorApplied", { name }));
  };

  return (
    <>
      <PageHeader title={t("settings.style.title")} description={t("settings.style.description")} onReset={() => void resetSection("style")} resetting={resettingSection === "style"} confirming={confirmingReset === "style"} />
      <SettingsSection title={t("settings.overlay.colors")} trailing={<span className={styles.colorPresetStatus}>{t("settings.overlay.currentColor", { name: activeColorPreset ? t(`settings.overlay.presets.${activeColorPreset.id}`) : t("settings.overlay.custom") })}</span>}>
        <div className={styles.colorPresetGrid} id="overlay-color-presets">
          {visibleColorPresets.map((preset) => {
            const active = preset.id === activeColorPreset?.id;
            return <Button type="button" variant="outline" className={styles.colorPresetButton} data-active={active} aria-pressed={active} key={preset.id} onClick={() => void applyColorPreset(preset)}>
              <span className={styles.colorPresetPreview} aria-hidden="true">{overlayColorKeys.map((key) => <i key={key} style={{ background: preset.colors[key] }} />)}</span>
              <strong>{t(`settings.overlay.presets.${preset.id}`)}</strong>
            </Button>;
          })}
        </div>
        <div className={styles.colorPresetActions}>
          <Button type="button" variant="ghost" size="sm" aria-controls="overlay-color-presets" aria-expanded={colorPresetsExpanded} onClick={() => setColorPresetsExpanded((expanded) => !expanded)}>
            {t(colorPresetsExpanded ? "settings.overlay.showFewerColors" : "settings.overlay.showMoreColors")}
            <ChevronDown className={styles.colorPresetChevron} data-expanded={colorPresetsExpanded} data-icon="inline-end" aria-hidden="true" />
          </Button>
        </div>
      </SettingsSection>
      <SettingsSection title={t("settings.style.common")}>
        <RangeRow label={t("settings.overlay.fontSize")} value={style.fontSize} min={16} max={72} suffix="px" onChange={(fontSize) => void updateStyle({ fontSize })} />
        <ColorRow label={t("settings.overlay.activeColor")} value={style.activeColor} onChange={(activeColor) => void updateStyle({ activeColor })} />
        <ColorRow label={t("settings.overlay.inactiveColor")} value={style.inactiveColor} onChange={(inactiveColor) => void updateStyle({ inactiveColor })} />
        <SelectRow label={t("settings.overlay.backgroundMode")} value={style.backgroundMode} onChange={(backgroundMode) => void updateStyle({ backgroundMode: backgroundMode as OverlayStyle["backgroundMode"] })} options={[["solid", t("settings.overlay.solid")], ["transparent", t("settings.overlay.transparent")]]} />
        <SelectRow label={t("settings.overlay.lyricLayout")} value={style.layout} onChange={(layout) => void updateStyle({ layout: layout as OverlayStyle["layout"] })} options={[["single", t("overlay.layout.single")], ["double", t("overlay.layout.double")]]} />
        <SelectRow label={t("settings.overlay.textDirection")} value={style.orientation} onChange={(orientation) => void updateStyle({ orientation: orientation as OverlayStyle["orientation"] })} options={[["horizontal", t("overlay.orientation.horizontal")], ["vertical", t("overlay.orientation.vertical")]]} />
        <ToggleRow label={t("settings.overlay.showTranslation")} value={secondaryFlags.translation} onChange={(translation) => updateStyle({ secondaryDisplay: secondaryDisplayFromFlags(translation, secondaryFlags.romanization) })} />
        <ToggleRow label={t("settings.overlay.showRomanization")} value={secondaryFlags.romanization} onChange={(romanization) => updateStyle({ secondaryDisplay: secondaryDisplayFromFlags(secondaryFlags.translation, romanization) })} />
        <SelectRow label={t("settings.overlay.karaoke")} value={style.karaokeStyle} onChange={(karaokeStyle) => void updateStyle({ karaokeStyle: karaokeStyle as OverlayStyle["karaokeStyle"] })} options={[["sweep", t("settings.overlay.karaokeSweep")], ["bounce", t("settings.overlay.karaokeBounce")], ["highlight", t("settings.overlay.karaokeHighlight")]]} />
      </SettingsSection>
      <SettingsSection title={t("settings.overlay.backgroundLayout")}>
        {style.backgroundMode !== "solid"
          ? <p className={styles.cardHint}>{t("settings.overlay.backgroundControlsHint")}</p>
          : style.background !== "glass"
            ? <p className={styles.cardHint}>{t("settings.overlay.glassControlsHint")}</p>
            : null}
        <RangeRow label={t("settings.overlay.opacity")} value={style.opacity} min={0.2} max={1} step={0.05} suffix="%" displayValue={Math.round(style.opacity * 100)} onChange={(opacity) => void updateStyle({ opacity })} />
        <RangeRow label={t("settings.overlay.backgroundOpacity")} disabled={style.backgroundMode !== "solid"} value={style.backgroundOpacity} min={0} max={1} step={0.05} suffix="%" displayValue={Math.round(style.backgroundOpacity * 100)} onChange={(backgroundOpacity) => void updateStyle({ backgroundOpacity })} />
        <ColorRow label={t("settings.overlay.backgroundColor")} disabled={style.backgroundMode !== "solid"} value={style.solidColor} onChange={(solidColor) => void updateStyle({ solidColor })} />
        <ToggleRow label={t("settings.overlay.glass")} disabled={style.backgroundMode !== "solid"} value={style.background === "glass"} onChange={(enabled) => updateStyle({ background: enabled ? "glass" : "solid" })} />
        <RangeRow label={t("settings.overlay.blur")} disabled={style.backgroundMode !== "solid" || style.background !== "glass"} value={style.backgroundBlur} min={0} max={40} suffix="%" onChange={(backgroundBlur) => void updateStyle({ backgroundBlur })} />
        <SelectRow label={t("settings.overlay.longLyrics")} value={style.longText} onChange={(longText) => void updateStyle({ longText: longText as OverlayStyle["longText"] })} options={[["shrink", t("settings.overlay.shrink")], ["wrap", t("settings.overlay.wrap")], ["marquee", t("settings.overlay.marquee")]]} />
        <SelectRow label={t("settings.overlay.alignment")} description={!alignmentAvailable ? t("settings.overlay.requiresDoubleLayout") : undefined} disabled={!alignmentAvailable} value={alignmentAvailable ? style.alignment : "center"} onChange={(alignment) => void updateStyle({ alignment: alignment as OverlayStyle["alignment"] })} options={[["center", t("settings.overlay.centered")], ["distributed", t("settings.overlay.distributed")]]} />
      </SettingsSection>
      <SettingsSection title={t("settings.overlay.secondary")}>
        {(!secondaryFlags.translation || !secondaryFlags.romanization) && <p className={styles.cardHint}>{t("settings.overlay.secondaryControlsHint")}</p>}
        <RangeRow label={t("settings.overlay.secondarySize")} value={style.secondaryFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(style.secondaryFontScale * 100)} onChange={(secondaryFontScale) => void updateStyle({ secondaryFontScale })} />
        <RangeRow label={t("settings.overlay.translationSize")} disabled={!secondaryFlags.translation} value={style.translationFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(style.translationFontScale * 100)} onChange={(translationFontScale) => void updateStyle({ translationFontScale })} />
        <ColorRow label={t("settings.overlay.translationColor")} disabled={!secondaryFlags.translation} value={style.translationColor} onChange={(translationColor) => void updateStyle({ translationColor })} />
        <RangeRow label={t("settings.overlay.romanizationSize")} disabled={!secondaryFlags.romanization} value={style.romanizationFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(style.romanizationFontScale * 100)} onChange={(romanizationFontScale) => void updateStyle({ romanizationFontScale })} />
        <ColorRow label={t("settings.overlay.romanizationColor")} disabled={!secondaryFlags.romanization} value={style.romanizationColor} onChange={(romanizationColor) => void updateStyle({ romanizationColor })} />
        <ToggleRow label={t("settings.overlay.autoCenter")} value={style.autoCenterWithTranslationOrRomanization} onChange={(autoCenterWithTranslationOrRomanization) => updateStyle({ autoCenterWithTranslationOrRomanization })} />
      </SettingsSection>
    </>
  );
}
