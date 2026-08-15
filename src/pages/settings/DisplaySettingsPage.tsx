import { useTranslation } from "react-i18next";
import { messageOf } from "../../shared/api";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { PageHeader, SettingsSection, ToggleRow } from "./components";
import { Button } from "@/components/ui/button";

export default function DisplaySettingsPage() {
  const { t } = useTranslation();
  const {
    config,
    overlaySettings,
    resettingSection,
    confirmingReset,
    setError,
    setVisible,
    setLocked,
    setOverlayHideWhenNotPlaying,
    resetSection,
    resetOverlayBounds,
  } = useSettingsContext();

  return <>
    <PageHeader title={t("settings.display.title")} description={t("settings.display.description")} onReset={() => void resetSection("display")} resetting={resettingSection === "display"} confirming={confirmingReset === "display"} />
    <SettingsSection title={t("settings.overlay.state")}>
      <ToggleRow label={t("settings.overlay.show")} description={t("settings.overlay.showHint")} value={overlaySettings.visible} onChange={setVisible} />
      <ToggleRow label={t("settings.overlay.autoHide")} description={t("settings.overlay.autoHideHint")} value={config.overlay.hideWhenNotPlaying} onChange={(hidden) => setOverlayHideWhenNotPlaying(hidden).catch((value) => setError(messageOf(value)))} />
      <ToggleRow label={t("settings.overlay.lock")} description={t("settings.overlay.lockHint")} value={overlaySettings.locked} onChange={setLocked} />
      <div className={styles.buttonRow}><Button variant="secondary" size="sm" onClick={() => void resetOverlayBounds()}>{t("settings.overlay.resetPosition")}</Button></div>
    </SettingsSection>
    <SettingsSection title={t("settings.display.directControl")}>
      <p className={styles.cardHint}>{t("settings.display.directControlHint")}</p>
    </SettingsSection>
  </>;
}
