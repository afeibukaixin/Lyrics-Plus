import type { TFunction } from "i18next";
import {
  ClockArrowLeft,
  ClockArrowRight,
  EyeOff,
  Lock,
  Minus,
  Pin,
  Plus,
  RotateCcw,
  Settings,
  Square,
  SquareDashed,
} from "lucide-react";
import type { ListLyricsPreferences } from "../../shared/types";
import { IconButton } from "@/components/ui/icon-button";
import styles from "./LyricsListWindow.module.scss";

type LyricsListToolbarProps = {
  t: TFunction;
  options: ListLyricsPreferences;
  appearance: ListLyricsPreferences["appearance"];
  offsetAvailable: boolean;
  offsetMs: number;
  translationAvailable: boolean;
  romanizationAvailable: boolean;
  updatePreferences: (next: ListLyricsPreferences) => Promise<boolean>;
  updateAppearance: (patch: Partial<ListLyricsPreferences["appearance"]>) => Promise<boolean>;
  updateLocked: (nextLocked: boolean) => Promise<unknown>;
  changeLyricsOffset: (deltaMs: number) => void;
  setLyricsOffset: (nextOffsetMs: number) => void;
  openStyleSettings: () => void;
  resetWindowSize: () => void;
  onFocusCapture: () => void;
  onBlurCapture: () => void;
};

function formatOffset(value: number) {
  if (value === 0) return "0";
  if (Math.abs(value) >= 1_000 && value % 1_000 === 0) return `${value > 0 ? "+" : ""}${value / 1_000}s`;
  return `${value > 0 ? "+" : ""}${value}ms`;
}

export function LyricsListToolbar({
  t,
  options,
  appearance,
  offsetAvailable,
  offsetMs,
  translationAvailable,
  romanizationAvailable,
  updatePreferences,
  updateAppearance,
  updateLocked,
  changeLyricsOffset,
  setLyricsOffset,
  openStyleSettings,
  resetWindowSize,
  onFocusCapture,
  onBlurCapture,
}: LyricsListToolbarProps) {
  const transparentBackground = appearance.backgroundMode === "transparent";
  const supportingToggleTitle = (track: string, enabled: boolean, available: boolean) => {
    const action = enabled
      ? t("overlay.toolbar.hideTrack", { track })
      : t("overlay.toolbar.showTrack", { track });
    return available ? action : t("lyricsList.toolbar.unavailableTrack", { action, track });
  };

  const backgroundLabel = transparentBackground
    ? t("overlay.toolbar.backgroundTransparent")
    : t("overlay.toolbar.backgroundVisible");

  return (
    <div
      className={styles.toolbar}
      data-no-window-drag
      role="toolbar"
      aria-label={t("lyricsList.toolbar.label")}
      onFocusCapture={onFocusCapture}
      onBlurCapture={onBlurCapture}
    >
      <IconButton label={t("lyricsList.toolbar.lock")} variant="ghost" size="icon-sm" onClick={() => void updateLocked(true)}><Lock /></IconButton>
      <IconButton label={t("lyricsList.toolbar.decreaseFont")} variant="ghost" size="icon-sm" disabled={appearance.fontSize <= 12} onClick={() => void updateAppearance({ fontSize: Math.max(12, appearance.fontSize - 2) })}><Minus /></IconButton>
      <IconButton label={t("lyricsList.toolbar.increaseFont")} variant="ghost" size="icon-sm" disabled={appearance.fontSize >= 56} onClick={() => void updateAppearance({ fontSize: Math.min(56, appearance.fontSize + 2) })}><Plus /></IconButton>
      <div className={styles.offsetControl} role="group" aria-label={t("lyricsList.toolbar.offsetGroup", { value: formatOffset(offsetMs) })}>
        <IconButton label={t("lyricsList.toolbar.delay")} variant="ghost" size="icon-sm" disabled={!offsetAvailable} onClick={(event) => changeLyricsOffset(event.shiftKey ? -500 : -100)}><ClockArrowLeft /></IconButton>
        <IconButton className={styles.offsetValue} label={t("lyricsList.toolbar.resetOffset", { value: formatOffset(offsetMs) })} variant="ghost" size="icon-sm" disabled={!offsetAvailable || offsetMs === 0} onClick={() => setLyricsOffset(0)}>{offsetAvailable ? formatOffset(offsetMs) : "—"}</IconButton>
        <IconButton label={t("lyricsList.toolbar.advance")} variant="ghost" size="icon-sm" disabled={!offsetAvailable} onClick={(event) => changeLyricsOffset(event.shiftKey ? 500 : 100)}><ClockArrowRight /></IconButton>
      </div>
      <IconButton
        label={t("overlay.toolbar.toggleBackground", { value: backgroundLabel })}
        tooltip={t("overlay.toolbar.toggleBackgroundTitle", { value: backgroundLabel })}
        variant="ghost"
        size="icon-sm"
        aria-pressed={!transparentBackground}
        data-on={!transparentBackground}
        onClick={() => void updateAppearance({ backgroundMode: transparentBackground ? "solid" : "transparent" })}
      >{transparentBackground ? <SquareDashed /> : <Square />}</IconButton>
      <IconButton
        className={styles.trackToggle}
        label={supportingToggleTitle(t("common.feature.translation"), options.showTranslation, translationAvailable)}
        tooltip={supportingToggleTitle(t("common.feature.translation"), options.showTranslation, translationAvailable)}
        variant="ghost"
        size="icon-sm"
        aria-pressed={options.showTranslation}
        data-available={translationAvailable}
        data-on={options.showTranslation}
        onClick={() => void updatePreferences({ ...options, showTranslation: !options.showTranslation })}
      >{t("overlay.toolbar.translationGlyph")}</IconButton>
      <IconButton
        className={styles.trackToggle}
        label={supportingToggleTitle(t("common.feature.romanization"), options.showRomanization, romanizationAvailable)}
        tooltip={supportingToggleTitle(t("common.feature.romanization"), options.showRomanization, romanizationAvailable)}
        variant="ghost"
        size="icon-sm"
        aria-pressed={options.showRomanization}
        data-available={romanizationAvailable}
        data-on={options.showRomanization}
        onClick={() => void updatePreferences({ ...options, showRomanization: !options.showRomanization })}
      >{t("overlay.toolbar.romanizationGlyph")}</IconButton>
      <IconButton label={t("lyricsList.toolbar.openSettings")} variant="ghost" size="icon-sm" onClick={openStyleSettings}><Settings /></IconButton>
      <IconButton
        label={options.alwaysOnTop ? t("lyricsList.toolbar.unpin") : t("lyricsList.toolbar.pin")}
        variant="ghost"
        size="icon-sm"
        aria-pressed={options.alwaysOnTop}
        onClick={() => void updatePreferences({ ...options, alwaysOnTop: !options.alwaysOnTop })}
      ><Pin /></IconButton>
      <IconButton label={t("lyricsList.toolbar.resetSize")} variant="ghost" size="icon-sm" onClick={resetWindowSize}><RotateCcw /></IconButton>
      <IconButton label={t("lyricsList.toolbar.hide")} variant="ghost" size="icon-sm" onClick={() => void updatePreferences({ ...options, enabled: false })}><EyeOff /></IconButton>
    </div>
  );
}
