import type { RefObject } from "react";
import type { TFunction } from "i18next";
import {
  ClockArrowLeft,
  ClockArrowRight,
  EyeOff,
  Lock,
  Minus,
  MoveHorizontal,
  MoveVertical,
  PanelTop,
  PanelsTopBottom,
  Plus,
  Settings,
  Square,
  SquareDashed,
} from "lucide-react";
import { ToolbarIconButton } from "@/components/ui/toolbar-icon-button";
import type { OverlayStyle } from "../../shared/types";
import { formatOffsetMs, nextValue } from "./OverlayLayout";
import styles from "./Overlay.module.scss";

type SecondaryFlags = {
  translation: boolean;
  romanization: boolean;
};

type OverlayToolbarProps = {
  toolbarRef: RefObject<HTMLDivElement | null>;
  style: OverlayStyle;
  vertical: boolean;
  transparentMode: boolean;
  backgroundLabel: string;
  offsetAvailable: boolean;
  offsetMs: number;
  offsetLabel: string;
  offsetValueTitle: string;
  secondaryFlags: SecondaryFlags;
  translationAvailable: boolean;
  romanizationAvailable: boolean;
  t: TFunction;
  updateStyle: (patch: Partial<OverlayStyle>) => Promise<void>;
  changeLyricsOffset: (deltaMs: number) => void;
  setLyricsOffset: (nextOffsetMs: number) => void;
  lockOverlay: () => void;
  hideOverlay: () => void;
  openSettings: () => void;
  toggleSupportingTrack: (kind: "translation" | "romanization") => void;
  supportingToggleTitle: (track: string, enabled: boolean, available: boolean) => string;
};

export function OverlayToolbar({
  toolbarRef,
  style,
  vertical,
  transparentMode,
  backgroundLabel,
  offsetAvailable,
  offsetMs,
  offsetLabel,
  offsetValueTitle,
  secondaryFlags,
  translationAvailable,
  romanizationAvailable,
  t,
  updateStyle,
  changeLyricsOffset,
  setLyricsOffset,
  lockOverlay,
  hideOverlay,
  openSettings,
  toggleSupportingTrack,
  supportingToggleTitle,
}: OverlayToolbarProps) {
  return (
    <div className={styles.toolbar} data-tauri-drag-region="false" role="toolbar" aria-label={t("overlay.toolbar.label")} ref={toolbarRef}>
      <ToolbarIconButton label={t("overlay.toolbar.lock")} variant="ghost" size="icon-sm" onClick={lockOverlay}><Lock /></ToolbarIconButton>
      <ToolbarIconButton label={t("overlay.toolbar.decreaseFont")} variant="ghost" size="icon-sm" onClick={() => void updateStyle({ fontSize: style.fontSize - 2 })}><Minus /></ToolbarIconButton>
      <ToolbarIconButton label={t("overlay.toolbar.increaseFont")} variant="ghost" size="icon-sm" onClick={() => void updateStyle({ fontSize: style.fontSize + 2 })}><Plus /></ToolbarIconButton>
      <div className={styles.offsetControl} role="group" aria-label={t("overlay.toolbar.offsetGroup", { value: offsetAvailable ? formatOffsetMs(offsetMs) : t("overlay.toolbar.unavailable") })}>
        <ToolbarIconButton
          label={t("overlay.toolbar.delay")}
          tooltip={t("overlay.toolbar.delayTitle")}
          variant="ghost"
          size="icon-sm"
          disabled={!offsetAvailable}
          onClick={(event) => changeLyricsOffset(event.shiftKey ? -500 : -100)}
        ><ClockArrowLeft /></ToolbarIconButton>
        <ToolbarIconButton
          className={styles.offsetValue}
          label={!offsetAvailable
            ? t("overlay.toolbar.noOffset")
            : offsetMs === 0
              ? t("overlay.toolbar.zeroOffset")
              : t("overlay.toolbar.offsetReset", { value: formatOffsetMs(offsetMs) })}
          tooltip={offsetValueTitle}
          variant="ghost"
          size="icon-sm"
          disabled={!offsetAvailable || offsetMs === 0}
          onClick={() => setLyricsOffset(0)}
        >{offsetLabel}</ToolbarIconButton>
        <ToolbarIconButton
          label={t("overlay.toolbar.advance")}
          tooltip={t("overlay.toolbar.advanceTitle")}
          variant="ghost"
          size="icon-sm"
          disabled={!offsetAvailable}
          onClick={(event) => changeLyricsOffset(event.shiftKey ? 500 : 100)}
        ><ClockArrowRight /></ToolbarIconButton>
      </div>
      <ToolbarIconButton label={t("overlay.toolbar.toggleLayout", { value: t(`overlay.layout.${style.layout}`) })} tooltip={t("overlay.toolbar.toggleLayoutTitle", { value: t(`overlay.layout.${style.layout}`) })} variant="ghost" size="icon-sm" onClick={() => void updateStyle({
        layout: nextValue(style.layout, ["single", "double"] as const),
      })}>{style.layout === "double" ? <PanelsTopBottom /> : <PanelTop />}</ToolbarIconButton>
      <ToolbarIconButton label={t("overlay.toolbar.toggleOrientation", { value: t(`overlay.orientation.${style.orientation}`) })} tooltip={t("overlay.toolbar.toggleOrientationTitle", { value: t(`overlay.orientation.${style.orientation}`) })} variant="ghost" size="icon-sm" onClick={() => void updateStyle({
        orientation: nextValue(style.orientation, ["horizontal", "vertical"] as const),
      })}>{vertical ? <MoveVertical /> : <MoveHorizontal />}</ToolbarIconButton>
      <ToolbarIconButton
        label={t("overlay.toolbar.toggleBackground", { value: backgroundLabel })}
        tooltip={t("overlay.toolbar.toggleBackgroundTitle", { value: backgroundLabel })}
        variant="ghost"
        size="icon-sm"
        aria-pressed={!transparentMode}
        onClick={() => void updateStyle(transparentMode
          ? {
              backgroundMode: "solid",
              ...(style.background === "transparent" ? { background: "solid" as const } : {}),
            }
          : { backgroundMode: "transparent" })}
      >{transparentMode ? <SquareDashed /> : <Square />}</ToolbarIconButton>
      <ToolbarIconButton
        label={supportingToggleTitle(t("common.feature.translation"), secondaryFlags.translation, translationAvailable)}
        tooltip={supportingToggleTitle(t("common.feature.translation"), secondaryFlags.translation, translationAvailable)}
        variant="ghost"
        size="icon-sm"
        className={styles.trackToggle}
        data-available={translationAvailable}
        aria-pressed={secondaryFlags.translation}
        onClick={() => toggleSupportingTrack("translation")}
      >{t("overlay.toolbar.translationGlyph")}</ToolbarIconButton>
      <ToolbarIconButton
        label={supportingToggleTitle(t("common.feature.romanization"), secondaryFlags.romanization, romanizationAvailable)}
        tooltip={supportingToggleTitle(t("common.feature.romanization"), secondaryFlags.romanization, romanizationAvailable)}
        variant="ghost"
        size="icon-sm"
        className={styles.trackToggle}
        data-available={romanizationAvailable}
        aria-pressed={secondaryFlags.romanization}
        onClick={() => toggleSupportingTrack("romanization")}
      >{t("overlay.toolbar.romanizationGlyph")}</ToolbarIconButton>
      <ToolbarIconButton label={t("overlay.toolbar.hide")} variant="ghost" size="icon-sm" onClick={hideOverlay}><EyeOff /></ToolbarIconButton>
      <ToolbarIconButton label={t("overlay.toolbar.openSettings")} variant="ghost" size="icon-sm" onClick={openSettings}><Settings /></ToolbarIconButton>
    </div>
  );
}
