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
import { ColorRow, RangeRow, SelectRow, SettingsSection, TextRow, ToggleRow } from "./components";
import { Button } from "@/components/ui/button";
import styles from "../settings.module.scss";
import { useTranslation } from "react-i18next";
import { useCallback, useEffect, useRef, useState } from "react";
import { reportFrontendError } from "../../shared/debugLog";
import { emitNotchWidthPreview, type NotchWidthPreviewTarget } from "../../shared/tauriEvent";
import { api, isTauriRuntime } from "../../shared/api";

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

export function auxiliarySections(mode: AuxiliaryMode, labels: AuxiliarySectionLabels, notchLyricsEnabled = true) {
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
  if (mode === "notch" && !notchLyricsEnabled) return [
    { id: "mode-state", label: labels.displayInteraction },
    { id: "mode-background", label: labels.backgroundSize },
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
        <SelectRow label={t("settings.overlay.karaoke")} value={appearance.karaokeStyle} options={[["sweep", t("settings.overlay.karaokeSweep")], ["highlight", t("settings.overlay.karaokeHighlight")]]} onChange={(karaokeStyle) => save(patchAppearance(value, { karaokeStyle: karaokeStyle as CompactKaraokeStyle }))} />
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
  const previewWidth = (target: NotchWidthPreviewTarget, width: number) => {
    notchWidthPreviewActiveRef.current = true;
    void emitNotchWidthPreview({ phase: "update", target, width }).catch((error) => {
      reportFrontendError("Failed to preview the Dynamic Island width", error);
    });
  };
  const commitWidth = async (target: NotchWidthPreviewTarget, width: number) => {
    const committedWidth = target === "expanded"
      ? Math.max(appearance.maxWidth, width)
      : width;
    const appearancePatch = target === "collapsed"
      ? { maxWidth: width }
      : { expandedMaxWidth: committedWidth };
    try {
      await update("notch", patchAppearance(value, appearancePatch));
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
      await emitNotchWidthPreview({ phase: "commit", target, width: committedWidth });
    } catch (error) {
      void emitNotchWidthPreview({ phase: "cancel" });
      reportFrontendError("Failed to finish the Dynamic Island width preview", error);
    }
  };
  const expandedWidthLocked = appearance.maxWidth >= 640;
  const expandedWidthMin = expandedWidthLocked ? 440 : Math.max(440, appearance.maxWidth);
  const expandedWidthValue = Math.max(appearance.expandedMaxWidth, appearance.maxWidth);
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
          <SelectRow label={t("settings.overlay.lyricLayout")} value={value.layout} options={[["single", t("overlay.layout.single")], ["double", t("overlay.layout.double")]]} onChange={(layout) => save({ ...value, layout: layout as NotchLyricsPreferences["layout"] })} />
          <ToggleRow label={t("settings.display.notch.translation")} value={value.showTranslation} onChange={(showTranslation) => save({ ...value, showTranslation })} />
          <ToggleRow label={t("settings.display.notch.romanization")} value={value.showRomanization} onChange={(showRomanization) => save({ ...value, showRomanization })} />
        </>}
      </>}
      <div className={styles.buttonRow}><Button variant="secondary" size="sm" onClick={() => void resetPosition("notch")}>{t("settings.style.modeControls.resetPosition")}</Button></div>
    </SettingsSection>
    {value.showLyrics && <>
    {inheritanceSection}
    <SettingsSection id="mode-text" title={t("settings.style.modeControls.text")}>
      {!modeInheritance.inheritFontFamily && <TextRow label={t("settings.overlay.fontFamily")} value={appearance.fontFamily} emptyValue={appearance.fontFamily} onChange={(fontFamily) => save(patchAppearance(value, { fontFamily }))} />}
      <RangeRow label={t("settings.overlay.fontSize")} value={appearance.fontSize} min={12} max={32} suffix="px" onChange={(fontSize) => save(patchAppearance(value, { fontSize }))} />
      <SelectRow label={t("settings.overlay.fontWeight")} value={String(appearance.fontWeight)} options={fontWeights} onChange={(fontWeight) => save(patchAppearance(value, { fontWeight: Number(fontWeight) as OverlayFontWeight }))} />
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
      <RangeRow
        label={t("settings.style.modeControls.maxWidth")}
        value={appearance.maxWidth}
        min={320}
        max={640}
        step={10}
        suffix="px"
        onChange={(maxWidth) => save(patchAppearance(value, { maxWidth }))}
        onValuePreview={(width) => previewWidth("collapsed", width)}
        onValueCommitted={(width) => commitWidth("collapsed", width)}
        onPreviewCanceled={cancelNotchWidthPreview}
      />
      <RangeRow
        label={t("settings.style.modeControls.hoverMaxWidth")}
        value={expandedWidthValue}
        min={expandedWidthMin}
        max={640}
        step={10}
        suffix="px"
        disabled={expandedWidthLocked}
        onChange={(expandedMaxWidth) => save(patchAppearance(value, { expandedMaxWidth }))}
        onValuePreview={(width) => previewWidth("expanded", width)}
        onValueCommitted={(width) => commitWidth("expanded", width)}
        onPreviewCanceled={cancelNotchWidthPreview}
      />
    </SettingsSection>
  </>;
}
