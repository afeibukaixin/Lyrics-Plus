import { defaultOverlayStyle, secondaryDisplayFlags, secondaryDisplayFromFlags, type LyricsBaseAppearance, type LyricsStyleMode, type OverlayStyle } from "../../shared/types";
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router";
import { useTranslation } from "react-i18next";
import { ChevronDown, ListMusic, Monitor, Palette, PanelTop, PanelTopDashed } from "lucide-react";
import { api, messageOf } from "../../shared/api";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { ColorRow, PageHeader, RangeRow, SelectRow, SettingsPage, SettingsSection, TextRow, ToggleRow } from "./components";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { cn } from "@/lib/utils";
import LyricsModeStyleSections, { auxiliarySections } from "./LyricsModeStyleSections";

type OverlayColorValues = Pick<
  OverlayStyle,
  "activeColor" | "inactiveColor" | "translationColor" | "romanizationColor"
>;

type OverlayColorPreset = {
  id: "lime" | "sky" | "aurora" | "lavender" | "rose" | "contrast" | "amber" | "emerald" | "indigo" | "coral" | "moonlight" | "neon";
  colors: OverlayColorValues;
};

const overlayColorPresets: OverlayColorPreset[] = [
  { id: "lime", colors: { activeColor: "#a3e635", inactiveColor: "#ecfccb", translationColor: "#d9f99d", romanizationColor: "#bef264" } },
  { id: "sky", colors: { activeColor: "#38bdf8", inactiveColor: "#dbeafe", translationColor: "#bae6fd", romanizationColor: "#93c5fd" } },
  { id: "aurora", colors: { activeColor: "#22d3ee", inactiveColor: "#ccfbf1", translationColor: "#99f6e4", romanizationColor: "#a5f3fc" } },
  { id: "lavender", colors: { activeColor: "#a78bfa", inactiveColor: "#ede9fe", translationColor: "#ddd6fe", romanizationColor: "#c4b5fd" } },
  { id: "rose", colors: { activeColor: "#fb7185", inactiveColor: "#fce7f3", translationColor: "#fbcfe8", romanizationColor: "#fecdd3" } },
  { id: "contrast", colors: { activeColor: "#ffffff", inactiveColor: "#cbd5e1", translationColor: "#e2e8f0", romanizationColor: "#94a3b8" } },
  { id: "amber", colors: { activeColor: "#fbbf24", inactiveColor: "#fffbeb", translationColor: "#fde68a", romanizationColor: "#fdba74" } },
  { id: "emerald", colors: { activeColor: "#34d399", inactiveColor: "#d1fae5", translationColor: "#a7f3d0", romanizationColor: "#6ee7b7" } },
  { id: "indigo", colors: { activeColor: "#818cf8", inactiveColor: "#e0e7ff", translationColor: "#c7d2fe", romanizationColor: "#a5b4fc" } },
  { id: "coral", colors: { activeColor: "#fb923c", inactiveColor: "#ffedd5", translationColor: "#fed7aa", romanizationColor: "#fdba74" } },
  { id: "moonlight", colors: { activeColor: "#f8fafc", inactiveColor: "#dbeafe", translationColor: "#e0e7ff", romanizationColor: "#cbd5e1" } },
  { id: "neon", colors: { activeColor: "#e879f9", inactiveColor: "#cffafe", translationColor: "#67e8f9", romanizationColor: "#c4b5fd" } },
];

const featuredColorPresetCount = 3;

const overlayColorKeys: Array<keyof OverlayColorValues> = [
  "activeColor",
  "inactiveColor",
  "translationColor",
  "romanizationColor",
];
const baseColorKeys: Array<keyof Omit<LyricsBaseAppearance, "fontFamily">> = [
  ...overlayColorKeys,
  "supportingColor",
  "backgroundColor",
];

type StyleMode = "base" | LyricsStyleMode;

function styleModeFromQuery(value: string | null): StyleMode {
  return value === "desktop" || value === "statusBar" || value === "listWindow" || value === "notch"
    ? value
    : "base";
}

function baseColorsForPreset(preset: OverlayColorPreset): Omit<LyricsBaseAppearance, "fontFamily"> {
  return {
    ...preset.colors,
    supportingColor: preset.colors.romanizationColor,
    backgroundColor: "#171821",
  };
}

function matchesColorPreset(style: LyricsBaseAppearance, preset: OverlayColorPreset) {
  const colors = baseColorsForPreset(preset);
  return baseColorKeys.every((key) => style[key].trim().toLowerCase() === colors[key].toLowerCase());
}

export default function StyleSettingsPage() {
  const { t } = useTranslation();
  const [searchParams, setSearchParams] = useSearchParams();
  const requestedMode = searchParams.get("mode");
  const {
    style,
    config,
    overlaySettings,
    resettingSection,
    confirmingReset,
    setError,
    setNotice,
    updateStyle,
    setVisible,
    setLocked,
    setOverlayHideWhenNotPlaying,
    setLyricsDisplayPreferences,
    setLyricsBaseAppearance,
    setLyricsStyleInheritance,
    resetLyricsBaseAppearance,
    resetOverlayBounds,
    resetSection,
  } = useSettingsContext();
  const [colorPresetsExpanded, setColorPresetsExpanded] = useState(false);
  const [mode, setMode] = useState<StyleMode>(() => styleModeFromQuery(requestedMode));
  const [resettingMode, setResettingMode] = useState<StyleMode | null>(null);

  useEffect(() => {
    setMode(styleModeFromQuery(requestedMode));
  }, [requestedMode]);

  const selectMode = (next: StyleMode) => {
    setMode(next);
    setSearchParams(next === "base" ? {} : { mode: next }, { replace: true });
  };

  const secondaryFlags = secondaryDisplayFlags(style.secondaryDisplay);
  const alignmentAvailable = style.layout === "double";
  const baseAppearance = config.lyrics.baseAppearance;
  const desktopInheritance = config.lyrics.styleInheritance.desktop;
  const activeColorPreset = overlayColorPresets.find((preset) => matchesColorPreset(baseAppearance, preset));
  const visibleColorPresets = colorPresetsExpanded ? overlayColorPresets : overlayColorPresets.slice(0, featuredColorPresetCount);
  const fontWeightOptions: Array<[string, string]> = [
    ["400", t("settings.overlay.fontWeightRegular")],
    ["500", t("settings.overlay.fontWeightMedium")],
    ["600", t("settings.overlay.fontWeightSemibold")],
    ["700", t("settings.overlay.fontWeightBold")],
    ["800", t("settings.overlay.fontWeightExtrabold")],
  ];
  const desktopSections = [
    { id: "style-state", label: t("settings.overlay.state") },
    { id: "mode-inheritance", label: t("settings.style.modeControls.inheritance") },
    { id: "style-text-effects", label: t("settings.overlay.textEffects") },
    { id: "style-common", label: t("settings.style.common") },
    { id: "style-background-layout", label: t("settings.overlay.backgroundLayout") },
    { id: "style-secondary", label: t("settings.overlay.secondary") },
  ];
  const baseSections = [
    { id: "base-presets", label: t("settings.overlay.colors") },
    { id: "base-font", label: t("settings.style.modeControls.baseFont") },
    { id: "base-colors", label: t("settings.style.modeControls.baseColors") },
  ];
  const sections = mode === "base" ? baseSections : mode === "desktop" ? desktopSections : auxiliarySections(mode, {
    inheritance: t("settings.style.modeControls.inheritance"),
    displayPosition: t("settings.style.modeControls.displayPosition"),
    text: t("settings.style.modeControls.text"),
    backgroundSize: t("settings.style.modeControls.backgroundSize"),
    displayContent: t("settings.style.modeControls.displayContent"),
    textLayout: t("settings.style.modeControls.textLayout"),
    colors: t("settings.style.modeControls.colors"),
    displayInteraction: t("settings.style.modeControls.displayInteraction"),
  });
  const modes: Array<{ id: StyleMode; label: string; icon: typeof Monitor }> = [
    { id: "base", label: t("settings.style.modes.base"), icon: Palette },
    { id: "desktop", label: t("settings.style.modes.desktop"), icon: Monitor },
    { id: "statusBar", label: t("settings.style.modes.statusBar"), icon: PanelTop },
    { id: "listWindow", label: t("settings.style.modes.listWindow"), icon: ListMusic },
    { id: "notch", label: t("settings.style.modes.notch"), icon: PanelTopDashed },
  ];

  const applyColorPreset = async (preset: OverlayColorPreset) => {
    setError(null);
    setNotice(null);
    const name = t(`settings.overlay.presets.${preset.id}`);
    try {
      await setLyricsBaseAppearance({ ...baseAppearance, ...baseColorsForPreset(preset) });
      setNotice(t("settings.overlay.colorApplied", { name }));
    } catch (error) {
      setError(messageOf(error));
    }
  };

  const resetCurrentMode = async () => {
    if (mode === "base") {
      setResettingMode(mode);
      try {
        await resetLyricsBaseAppearance();
        setNotice(t("settings.shell.resetDone", { section: t("settings.style.modes.base") }));
      } catch (error) {
        setError(messageOf(error));
      } finally {
        setResettingMode(null);
      }
      return;
    }
    if (mode === "desktop") {
      await resetSection("style");
      return;
    }
    setResettingMode(mode);
    setError(null);
    try {
      await api.resetLyricsStyleMode(mode);
      setNotice(t("settings.shell.resetDone", { section: modes.find((item) => item.id === mode)?.label ?? t("settings.style.title") }));
    } catch (error) {
      setError(messageOf(error));
    } finally {
      setResettingMode(null);
    }
  };

  const resetDisplayPosition = async (target: "statusBar" | "listWindow" | "notch") => {
    try {
      await api.resetLyricsDisplayPosition(target);
      setNotice(t("settings.style.modeControls.positionReset"));
    } catch (error) {
      setError(messageOf(error));
    }
  };

  return (
    <SettingsPage sections={sections}>
      <PageHeader title={t("settings.style.title")} description={t("settings.style.description")} onReset={() => void resetCurrentMode()} resetting={mode === "desktop" ? resettingSection === "style" : resettingMode === mode} confirming={mode === "desktop" && confirmingReset === "style"} />
      <ToggleGroup className={cn(styles.lyricsModeSelector, "grid w-full")} variant="outline" aria-label={t("settings.style.modes.selector")} value={[mode]} onValueChange={(values) => { const next = values[0] as StyleMode | undefined; if (next) selectMode(next); }}>
        {modes.map((item) => {
          const Icon = item.icon;
          return <ToggleGroupItem key={item.id} value={item.id} aria-label={item.label}>
            <Icon aria-hidden="true" /><span>{item.label}</span>
          </ToggleGroupItem>;
        })}
      </ToggleGroup>
      {mode === "base" ? <>
      <SettingsSection id="base-presets" title={t("settings.overlay.colors")} trailing={<span className={styles.colorPresetStatus}>{t("settings.overlay.currentColor", { name: activeColorPreset ? t(`settings.overlay.presets.${activeColorPreset.id}`) : t("settings.overlay.custom") })}</span>}>
        <div className={styles.colorPresetGrid} id="base-color-presets">
          {visibleColorPresets.map((preset) => {
            const active = preset.id === activeColorPreset?.id;
            return <Button type="button" variant="outline" className={styles.colorPresetButton} data-active={active} aria-pressed={active} key={preset.id} onClick={() => void applyColorPreset(preset)}>
              <span className={styles.colorPresetPreview} aria-hidden="true">{overlayColorKeys.map((key) => <i key={key} style={{ background: preset.colors[key] }} />)}</span>
              <strong>{t(`settings.overlay.presets.${preset.id}`)}</strong>
            </Button>;
          })}
        </div>
        <div className={styles.colorPresetActions}>
          <Button type="button" variant="ghost" size="sm" aria-controls="base-color-presets" aria-expanded={colorPresetsExpanded} onClick={() => setColorPresetsExpanded((expanded) => !expanded)}>
            {t(colorPresetsExpanded ? "settings.overlay.showFewerColors" : "settings.overlay.showMoreColors")}
            <ChevronDown className={styles.colorPresetChevron} data-expanded={colorPresetsExpanded} data-icon="inline-end" aria-hidden="true" />
          </Button>
        </div>
      </SettingsSection>
      <SettingsSection id="base-font" title={t("settings.style.modeControls.baseFont")}>
        <TextRow label={t("settings.overlay.fontFamily")} description={t("settings.overlay.fontFamilyHint")} value={baseAppearance.fontFamily} emptyValue={defaultOverlayStyle.fontFamily} onChange={(fontFamily) => void setLyricsBaseAppearance({ ...baseAppearance, fontFamily }).catch((error) => setError(messageOf(error)))} />
      </SettingsSection>
      <SettingsSection id="base-colors" title={t("settings.style.modeControls.baseColors")}>
        <ColorRow label={t("settings.style.modeControls.baseActiveColor")} description={t("settings.style.modeControls.baseActiveColorHint")} value={baseAppearance.activeColor} onChange={(activeColor) => void setLyricsBaseAppearance({ ...baseAppearance, activeColor }).catch((error) => setError(messageOf(error)))} />
        <ColorRow label={t("settings.style.modeControls.baseInactiveColor")} description={t("settings.style.modeControls.baseInactiveColorHint")} value={baseAppearance.inactiveColor} onChange={(inactiveColor) => void setLyricsBaseAppearance({ ...baseAppearance, inactiveColor }).catch((error) => setError(messageOf(error)))} />
        <ColorRow label={t("settings.overlay.translationColor")} description={t("settings.overlay.translationColorHint")} value={baseAppearance.translationColor} onChange={(translationColor) => void setLyricsBaseAppearance({ ...baseAppearance, translationColor }).catch((error) => setError(messageOf(error)))} />
        <ColorRow label={t("settings.overlay.romanizationColor")} description={t("settings.overlay.romanizationColorHint")} value={baseAppearance.romanizationColor} onChange={(romanizationColor) => void setLyricsBaseAppearance({ ...baseAppearance, romanizationColor }).catch((error) => setError(messageOf(error)))} />
        <ColorRow label={t("settings.overlay.backgroundColor")} value={baseAppearance.backgroundColor} onChange={(backgroundColor) => void setLyricsBaseAppearance({ ...baseAppearance, backgroundColor }).catch((error) => setError(messageOf(error)))} />
      </SettingsSection>
      </> : mode === "desktop" ? <>
      <SettingsSection id="style-state" title={t("settings.overlay.state")}>
        <ToggleRow label={t("settings.overlay.show")} description={t("settings.overlay.showHint")} value={overlaySettings.visible} onChange={setVisible} />
        <ToggleRow label={t("settings.overlay.autoHide")} description={t("settings.overlay.autoHideHint")} value={config.overlay.hideWhenNotPlaying} onChange={(hidden) => setOverlayHideWhenNotPlaying(hidden).catch((error) => setError(messageOf(error)))} />
        <ToggleRow label={t("settings.overlay.lock")} value={overlaySettings.locked} onChange={setLocked} />
        <div className={styles.buttonRow}><Button variant="secondary" size="sm" onClick={() => void resetOverlayBounds()}>{t("settings.overlay.resetPosition")}</Button></div>
      </SettingsSection>
      <SettingsSection id="mode-inheritance" title={t("settings.style.modeControls.inheritance")}>
        <ToggleRow label={t("settings.style.modeControls.inheritFontFamily")} value={desktopInheritance.inheritFontFamily} onChange={(inheritFontFamily) => setLyricsStyleInheritance("desktop", { ...desktopInheritance, inheritFontFamily })} />
        <ToggleRow label={t("settings.style.modeControls.inheritColors")} value={desktopInheritance.inheritColors} onChange={(inheritColors) => setLyricsStyleInheritance("desktop", { ...desktopInheritance, inheritColors })} />
      </SettingsSection>
      <SettingsSection id="style-text-effects" title={t("settings.overlay.textEffects")}>
        {!desktopInheritance.inheritFontFamily && <TextRow label={t("settings.overlay.fontFamily")} description={t("settings.overlay.fontFamilyHint")} value={style.fontFamily} emptyValue={defaultOverlayStyle.fontFamily} onChange={(fontFamily) => void updateStyle({ fontFamily })} />}
        <RangeRow label={t("settings.overlay.fontSize")} value={style.fontSize} min={16} max={72} suffix="px" onChange={(fontSize) => void updateStyle({ fontSize })} />
        <SelectRow label={t("settings.overlay.fontWeight")} value={String(style.fontWeight)} onChange={(fontWeight) => void updateStyle({ fontWeight: Number(fontWeight) as OverlayStyle["fontWeight"] })} options={fontWeightOptions} />
        <SelectRow label={t("settings.overlay.secondaryFontWeight")} value={String(style.secondaryFontWeight)} onChange={(secondaryFontWeight) => void updateStyle({ secondaryFontWeight: Number(secondaryFontWeight) as OverlayStyle["secondaryFontWeight"] })} options={fontWeightOptions} />
        <RangeRow label={t("settings.overlay.lineHeight")} value={style.lineHeight} min={0.8} max={2} step={0.05} suffix="×" onChange={(lineHeight) => void updateStyle({ lineHeight })} />
        <RangeRow label={t("settings.overlay.textShadowOffsetX")} value={style.textShadowOffsetX} min={-20} max={20} suffix="px" onChange={(textShadowOffsetX) => void updateStyle({ textShadowOffsetX })} />
        <RangeRow label={t("settings.overlay.textShadowOffsetY")} value={style.textShadowOffsetY} min={-20} max={20} suffix="px" onChange={(textShadowOffsetY) => void updateStyle({ textShadowOffsetY })} />
        <RangeRow label={t("settings.overlay.textShadowBlur")} value={style.textShadowBlur} min={0} max={40} suffix="px" onChange={(textShadowBlur) => void updateStyle({ textShadowBlur })} />
        <ColorRow label={t("settings.overlay.textShadowColor")} description={t("settings.overlay.textShadowColorHint")} value={style.textShadowColor} onChange={(textShadowColor) => void updateStyle({ textShadowColor })} />
      </SettingsSection>
      <SettingsSection id="style-common" title={t("settings.style.common")}>
        {!desktopInheritance.inheritColors && <>
          <ColorRow label={t("settings.style.modeControls.desktopActiveColor")} description={t("settings.style.modeControls.desktopActiveColorHint")} value={style.activeColor} onChange={(activeColor) => void updateStyle({ activeColor })} />
          <ColorRow label={t("settings.style.modeControls.desktopInactiveColor")} description={t("settings.style.modeControls.desktopInactiveColorHint")} value={style.inactiveColor} onChange={(inactiveColor) => void updateStyle({ inactiveColor })} />
        </>}
        <SelectRow label={t("settings.overlay.backgroundMode")} value={style.backgroundMode} onChange={(backgroundMode) => void updateStyle({ backgroundMode: backgroundMode as OverlayStyle["backgroundMode"] })} options={[["solid", t("settings.overlay.solid")], ["transparent", t("settings.overlay.transparent")]]} />
        <SelectRow label={t("settings.overlay.lyricLayout")} value={style.layout} onChange={(layout) => void updateStyle({ layout: layout as OverlayStyle["layout"] })} options={[["single", t("overlay.layout.single")], ["double", t("overlay.layout.double")]]} />
        <SelectRow label={t("settings.overlay.textDirection")} value={style.orientation} onChange={(orientation) => void updateStyle({ orientation: orientation as OverlayStyle["orientation"] })} options={[["horizontal", t("overlay.orientation.horizontal")], ["vertical", t("overlay.orientation.vertical")]]} />
        <ToggleRow label={t("settings.overlay.showTranslation")} value={secondaryFlags.translation} onChange={(translation) => updateStyle({ secondaryDisplay: secondaryDisplayFromFlags(translation, secondaryFlags.romanization) })} />
        <ToggleRow label={t("settings.overlay.showRomanization")} value={secondaryFlags.romanization} onChange={(romanization) => updateStyle({ secondaryDisplay: secondaryDisplayFromFlags(secondaryFlags.translation, romanization) })} />
        <SelectRow label={t("settings.overlay.karaoke")} value={style.karaokeStyle} onChange={(karaokeStyle) => void updateStyle({ karaokeStyle: karaokeStyle as OverlayStyle["karaokeStyle"] })} options={[["sweep", t("settings.overlay.karaokeSweep")], ["bounce", t("settings.overlay.karaokeBounce")], ["highlight", t("settings.overlay.karaokeHighlight")]]} />
      </SettingsSection>
      <SettingsSection id="style-background-layout" title={t("settings.overlay.backgroundLayout")}>
        {style.backgroundMode !== "solid"
          ? <p className={styles.cardHint}>{t("settings.overlay.backgroundControlsHint")}</p>
          : style.background !== "glass"
            ? <p className={styles.cardHint}>{t("settings.overlay.glassControlsHint")}</p>
            : null}
        <RangeRow label={t("settings.overlay.opacity")} value={style.opacity} min={0.2} max={1} step={0.05} suffix="%" displayValue={Math.round(style.opacity * 100)} onChange={(opacity) => void updateStyle({ opacity })} />
        <RangeRow label={t("settings.overlay.backgroundOpacity")} disabled={style.backgroundMode !== "solid"} value={style.backgroundOpacity} min={0} max={1} step={0.05} suffix="%" displayValue={Math.round(style.backgroundOpacity * 100)} onChange={(backgroundOpacity) => void updateStyle({ backgroundOpacity })} />
        {!desktopInheritance.inheritColors && <ColorRow label={t("settings.overlay.backgroundColor")} disabled={style.backgroundMode !== "solid"} value={style.solidColor} onChange={(solidColor) => void updateStyle({ solidColor })} />}
        <ToggleRow label={t("settings.overlay.glass")} disabled={style.backgroundMode !== "solid"} value={style.background === "glass"} onChange={(enabled) => updateStyle({ background: enabled ? "glass" : "solid" })} />
        <RangeRow label={t("settings.overlay.blur")} disabled={style.backgroundMode !== "solid" || style.background !== "glass"} value={style.backgroundBlur} min={0} max={40} suffix="%" onChange={(backgroundBlur) => void updateStyle({ backgroundBlur })} />
        <RangeRow label={t("settings.overlay.backgroundRadius")} value={style.backgroundRadius} min={0} max={64} suffix="px" onChange={(backgroundRadius) => void updateStyle({ backgroundRadius })} />
        <RangeRow label={t("settings.overlay.backgroundPaddingX")} value={style.backgroundPaddingX} min={0} max={64} suffix="px" onChange={(backgroundPaddingX) => void updateStyle({ backgroundPaddingX })} />
        <RangeRow label={t("settings.overlay.backgroundPaddingY")} value={style.backgroundPaddingY} min={0} max={64} suffix="px" onChange={(backgroundPaddingY) => void updateStyle({ backgroundPaddingY })} />
        <SelectRow label={t("settings.overlay.longLyrics")} value={style.longText} onChange={(longText) => void updateStyle({ longText: longText as OverlayStyle["longText"] })} options={[["shrink", t("settings.overlay.shrink")], ["wrap", t("settings.overlay.wrap")], ["marquee", t("settings.overlay.marquee")]]} />
        <SelectRow label={t("settings.overlay.alignment")} description={!alignmentAvailable ? t("settings.overlay.requiresDoubleLayout") : undefined} disabled={!alignmentAvailable} value={alignmentAvailable ? style.alignment : "center"} onChange={(alignment) => void updateStyle({ alignment: alignment as OverlayStyle["alignment"] })} options={[["center", t("settings.overlay.centered")], ["distributed", t("settings.overlay.distributed")]]} />
      </SettingsSection>
      <SettingsSection id="style-secondary" title={t("settings.overlay.secondary")}>
        {(!secondaryFlags.translation || !secondaryFlags.romanization) && <p className={styles.cardHint}>{t("settings.overlay.secondaryControlsHint")}</p>}
        <RangeRow label={t("settings.overlay.secondarySize")} value={style.secondaryFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(style.secondaryFontScale * 100)} onChange={(secondaryFontScale) => void updateStyle({ secondaryFontScale })} />
        <RangeRow label={t("settings.overlay.translationSize")} disabled={!secondaryFlags.translation} value={style.translationFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(style.translationFontScale * 100)} onChange={(translationFontScale) => void updateStyle({ translationFontScale })} />
        {!desktopInheritance.inheritColors && <ColorRow label={t("settings.overlay.translationColor")} disabled={!secondaryFlags.translation} value={style.translationColor} onChange={(translationColor) => void updateStyle({ translationColor })} />}
        <RangeRow label={t("settings.overlay.romanizationSize")} disabled={!secondaryFlags.romanization} value={style.romanizationFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(style.romanizationFontScale * 100)} onChange={(romanizationFontScale) => void updateStyle({ romanizationFontScale })} />
        {!desktopInheritance.inheritColors && <ColorRow label={t("settings.overlay.romanizationColor")} disabled={!secondaryFlags.romanization} value={style.romanizationColor} onChange={(romanizationColor) => void updateStyle({ romanizationColor })} />}
        <ToggleRow label={t("settings.overlay.autoCenter")} value={style.autoCenterWithTranslationOrRomanization} onChange={(autoCenterWithTranslationOrRomanization) => updateStyle({ autoCenterWithTranslationOrRomanization })} />
      </SettingsSection>
      </> : <LyricsModeStyleSections mode={mode} displays={config.lyrics.displays} inheritance={config.lyrics.styleInheritance} update={setLyricsDisplayPreferences} updateInheritance={setLyricsStyleInheritance} resetPosition={resetDisplayPosition} />}
    </SettingsPage>
  );
}
