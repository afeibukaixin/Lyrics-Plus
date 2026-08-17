import type {
  ListLyricsPreferences,
  LyricsDisplayPreferences,
  LyricsModeStyleInheritance,
  LyricsStyleInheritance,
  LyricsStyleMode,
  NotchLyricsPreferences,
  OverlayFontWeight,
  StatusBarLyricsPreferences,
} from "../../shared/types";
import { ColorRow, RangeRow, SelectRow, SettingsSection, TextRow, ToggleRow } from "./components";
import { Button } from "@/components/ui/button";
import styles from "../settings.module.scss";
import { useTranslation } from "react-i18next";
import { useCallback, useEffect, useRef } from "react";
import { reportFrontendError } from "../../shared/debugLog";
import { emitNotchWidthPreview } from "../../shared/tauriEvent";

type AuxiliaryMode = Exclude<LyricsStyleMode, "desktop">;

type Props = {
  mode: AuxiliaryMode;
  displays: LyricsDisplayPreferences;
  inheritance: LyricsStyleInheritance;
  update: <Mode extends AuxiliaryMode>(mode: Mode, preferences: LyricsDisplayPreferences[Mode]) => Promise<void>;
  updateInheritance: (mode: LyricsStyleMode, inheritance: LyricsModeStyleInheritance) => Promise<void>;
  resetPosition: (mode: AuxiliaryMode) => Promise<void>;
};

function patchAppearance<T extends { appearance: object }>(preferences: T, patch: Partial<T["appearance"]>): T {
  return { ...preferences, appearance: { ...preferences.appearance, ...patch } };
}

type AuxiliarySectionLabels = {
  inheritance: string;
  displayPosition: string;
  text: string;
  backgroundSize: string;
  displayContent: string;
  textLayout: string;
  colors: string;
  displayInteraction: string;
};

export function auxiliarySections(mode: AuxiliaryMode, labels: AuxiliarySectionLabels) {
  if (mode === "statusBar") return [
    { id: "mode-inheritance", label: labels.inheritance },
    { id: "mode-state", label: labels.displayInteraction },
    { id: "mode-text", label: labels.text },
  ];
  if (mode === "listWindow") return [
    { id: "mode-inheritance", label: labels.inheritance },
    { id: "mode-state", label: labels.displayContent },
    { id: "mode-text", label: labels.textLayout },
    { id: "mode-colors", label: labels.colors },
  ];
  return [
    { id: "mode-inheritance", label: labels.inheritance },
    { id: "mode-state", label: labels.displayInteraction },
    { id: "mode-text", label: labels.text },
    { id: "mode-background", label: labels.backgroundSize },
  ];
}

export default function LyricsModeStyleSections({ mode, displays, inheritance, update, updateInheritance, resetPosition }: Props) {
  const { t } = useTranslation();
  const notchWidthPreviewActiveRef = useRef(false);
  const cancelNotchWidthPreview = useCallback(() => {
    if (!notchWidthPreviewActiveRef.current) return;
    notchWidthPreviewActiveRef.current = false;
    void emitNotchWidthPreview({ phase: "cancel" }).catch((error) => {
      reportFrontendError("Failed to cancel the Dynamic Island width preview", error);
    });
  }, []);

  useEffect(() => {
    if (mode !== "notch") cancelNotchWidthPreview();
  }, [cancelNotchWidthPreview, mode]);

  useEffect(() => () => cancelNotchWidthPreview(), [cancelNotchWidthPreview]);

  const fontWeights: Array<[string, string]> = [
    ["400", t("settings.overlay.fontWeightRegular")],
    ["500", t("settings.overlay.fontWeightMedium")],
    ["600", t("settings.overlay.fontWeightSemibold")],
    ["700", t("settings.overlay.fontWeightBold")],
    ["800", t("settings.overlay.fontWeightExtrabold")],
  ];
  const modeInheritance = inheritance[mode];
  const inheritanceSection = <SettingsSection id="mode-inheritance" title={t("settings.style.modeControls.inheritance")}>
    <ToggleRow label={t("settings.style.modeControls.inheritFontFamily")} value={modeInheritance.inheritFontFamily} onChange={(inheritFontFamily) => updateInheritance(mode, { ...modeInheritance, inheritFontFamily })} />
    <ToggleRow label={t("settings.style.modeControls.inheritColors")} value={modeInheritance.inheritColors} onChange={(inheritColors) => updateInheritance(mode, { ...modeInheritance, inheritColors })} />
  </SettingsSection>;
  if (mode === "statusBar") {
    const value = displays.statusBar;
    const appearance = value.appearance;
    const save = (next: StatusBarLyricsPreferences) => void update("statusBar", next);
    return <>
      {inheritanceSection}
      <SettingsSection id="mode-state" title={t("settings.style.modeControls.displayInteraction")}>
        <ToggleRow label={t("settings.display.statusBar.show")} value={value.enabled} onChange={(enabled) => save({ ...value, enabled })} />
        <ToggleRow label={t("settings.display.statusBar.autoHide")} description={t("settings.display.statusBar.autoHideHint")} value={value.hideWhenNotPlaying} onChange={(hideWhenNotPlaying) => save({ ...value, hideWhenNotPlaying })} />
        <RangeRow label={t("settings.display.statusBar.width")} description={t("settings.display.statusBar.widthHint")} value={appearance.width} min={120} max={360} step={5} suffix=" pt" onChange={(width) => save(patchAppearance(value, { width }))} />
      </SettingsSection>
      <SettingsSection id="mode-text" title={t("settings.style.modeControls.text")}>
        {!modeInheritance.inheritFontFamily && <TextRow label={t("settings.overlay.fontFamily")} value={appearance.fontFamily} emptyValue={appearance.fontFamily} onChange={(fontFamily) => save(patchAppearance(value, { fontFamily }))} />}
        <RangeRow label={t("settings.overlay.fontSize")} value={appearance.fontSize} min={10} max={18} suffix=" pt" onChange={(fontSize) => save(patchAppearance(value, { fontSize }))} />
        <SelectRow label={t("settings.overlay.fontWeight")} value={String(appearance.fontWeight)} options={fontWeights} onChange={(fontWeight) => save(patchAppearance(value, { fontWeight: Number(fontWeight) as OverlayFontWeight }))} />
        {!modeInheritance.inheritColors && <>
          <ColorRow label={t("settings.display.statusBar.textColor")} description={t("settings.display.statusBar.textColorHint")} value={appearance.textColor} onChange={(textColor) => save(patchAppearance(value, { textColor }))} />
          <ColorRow label={t("settings.display.statusBar.highlightColor")} description={t("settings.display.statusBar.highlightColorHint")} value={appearance.highlightColor} onChange={(highlightColor) => save(patchAppearance(value, { highlightColor }))} />
          <ColorRow label={t("settings.display.statusBar.inactiveColor")} description={t("settings.display.statusBar.inactiveColorHint")} value={appearance.inactiveColor} onChange={(inactiveColor) => save(patchAppearance(value, { inactiveColor }))} />
        </>}
      </SettingsSection>
    </>;
  }

  if (mode === "listWindow") {
    const value = displays.listWindow;
    const appearance = value.appearance;
    const save = (next: ListLyricsPreferences) => void update("listWindow", next);
    return <>
      {inheritanceSection}
      <SettingsSection id="mode-state" title={t("settings.style.modeControls.displayContent")}>
        <ToggleRow label={t("settings.display.listWindow.show")} value={value.enabled} onChange={(enabled) => save({ ...value, enabled })} />
        <ToggleRow label={t("settings.display.listWindow.translation")} value={value.showTranslation} onChange={(showTranslation) => save({ ...value, showTranslation })} />
        <ToggleRow label={t("settings.display.listWindow.romanization")} value={value.showRomanization} onChange={(showRomanization) => save({ ...value, showRomanization })} />
      </SettingsSection>
      <SettingsSection id="mode-text" title={t("settings.style.modeControls.textLayout")}>
        {!modeInheritance.inheritFontFamily && <TextRow label={t("settings.overlay.fontFamily")} value={appearance.fontFamily} emptyValue={appearance.fontFamily} onChange={(fontFamily) => save(patchAppearance(value, { fontFamily }))} />}
        <RangeRow label={t("settings.style.modeControls.mainFontSize")} value={appearance.fontSize} min={12} max={56} suffix="px" onChange={(fontSize) => save(patchAppearance(value, { fontSize }))} />
        <SelectRow label={t("settings.style.modeControls.mainFontWeight")} value={String(appearance.fontWeight)} options={fontWeights} onChange={(fontWeight) => save(patchAppearance(value, { fontWeight: Number(fontWeight) as OverlayFontWeight }))} />
        <RangeRow label={t("settings.style.modeControls.secondaryFontSize")} value={appearance.secondaryFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(appearance.secondaryFontScale * 100)} onChange={(secondaryFontScale) => save(patchAppearance(value, { secondaryFontScale }))} />
        <RangeRow label={t("settings.overlay.lineHeight")} value={appearance.lineHeight} min={0.8} max={2} step={0.05} suffix="×" onChange={(lineHeight) => save(patchAppearance(value, { lineHeight }))} />
        <RangeRow label={t("settings.style.modeControls.lineGap")} value={appearance.lineGap} min={0} max={32} suffix="px" onChange={(lineGap) => save(patchAppearance(value, { lineGap }))} />
        <SelectRow label={t("settings.style.modeControls.alignment")} value={appearance.alignment} options={[["left", t("settings.style.modeControls.left")], ["center", t("settings.style.modeControls.center")], ["right", t("settings.style.modeControls.right")]]} onChange={(alignment) => save(patchAppearance(value, { alignment: alignment as ListLyricsPreferences["appearance"]["alignment"] }))} />
      </SettingsSection>
      <SettingsSection id="mode-colors" title={t("settings.style.modeControls.colors")}>
        {!modeInheritance.inheritColors && <>
          <ColorRow label={t("settings.display.listWindow.activeColor")} description={t("settings.display.listWindow.activeColorHint")} value={appearance.activeColor} onChange={(activeColor) => save(patchAppearance(value, { activeColor }))} />
          <ColorRow label={t("settings.display.listWindow.inactiveColor")} description={t("settings.display.listWindow.inactiveColorHint")} value={appearance.inactiveColor} onChange={(inactiveColor) => save(patchAppearance(value, { inactiveColor }))} />
          <ColorRow label={t("settings.overlay.translationColor")} value={appearance.translationColor} onChange={(translationColor) => save(patchAppearance(value, { translationColor }))} />
          <ColorRow label={t("settings.overlay.romanizationColor")} value={appearance.romanizationColor} onChange={(romanizationColor) => save(patchAppearance(value, { romanizationColor }))} />
          <ColorRow label={t("settings.style.modeControls.windowBackground")} value={appearance.backgroundColor} onChange={(backgroundColor) => save(patchAppearance(value, { backgroundColor }))} />
        </>}
        <RangeRow label={t("settings.overlay.backgroundOpacity")} value={appearance.backgroundOpacity} min={0} max={1} step={0.05} suffix="%" displayValue={Math.round(appearance.backgroundOpacity * 100)} onChange={(backgroundOpacity) => save(patchAppearance(value, { backgroundOpacity }))} />
        <ColorRow label={t("settings.style.modeControls.activeBackground")} value={appearance.activeBackgroundColor} onChange={(activeBackgroundColor) => save(patchAppearance(value, { activeBackgroundColor }))} />
      </SettingsSection>
    </>;
  }

  const value = displays.notch;
  const appearance = value.appearance;
  const save = (next: NotchLyricsPreferences) => void update("notch", next);
  const previewMaxWidth = (width: number) => {
    notchWidthPreviewActiveRef.current = true;
    void emitNotchWidthPreview({ phase: "update", width }).catch((error) => {
      reportFrontendError("Failed to preview the Dynamic Island width", error);
    });
  };
  const commitMaxWidth = async (width: number) => {
    try {
      await update("notch", patchAppearance(value, { maxWidth: width }));
    } catch (error) {
      notchWidthPreviewActiveRef.current = false;
      void emitNotchWidthPreview({ phase: "cancel" }).catch((emitError) => {
        reportFrontendError("Failed to cancel the Dynamic Island width preview", emitError);
      });
      reportFrontendError("Failed to save the Dynamic Island width", error);
      throw error;
    }

    notchWidthPreviewActiveRef.current = false;
    try {
      await emitNotchWidthPreview({ phase: "commit", width });
    } catch (error) {
      void emitNotchWidthPreview({ phase: "cancel" });
      reportFrontendError("Failed to finish the Dynamic Island width preview", error);
    }
  };
  return <>
    {inheritanceSection}
    <SettingsSection id="mode-state" title={t("settings.style.modeControls.displayInteraction")}>
      <ToggleRow label={t("settings.display.notch.show")} value={value.enabled} onChange={(enabled) => save({ ...value, enabled })} />
      <ToggleRow label={t("settings.display.notch.autoHide")} description={t("settings.display.notch.autoHideHint")} value={value.hideWhenNotPlaying} onChange={(hideWhenNotPlaying) => save({ ...value, hideWhenNotPlaying })} />
      <ToggleRow label={t("settings.display.notch.showTwoLines")} value={value.showTwoLines} onChange={(showTwoLines) => save({ ...value, showTwoLines })} />
      <ToggleRow label={t("settings.display.notch.translation")} value={value.showTranslation} onChange={(showTranslation) => save({ ...value, showTranslation })} />
      <ToggleRow label={t("settings.display.notch.romanization")} value={value.showRomanization} onChange={(showRomanization) => save({ ...value, showRomanization })} />
      <div className={styles.buttonRow}><Button variant="secondary" size="sm" onClick={() => void resetPosition("notch")}>{t("settings.style.modeControls.resetPosition")}</Button></div>
    </SettingsSection>
    <SettingsSection id="mode-text" title={t("settings.style.modeControls.text")}>
      {!modeInheritance.inheritFontFamily && <TextRow label={t("settings.overlay.fontFamily")} value={appearance.fontFamily} emptyValue={appearance.fontFamily} onChange={(fontFamily) => save(patchAppearance(value, { fontFamily }))} />}
      <RangeRow label={t("settings.overlay.fontSize")} value={appearance.fontSize} min={12} max={32} suffix="px" onChange={(fontSize) => save(patchAppearance(value, { fontSize }))} />
      <SelectRow label={t("settings.overlay.fontWeight")} value={String(appearance.fontWeight)} options={fontWeights} onChange={(fontWeight) => save(patchAppearance(value, { fontWeight: Number(fontWeight) as OverlayFontWeight }))} />
      {!modeInheritance.inheritColors && <>
        <ColorRow label={t("settings.display.notch.activeColor")} description={t("settings.display.notch.activeColorHint")} value={appearance.activeColor} onChange={(activeColor) => save(patchAppearance(value, { activeColor }))} />
        <ColorRow label={t("settings.display.notch.inactiveColor")} description={t("settings.display.notch.inactiveColorHint")} value={appearance.inactiveColor} onChange={(inactiveColor) => save(patchAppearance(value, { inactiveColor }))} />
        <ColorRow label={t("settings.overlay.translationColor")} value={appearance.translationColor} onChange={(translationColor) => save(patchAppearance(value, { translationColor }))} />
        <ColorRow label={t("settings.overlay.romanizationColor")} value={appearance.romanizationColor} onChange={(romanizationColor) => save(patchAppearance(value, { romanizationColor }))} />
      </>}
    </SettingsSection>
    <SettingsSection id="mode-background" title={t("settings.style.modeControls.backgroundSize")}>
      <RangeRow label={t("settings.overlay.backgroundRadius")} value={appearance.borderRadius} min={0} max={40} suffix="px" onChange={(borderRadius) => save(patchAppearance(value, { borderRadius }))} />
      <RangeRow
        label={t("settings.style.modeControls.maxWidth")}
        value={appearance.maxWidth}
        min={400}
        max={640}
        step={10}
        suffix="px"
        onChange={(maxWidth) => save(patchAppearance(value, { maxWidth }))}
        onValuePreview={previewMaxWidth}
        onValueCommitted={commitMaxWidth}
        onPreviewCanceled={cancelNotchWidthPreview}
      />
    </SettingsSection>
  </>;
}
