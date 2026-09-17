import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { OverlayFontWeight } from "../../../shared/types";
import { api } from "../../../shared/api";
import { fontFamiliesFromCss } from "../../../shared/fontFamily";
import { isMacTauriRuntime } from "../../../shared/tauriEvent";
import { SelectRow } from "../shared/components";

const WEIGHTS = [
  [100, "fontWeightUltralight"],
  [200, "fontWeightThin"],
  [300, "fontWeightLight"],
  [400, "fontWeightRegular"],
  [500, "fontWeightMedium"],
  [600, "fontWeightSemibold"],
  [700, "fontWeightBold"],
  [800, "fontWeightExtrabold"],
  [900, "fontWeightBlack"],
] as const satisfies ReadonlyArray<readonly [OverlayFontWeight, string]>;

type Props = {
  label: string;
  family: string;
  value: OverlayFontWeight;
  showHint?: boolean;
  onChange: (weight: OverlayFontWeight) => void;
};

export default function FontWeightSelect({ label, family, value, showHint = true, onChange }: Props) {
  const { t } = useTranslation();
  const primaryFamily = fontFamiliesFromCss(family)[0]?.family ?? family;
  const [available, setAvailable] = useState<{ family: string; weights: number[] | null } | null>(null);

  useEffect(() => {
    if (!isMacTauriRuntime() || !primaryFamily.trim()) return;
    let active = true;
    void api.getFontAvailableWeights(primaryFamily)
      .then((weights) => {
        if (active) setAvailable({ family: primaryFamily, weights });
      })
      .catch(() => {
        if (active) setAvailable({ family: primaryFamily, weights: null });
      });
    return () => { active = false; };
  }, [primaryFamily]);

  const availableWeights = available?.family === primaryFamily ? available.weights : null;
  const options: Array<[string, string]> = WEIGHTS.map(([weight, key]) => [
    String(weight),
    `${weight} ${t(`settings.overlay.${key}`)}${availableWeights?.includes(weight) ? " ✓" : ""}`,
  ]);
  const description = showHint && availableWeights
    ? `${t("settings.overlay.fontWeightNativeHint")}${availableWeights.includes(value) ? "" : ` ${t("settings.overlay.fontWeightUnavailableHint")}`}`
    : undefined;

  return <SelectRow label={label} description={description} value={String(value)} options={options} onChange={(next) => onChange(Number(next) as OverlayFontWeight)} />;
}
