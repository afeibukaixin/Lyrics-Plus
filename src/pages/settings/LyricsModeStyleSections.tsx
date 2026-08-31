import type {
  CompactKaraokeStyle,
  ListLyricsPreferences,
  LyricsDisplayPreferences,
  LyricsModeStyleInheritance,
  LyricsMonitor,
  LyricsStyleInheritance,
  LyricsStyleMode,
  NotchSlotContent,
  NotchLyricsPreferences,
  OverlayFontWeight,
  StatusBarLyricsPreferences,
} from "../../shared/types";
import { ColorRow, RangePairRow, RangeRow, SelectRow, SettingsSection, TextRow, ToggleRow } from "./components";
import { Button } from "@/components/ui/button";
import styles from "../settings.module.scss";
import { useTranslation } from "react-i18next";
import { useCallback, useEffect, useRef, useState } from "react";
import { reportFrontendError } from "../../shared/debugLog";
import { emitNotchWidthPreview } from "../../shared/tauriEvent";
import { api, isTauriRuntime } from "../../shared/api";

type AuxiliaryMode = Exclude<LyricsStyleMode, "desktop">;

type Props = {
  mode: AuxiliaryMode;
  displays: LyricsDisplayPreferences;
  inheritance: LyricsStyleInheritance;
  update: <Mode extends AuxiliaryMode>(mode: Mode, preferences: LyricsDisplayPreferences[Mode]) => Promise<void>;
  setListLyricsLocked: (locked: boolean) => Promise<void>;
  updateInheritance: (mode: LyricsStyleMode, inheritance: LyricsModeStyleInheritance) => Promise<void>;
  resetPosition: (mode: AuxiliaryMode) => Promise<void>;
};

function patchAppearance<T extends { appearance: object }>(preferences: T, patch: Partial<T["appearance"]>): T {
  return { ...preferences, appearance: { ...preferences.appearance, ...patch } };
}

type AuxiliarySectionLabels = {
  inheritance: string;
  backgroundSize: string;
  size: string;
  textLayout: string;
  colorEffects: string;
  displayInteraction: string;
};

export function auxiliarySections(mode: AuxiliaryMode, labels: AuxiliarySectionLabels, notchLyricsEnabled = true) {
  if (mode === "statusBar") return [
    { id: "mode-state", label: labels.displayInteraction },
    { id: "mode-inheritance", label: labels.inheritance },
    { id: "mode-text", label: labels.textLayout },
    { id: "mode-colors", label: labels.colorEffects },
    { id: "mode-background", label: labels.size },
  ];
  if (mode === "listWindow") return [
    { id: "mode-state", label: labels.displayInteraction },
    { id: "mode-inheritance", label: labels.inheritance },
    { id: "mode-text", label: labels.textLayout },
    { id: "mode-colors", label: labels.colorEffects },
    { id: "mode-background", label: labels.backgroundSize },
  ];
  if (mode === "notch" && !notchLyricsEnabled) return [
    { id: "mode-state", label: labels.displayInteraction },
    { id: "mode-background", label: labels.backgroundSize },
  ];
  return [
    { id: "mode-state", label: labels.displayInteraction },
    { id: "mode-inheritance", label: labels.inheritance },
    { id: "mode-text", label: labels.textLayout },
    { id: "mode-colors", label: labels.colorEffects },
    { id: "mode-background", label: labels.backgroundSize },
  ];
}

export default function LyricsModeStyleSections({ mode, displays, inheritance, update, setListLyricsLocked, updateInheritance, resetPosition }: Props) {
  const { t } = useTranslation();
  const [notchMonitors, setNotchMonitors] = useState<LyricsMonitor[]>([]);
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

  const updateListLocked = (locked: boolean) => setListLyricsLocked(locked).catch((error) => {
    reportFrontendError("Failed to update lyrics window lock state", error);
  });

  useEffect(() => {
    if (mode !== "notch" || !isTauriRuntime()) {
      setNotchMonitors([]);
      return;
    }
    let active = true;
    void api.getLyricsMonitors()
      .then((monitors) => {
        if (active) setNotchMonitors(monitors);
      })
      .catch(() => {
        if (active) setNotchMonitors([]);
      });
    return () => {
      active = false;
    };
  }, [mode]);

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
      <SettingsSection id="mode-state" title={t("settings.style.modeControls.displayInteraction")}>
        <ToggleRow label={t("settings.display.statusBar.show")} value={value.enabled} onChange={(enabled) => save({ ...value, enabled })} />
        <ToggleRow label={t("settings.display.statusBar.autoHide")} description={t("settings.display.statusBar.autoHideHint")} value={value.hideWhenNotPlaying} onChange={(hideWhenNotPlaying) => save({ ...value, hideWhenNotPlaying })} />
      </SettingsSection>
      {inheritanceSection}
      <SettingsSection id="mode-text" title={t("settings.style.modeControls.textLayout")}>
        {!modeInheritance.inheritFontFamily && <TextRow label={t("settings.overlay.fontFamily")} value={appearance.fontFamily} emptyValue={appearance.fontFamily} onChange={(fontFamily) => save(patchAppearance(value, { fontFamily }))} />}
        <RangeRow label={t("settings.overlay.fontSize")} value={appearance.fontSize} min={10} max={18} suffix=" pt" onChange={(fontSize) => save(patchAppearance(value, { fontSize }))} />
        <SelectRow label={t("settings.overlay.fontWeight")} value={String(appearance.fontWeight)} options={fontWeights} onChange={(fontWeight) => save(patchAppearance(value, { fontWeight: Number(fontWeight) as OverlayFontWeight }))} />
        <SelectRow label={t("settings.display.statusBar.alignment")} value={appearance.alignment} options={[["left", t("settings.display.statusBar.alignmentLeft")], ["center", t("settings.display.statusBar.alignmentCenter")], ["right", t("settings.display.statusBar.alignmentRight")]]} onChange={(alignment) => save(patchAppearance(value, { alignment: alignment as StatusBarLyricsPreferences["appearance"]["alignment"] }))} />
      </SettingsSection>
      <SettingsSection id="mode-colors" title={t("settings.style.modeControls.colorEffects")}>
        <SelectRow label={t("settings.overlay.karaoke")} value={appearance.karaokeStyle} options={[["sweep", t("settings.overlay.karaokeSweep")], ["highlight", t("settings.overlay.karaokeHighlight")]]} onChange={(karaokeStyle) => save(patchAppearance(value, { karaokeStyle: karaokeStyle as CompactKaraokeStyle }))} />
        {!modeInheritance.inheritColors && <>
          <ColorRow label={t("settings.display.statusBar.textColor")} description={t("settings.display.statusBar.textColorHint")} value={appearance.textColor} onChange={(textColor) => save(patchAppearance(value, { textColor }))} />
          <ColorRow label={t("settings.display.statusBar.highlightColor")} description={t("settings.display.statusBar.highlightColorHint")} value={appearance.highlightColor} onChange={(highlightColor) => save(patchAppearance(value, { highlightColor }))} />
          <ColorRow label={t("settings.display.statusBar.inactiveColor")} description={t("settings.display.statusBar.inactiveColorHint")} value={appearance.inactiveColor} onChange={(inactiveColor) => save(patchAppearance(value, { inactiveColor }))} />
        </>}
      </SettingsSection>
      <SettingsSection id="mode-background" title={t("settings.style.modeControls.size")}>
        <RangeRow label={t("settings.display.statusBar.width")} description={t("settings.display.statusBar.widthHint")} value={appearance.width} min={120} max={360} step={5} suffix=" pt" onChange={(width) => save(patchAppearance(value, { width }))} />
      </SettingsSection>
    </>;
  }

  if (mode === "listWindow") {
    const value = displays.listWindow;
    const appearance = value.appearance;
    const save = (next: ListLyricsPreferences) => void update("listWindow", next);
    return <>
      <SettingsSection id="mode-state" title={t("settings.style.modeControls.displayInteraction")}>
        <ToggleRow label={t("settings.display.listWindow.show")} value={value.enabled} onChange={(enabled) => save({ ...value, enabled })} />
        <ToggleRow label={t("settings.display.listWindow.lock")} description={t("settings.display.listWindow.lockHint")} value={value.locked} onChange={updateListLocked} />
        <ToggleRow label={t("settings.display.listWindow.translation")} value={value.showTranslation} onChange={(showTranslation) => save({ ...value, showTranslation })} />
        <ToggleRow label={t("settings.display.listWindow.romanization")} value={value.showRomanization} onChange={(showRomanization) => save({ ...value, showRomanization })} />
      </SettingsSection>
      {inheritanceSection}
      <SettingsSection id="mode-text" title={t("settings.style.modeControls.textLayout")}>
        {!modeInheritance.inheritFontFamily && <TextRow label={t("settings.overlay.fontFamily")} value={appearance.fontFamily} emptyValue={appearance.fontFamily} onChange={(fontFamily) => save(patchAppearance(value, { fontFamily }))} />}
        <RangeRow label={t("settings.style.modeControls.mainFontSize")} value={appearance.fontSize} min={12} max={56} suffix="px" onChange={(fontSize) => save(patchAppearance(value, { fontSize }))} />
        <SelectRow label={t("settings.style.modeControls.mainFontWeight")} value={String(appearance.fontWeight)} options={fontWeights} onChange={(fontWeight) => save(patchAppearance(value, { fontWeight: Number(fontWeight) as OverlayFontWeight }))} />
        <RangeRow label={t("settings.style.modeControls.secondaryFontSize")} value={appearance.secondaryFontScale} min={0.35} max={1} step={0.05} suffix="%" displayValue={Math.round(appearance.secondaryFontScale * 100)} onChange={(secondaryFontScale) => save(patchAppearance(value, { secondaryFontScale }))} />
        <RangeRow label={t("settings.overlay.lineHeight")} value={appearance.lineHeight} min={0.8} max={2} step={0.05} suffix="×" onChange={(lineHeight) => save(patchAppearance(value, { lineHeight }))} />
        <RangeRow label={t("settings.style.modeControls.lineGap")} value={appearance.lineGap} min={0} max={32} suffix="px" onChange={(lineGap) => save(patchAppearance(value, { lineGap }))} />
        <SelectRow label={t("settings.style.modeControls.alignment")} value={appearance.alignment} options={[["left", t("settings.style.modeControls.left")], ["center", t("settings.style.modeControls.center")], ["right", t("settings.style.modeControls.right")]]} onChange={(alignment) => save(patchAppearance(value, { alignment: alignment as ListLyricsPreferences["appearance"]["alignment"] }))} />
      </SettingsSection>
      <SettingsSection id="mode-colors" title={t("settings.style.modeControls.colorEffects")}>
        {!modeInheritance.inheritColors && <>
          <ColorRow label={t("settings.display.listWindow.activeColor")} description={t("settings.display.listWindow.activeColorHint")} value={appearance.activeColor} onChange={(activeColor) => save(patchAppearance(value, { activeColor }))} />
          <ColorRow label={t("settings.display.listWindow.inactiveColor")} description={t("settings.display.listWindow.inactiveColorHint")} value={appearance.inactiveColor} onChange={(inactiveColor) => save(patchAppearance(value, { inactiveColor }))} />
          <ColorRow label={t("settings.overlay.translationColor")} value={appearance.translationColor} onChange={(translationColor) => save(patchAppearance(value, { translationColor }))} />
          <ColorRow label={t("settings.overlay.romanizationColor")} value={appearance.romanizationColor} onChange={(romanizationColor) => save(patchAppearance(value, { romanizationColor }))} />
        </>}
        <ColorRow label={t("settings.style.modeControls.activeBackground")} value={appearance.activeBackgroundColor} onChange={(activeBackgroundColor) => save(patchAppearance(value, { activeBackgroundColor }))} />
      </SettingsSection>
      <SettingsSection id="mode-background" title={t("settings.style.modeControls.backgroundSize")}>
        {!modeInheritance.inheritColors && <ColorRow label={t("settings.style.modeControls.windowBackground")} value={appearance.backgroundColor} onChange={(backgroundColor) => save(patchAppearance(value, { backgroundColor }))} />}
        <RangeRow label={t("settings.overlay.backgroundOpacity")} value={appearance.backgroundOpacity} min={0} max={1} step={0.05} suffix="%" displayValue={Math.round(appearance.backgroundOpacity * 100)} onChange={(backgroundOpacity) => save(patchAppearance(value, { backgroundOpacity }))} />
      </SettingsSection>
    </>;
  }

  const value = displays.notch;
  const appearance = value.appearance;
  const save = (next: NotchLyricsPreferences) => void update("notch", next);
  const slotOptions: Array<[NotchSlotContent, string]> = [
    ["empty", t("settings.display.notch.slotEmpty")],
    ["title", t("settings.display.notch.slotTitle")],
    ["artist", t("settings.display.notch.slotArtist")],
    ["artwork", t("settings.display.notch.slotArtwork")],
    ["spectrum", t("settings.display.notch.slotSpectrum")],
  ];
  const selectedMonitorId = notchMonitors.some((monitor) => monitor.id === value.monitorId)
    ? value.monitorId ?? ""
    : notchMonitors.find((monitor) => monitor.isPrimary)?.id ?? notchMonitors[0]?.id ?? "";
  const monitorOptions: Array<[string, string]> = notchMonitors.map((monitor, index) => {
    const name = monitor.name || t("settings.display.notch.displayFallback", { index: index + 1 });
    const primary = monitor.isPrimary ? ` · ${t("settings.display.notch.primaryDisplay")}` : "";
    return [monitor.id, `${name} · ${monitor.width}×${monitor.height}${primary}`];
  });
  const normalizeNotchWidthRange = (next: [number, number]): [number, number] => {
    const maxWidth = Math.min(next[0], next[1]);
    return [maxWidth, Math.max(440, maxWidth, next[1])];
  };
  const previewWidth = (widths: [number, number]) => {
    notchWidthPreviewActiveRef.current = true;
    void emitNotchWidthPreview({ phase: "update", maxWidth: widths[0], expandedMaxWidth: widths[1] }).catch((error) => {
      reportFrontendError("Failed to preview the Dynamic Island width", error);
    });
  };
  const commitWidth = async (widths: [number, number]) => {
    const [maxWidth, expandedMaxWidth] = normalizeNotchWidthRange(widths);
    try {
      await update("notch", patchAppearance(value, { maxWidth, expandedMaxWidth }));
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
      await emitNotchWidthPreview({ phase: "commit", maxWidth, expandedMaxWidth });
    } catch (error) {
      void emitNotchWidthPreview({ phase: "cancel" });
      reportFrontendError("Failed to finish the Dynamic Island width preview", error);
    }
  };
  const notchWidthRange = normalizeNotchWidthRange([appearance.maxWidth, appearance.expandedMaxWidth]);
  return <>
    <SettingsSection id="mode-state" title={t("settings.style.modeControls.displayInteraction")}>
      <ToggleRow label={t("settings.display.notch.show")} description={t("settings.display.notch.showHint")} value={value.enabled} onChange={(enabled) => save({ ...value, enabled })} />
      {value.enabled && <>
        {notchMonitors.length >= 2 && selectedMonitorId && <SelectRow label={t("settings.display.notch.display")} value={selectedMonitorId} options={monitorOptions} onChange={(monitorId) => save({ ...value, monitorId })} />}
        <ToggleRow label={t("settings.display.notch.autoHide")} description={t("settings.display.notch.autoHideHint")} value={value.hideWhenNotPlaying} onChange={(hideWhenNotPlaying) => save({ ...value, hideWhenNotPlaying })} />
        <ToggleRow label={t("settings.display.notch.showLyrics")} description={t("settings.display.notch.showLyricsHint")} value={value.showLyrics} onChange={(showLyrics) => save({ ...value, showLyrics })} />
        <SelectRow label={t("settings.display.notch.leftSlot")} value={value.leftSlot} options={slotOptions} onChange={(leftSlot) => save({ ...value, leftSlot: leftSlot as NotchSlotContent })} />
        <SelectRow label={t("settings.display.notch.rightSlot")} value={value.rightSlot} options={slotOptions} onChange={(rightSlot) => save({ ...value, rightSlot: rightSlot as NotchSlotContent })} />
        {value.showLyrics && <>
          <ToggleRow label={t("settings.display.notch.translation")} value={value.showTranslation} onChange={(showTranslation) => save({ ...value, showTranslation })} />
          <ToggleRow label={t("settings.display.notch.romanization")} value={value.showRomanization} onChange={(showRomanization) => save({ ...value, showRomanization })} />
        </>}
      </>}
      <div className={styles.buttonRow}><Button variant="secondary" size="sm" onClick={() => void resetPosition("notch")}>{t("settings.style.modeControls.resetPosition")}</Button></div>
    </SettingsSection>
    {value.showLyrics && <>
    {inheritanceSection}
    <SettingsSection id="mode-text" title={t("settings.style.modeControls.textLayout")}>
      <SelectRow label={t("settings.overlay.lyricLayout")} value={value.layout} options={[["single", t("overlay.layout.single")], ["double", t("overlay.layout.double")]]} onChange={(layout) => save({ ...value, layout: layout as NotchLyricsPreferences["layout"] })} />
      <SelectRow label={t("settings.overlay.doubleLineMode")} description={t("settings.overlay.doubleLineModeHint")} disabled={value.layout !== "double"} value={value.doubleLineMode} options={[["rolling", t("settings.overlay.doubleLineRolling")], ["alternating", t("settings.overlay.doubleLineAlternating")]]} onChange={(doubleLineMode) => save({ ...value, doubleLineMode: doubleLineMode as NotchLyricsPreferences["doubleLineMode"] })} />
      {!modeInheritance.inheritFontFamily && <TextRow label={t("settings.overlay.fontFamily")} value={appearance.fontFamily} emptyValue={appearance.fontFamily} onChange={(fontFamily) => save(patchAppearance(value, { fontFamily }))} />}
      <RangeRow label={t("settings.overlay.fontSize")} value={appearance.fontSize} min={12} max={32} suffix="px" onChange={(fontSize) => save(patchAppearance(value, { fontSize }))} />
      <SelectRow label={t("settings.overlay.fontWeight")} value={String(appearance.fontWeight)} options={fontWeights} onChange={(fontWeight) => save(patchAppearance(value, { fontWeight: Number(fontWeight) as OverlayFontWeight }))} />
      <SelectRow label={t("settings.overlay.secondaryFontWeight")} value={String(appearance.secondaryFontWeight)} options={fontWeights} onChange={(secondaryFontWeight) => save(patchAppearance(value, { secondaryFontWeight: Number(secondaryFontWeight) as OverlayFontWeight }))} />
      <RangeRow label={t("settings.style.modeControls.lineGap")} disabled={value.layout !== "double"} value={appearance.lineGap} min={0} max={32} suffix="px" onChange={(lineGap) => save(patchAppearance(value, { lineGap }))} />
    </SettingsSection>
    <SettingsSection id="mode-colors" title={t("settings.style.modeControls.colorEffects")}>
      <SelectRow label={t("settings.overlay.karaoke")} value={appearance.karaokeStyle} options={[["sweep", t("settings.overlay.karaokeSweep")], ["highlight", t("settings.overlay.karaokeHighlight")]]} onChange={(karaokeStyle) => save(patchAppearance(value, { karaokeStyle: karaokeStyle as CompactKaraokeStyle }))} />
      {!modeInheritance.inheritColors && <>
        <ColorRow label={t("settings.display.notch.activeColor")} description={t("settings.display.notch.activeColorHint")} value={appearance.activeColor} onChange={(activeColor) => save(patchAppearance(value, { activeColor }))} />
        <ColorRow label={t("settings.display.notch.inactiveColor")} description={t("settings.display.notch.inactiveColorHint")} value={appearance.inactiveColor} onChange={(inactiveColor) => save(patchAppearance(value, { inactiveColor }))} />
        <ColorRow label={t("settings.overlay.translationColor")} value={appearance.translationColor} onChange={(translationColor) => save(patchAppearance(value, { translationColor }))} />
        <ColorRow label={t("settings.overlay.romanizationColor")} value={appearance.romanizationColor} onChange={(romanizationColor) => save(patchAppearance(value, { romanizationColor }))} />
      </>}
    </SettingsSection>
    </>}
    <SettingsSection id="mode-background" title={t("settings.style.modeControls.backgroundSize")}>
      <RangeRow label={t("settings.overlay.backgroundRadius")} value={appearance.borderRadius} min={0} max={40} suffix="px" onChange={(borderRadius) => save(patchAppearance(value, { borderRadius }))} />
      <RangePairRow
        label={t("settings.style.modeControls.widthRange")}
        firstLabel={t("settings.style.modeControls.compactWidth")}
        secondLabel={t("settings.style.modeControls.hoverWidth")}
        values={notchWidthRange}
        min={320}
        max={640}
        step={10}
        suffix="px"
        normalizeValues={normalizeNotchWidthRange}
        onValuePreview={previewWidth}
        onValueCommitted={commitWidth}
        onPreviewCanceled={cancelNotchWidthPreview}
      />
    </SettingsSection>
  </>;
}
