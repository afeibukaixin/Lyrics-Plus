import type { TFunction } from "i18next";
import {
  Captions,
  ClockArrowLeft,
  ClockArrowRight,
  PanelTop,
  PanelsTopBottom,
  Settings,
} from "lucide-react";
import { ToolbarIconButton } from "@/components/ui/toolbar-icon-button";
import type { NotchLyricsPreferences } from "../../shared/types";
import styles from "./NotchLyricsWindow.module.scss";

function formatLyricsOffset(offsetMs: number) {
  if (offsetMs === 0) return "0ms";
  return `${offsetMs > 0 ? "+" : "−"}${Math.abs(offsetMs)}ms`;
}

function formatCompactLyricsOffset(offsetMs: number) {
  if (offsetMs === 0) return "0s";
  const seconds = (Math.abs(offsetMs) / 1_000).toFixed(3).replace(/\.?0+$/, "");
  return `${offsetMs > 0 ? "+" : "−"}${seconds}s`;
}

type NotchLyricsQuickControlsProps = {
  notch: NotchLyricsPreferences;
  offsetAvailable: boolean;
  offsetMs: number;
  romanizationAvailable: boolean;
  translationAvailable: boolean;
  onChangeOffset: (deltaMs: number) => void;
  onOpenSettings: () => void;
  onPatchNotch: (patch: Partial<NotchLyricsPreferences>) => void;
  onResetOffset: () => void;
  t: TFunction;
};

export function NotchLyricsQuickControls({
  notch,
  offsetAvailable,
  offsetMs,
  romanizationAvailable,
  translationAvailable,
  onChangeOffset,
  onOpenSettings,
  onPatchNotch,
  onResetOffset,
  t,
}: NotchLyricsQuickControlsProps) {
  const translation = t("common.feature.translation");
  const romanization = t("common.feature.romanization");
  const translationAction = notch.showTranslation
    ? t("overlay.toolbar.hideTrack", { track: translation })
    : t("overlay.toolbar.showTrack", { track: translation });
  const romanizationAction = notch.showRomanization
    ? t("overlay.toolbar.hideTrack", { track: romanization })
    : t("overlay.toolbar.showTrack", { track: romanization });
  const translationLabel = translationAvailable
    ? translationAction
    : t("notchLyrics.toolbar.unavailableTrack", { action: translationAction, track: translation });
  const romanizationLabel = romanizationAvailable
    ? romanizationAction
    : t("notchLyrics.toolbar.unavailableTrack", { action: romanizationAction, track: romanization });
  const layoutValue = t(`overlay.layout.${notch.layout}`);
  const offsetDisplayLabel = offsetAvailable ? formatCompactLyricsOffset(offsetMs) : "—";
  const offsetAriaLabel = offsetAvailable ? formatLyricsOffset(offsetMs) : "—";
  const offsetValueLabel = !offsetAvailable
    ? t("overlay.toolbar.noOffset")
    : offsetMs === 0
      ? t("overlay.toolbar.zeroOffset")
      : t("overlay.toolbar.offsetReset", { value: formatLyricsOffset(offsetMs) });
  const offsetValueTooltip = !offsetAvailable
    ? t("notchLyrics.toolbar.unavailableOffset")
    : offsetMs === 0
      ? t("overlay.toolbar.offsetZeroTitle")
      : t("overlay.toolbar.offsetTitle", { value: formatLyricsOffset(offsetMs) });

  return (
    <div className={styles.lyricsQuickControls} role="group" aria-label={t("notchLyrics.toolbar.label")}>
      {!notch.showLyrics ? (
        <div className={styles.lyricsQuickControlsOff}>
          <ToolbarIconButton
            className={styles.quickToggle}
            interactionMode="native"
            label={t("notchLyrics.toolbar.showLyrics")}
            variant="ghost"
            size="icon-sm"
            aria-pressed={false}
            onClick={() => onPatchNotch({ showLyrics: true })}
          ><Captions aria-hidden="true" /></ToolbarIconButton>
          <ToolbarIconButton
            interactionMode="native"
            label={t("notchLyrics.toolbar.openSettings")}
            variant="ghost"
            size="icon-sm"
            onClick={onOpenSettings}
          ><Settings aria-hidden="true" /></ToolbarIconButton>
        </div>
      ) : (
        <>
          <div className={styles.lyricsQuickControlRow}>
            <ToolbarIconButton
              className={styles.quickToggle}
              interactionMode="native"
              label={t("notchLyrics.toolbar.hideLyrics")}
              variant="ghost"
              size="icon-sm"
              aria-pressed
              onClick={() => onPatchNotch({ showLyrics: false })}
            ><Captions aria-hidden="true" /></ToolbarIconButton>
            <ToolbarIconButton
              className={styles.quickToggle}
              interactionMode="native"
              label={t("overlay.toolbar.toggleLayout", { value: layoutValue })}
              tooltip={t("overlay.toolbar.toggleLayoutTitle", { value: layoutValue })}
              variant="ghost"
              size="icon-sm"
              aria-pressed={notch.layout === "double"}
              onClick={() => onPatchNotch({ layout: notch.layout === "double" ? "single" : "double" })}
            >{notch.layout === "double" ? <PanelsTopBottom aria-hidden="true" /> : <PanelTop aria-hidden="true" />}</ToolbarIconButton>
            <ToolbarIconButton
              className={styles.trackToggle}
              interactionMode="native"
              label={translationLabel}
              tooltip={translationLabel}
              variant="ghost"
              size="icon-sm"
              aria-pressed={notch.showTranslation}
              data-available={translationAvailable}
              onClick={() => onPatchNotch({ showTranslation: !notch.showTranslation })}
            >{t("overlay.toolbar.translationGlyph")}</ToolbarIconButton>
            <ToolbarIconButton
              className={styles.trackToggle}
              interactionMode="native"
              label={romanizationLabel}
              tooltip={romanizationLabel}
              variant="ghost"
              size="icon-sm"
              aria-pressed={notch.showRomanization}
              data-available={romanizationAvailable}
              onClick={() => onPatchNotch({ showRomanization: !notch.showRomanization })}
            >{t("overlay.toolbar.romanizationGlyph")}</ToolbarIconButton>
          </div>
          <div className={styles.lyricsQuickControlRow}>
            <div
              className={styles.offsetControl}
              role="group"
              aria-label={t("overlay.toolbar.offsetGroup", { value: offsetAriaLabel })}
            >
              <ToolbarIconButton
                interactionMode="native"
                label={t("overlay.toolbar.delay")}
                tooltip={t("overlay.toolbar.delayTitle")}
                variant="ghost"
                size="icon-sm"
                disabled={!offsetAvailable}
                onClick={(event) => onChangeOffset(event.shiftKey ? -500 : -100)}
              ><ClockArrowLeft aria-hidden="true" /></ToolbarIconButton>
              <ToolbarIconButton
                className={styles.offsetValue}
                interactionMode="native"
                label={offsetValueLabel}
                tooltip={offsetValueTooltip}
                variant="ghost"
                size="icon-sm"
                disabled={!offsetAvailable || offsetMs === 0}
                onClick={onResetOffset}
              >{offsetDisplayLabel}</ToolbarIconButton>
              <ToolbarIconButton
                interactionMode="native"
                label={t("overlay.toolbar.advance")}
                tooltip={t("overlay.toolbar.advanceTitle")}
                variant="ghost"
                size="icon-sm"
                disabled={!offsetAvailable}
                onClick={(event) => onChangeOffset(event.shiftKey ? 500 : 100)}
              ><ClockArrowRight aria-hidden="true" /></ToolbarIconButton>
            </div>
            <ToolbarIconButton
              interactionMode="native"
              label={t("notchLyrics.toolbar.openSettings")}
              variant="ghost"
              size="icon-sm"
              onClick={onOpenSettings}
            ><Settings aria-hidden="true" /></ToolbarIconButton>
          </div>
        </>
      )}
    </div>
  );
}
