import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Field, FieldContent, FieldTitle } from "@/components/ui/field";
import { api, messageOf } from "../../../shared/api";
import type { FontFamily, OverlayFontWeight } from "../../../shared/types";
import {
  defaultFontFamilies,
  fontFamiliesFromCss,
  fontFallbacksFromCss,
  fontFallbacksToCss,
  isSystemFontFamily,
  normalizeFontFamilies,
} from "../../../shared/fontFamily";
import { createTauriListenerCleanup, isMacTauriRuntime } from "../../../shared/tauriEvent";
import styles from "../settings.module.scss";
import { TextRow } from "../shared/components";

type Props = {
  fontFamily: string;
  fontFamilies: string;
  fontWeight?: OverlayFontWeight;
  disabled?: boolean;
  onChange: (fontFamily: string, fontFamilies: string, fontWeight?: OverlayFontWeight) => Promise<unknown>;
  onError: (message: string) => void;
};

type FontSelection = { primary: FontFamily; fallbacks: string; fontWeight?: OverlayFontWeight };
type FontPanelSelection = FontFamily & { fontWeight: OverlayFontWeight | null };

const FONT_PANEL_SELECTED_EVENT = "font://selected";

export default function FontFamilyChain({
  fontFamily,
  fontFamilies,
  fontWeight,
  disabled = false,
  onChange,
  onError,
}: Props) {
  const { t } = useTranslation();
  const parsedFamilies = fontFamiliesFromCss(fontFamily);
  const primary = parsedFamilies[0] ?? defaultFontFamilies[0];
  const [previewSelection, setPreviewSelection] = useState<FontSelection | null>(null);
  const shownPrimary = previewSelection?.primary ?? primary;
  const fallbackValue = previewSelection?.fallbacks ?? fontFamilies;
  const latestRef = useRef({ onChange, onError, fontWeight });
  latestRef.current = { onChange, onError, fontWeight };
  // Keep the latest uncommitted fallback chain available to font panel events.
  const fallbackValueRef = useRef(fallbackValue);
  fallbackValueRef.current = fallbackValue;
  const pendingRef = useRef<FontSelection | null>(null);
  const savingRef = useRef(false);
  const mountedRef = useRef(true);

  useEffect(() => {
    if (previewSelection?.primary.family === primary.family && previewSelection.fallbacks === fontFamilies) {
      setPreviewSelection(null);
    }
  }, [previewSelection, primary.family, fontFamilies]);

  const saveLatest = async () => {
    if (savingRef.current) return;
    savingRef.current = true;
    try {
      while (pendingRef.current) {
        const selection = pendingRef.current;
        pendingRef.current = null;
        try {
          const saved = await latestRef.current.onChange(selection.primary.family, selection.fallbacks, selection.fontWeight);
          if (saved === false && !pendingRef.current && mountedRef.current) {
            setPreviewSelection(null);
          }
        } catch (error) {
          latestRef.current.onError(messageOf(error));
          if (!pendingRef.current && mountedRef.current) setPreviewSelection(null);
        }
      }
    } finally {
      savingRef.current = false;
    }
  };

  const changeFonts = (nextPrimary: FontFamily, nextFallbacks: string, nextWeight?: OverlayFontWeight) => {
    const [normalizedPrimary] = normalizeFontFamilies([nextPrimary]);
    fallbackValueRef.current = nextFallbacks;
    const selection = { primary: normalizedPrimary, fallbacks: nextFallbacks, fontWeight: nextWeight };
    pendingRef.current = selection;
    setPreviewSelection(selection);
    void saveLatest();
  };

  const changeFontsRef = useRef(changeFonts);
  changeFontsRef.current = changeFonts;

  useEffect(() => {
    if (!isMacTauriRuntime()) return;
    return createTauriListenerCleanup(
      listen<FontPanelSelection>(FONT_PANEL_SELECTED_EVENT, ({ payload }) => {
        if (!mountedRef.current) return;
        if (payload.fontWeight != null && latestRef.current.fontWeight == null) return;
        changeFontsRef.current(payload, fallbackValueRef.current, payload.fontWeight ?? undefined);
      }),
    );
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    if (!isMacTauriRuntime()) return;
    return () => {
      mountedRef.current = false;
      void api.closeFontPanel().catch(() => undefined);
    };
  }, []);

  const openPrimaryPicker = () => {
    void api.openFontPanel(shownPrimary.family, fontWeight).catch((error) => onError(messageOf(error)));
  };

  const chooseSystemDefault = () => {
    changeFonts(defaultFontFamilies[0], fallbackValueRef.current);
  };

  return (
    <div className={styles.fontFamilySetting} data-disabled={disabled || undefined}>
      <Field className={styles.fontFamilySettingRow} orientation="horizontal">
        <FieldContent>
          <FieldTitle>{t("settings.overlay.fontFamilyMain")}</FieldTitle>
        </FieldContent>
        <div className={styles.fontFamilyPrimaryActions}>
          <Button type="button" variant="outline" disabled={disabled || !isMacTauriRuntime()} className={styles.fontFamilyValue} onClick={openPrimaryPicker}>
            <span>{isSystemFontFamily(shownPrimary.family) ? t("settings.overlay.fontFamilySystemDefault") : shownPrimary.name}</span>
            <ChevronRight data-icon="inline-end" aria-hidden="true" />
          </Button>
          <Button type="button" variant="ghost" size="sm" disabled={disabled} onClick={chooseSystemDefault}>
            {t("settings.overlay.fontFamilySystemDefault")}
          </Button>
        </div>
      </Field>
      <TextRow
        label={t("settings.overlay.fontFamilyCustom")}
        description={t("settings.overlay.fontFamilyCustomHint")}
        value={fallbackValue}
        emptyValue=""
        disabled={disabled}
        onChange={(value) => {
          // Normalize the fallback list on its own so matching the primary font does not clear it.
          const [, ...normalizedFallbacks] = normalizeFontFamilies([
            defaultFontFamilies[0],
            ...fontFallbacksFromCss(value),
          ]);
          changeFonts(shownPrimary, fontFallbacksToCss(normalizedFallbacks));
        }}
      />
    </div>
  );
}
