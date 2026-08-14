import { Link } from "react-router";
import { useTranslation } from "react-i18next";
import { messageOf } from "../../shared/api";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { SettingsCard, SettingsHeading, ToggleRow } from "./components";

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
    <SettingsHeading title={t("settings.display.title")} description={t("settings.display.description")} onReset={() => void resetSection("display")} resetting={resettingSection === "display"} confirming={confirmingReset === "display"} />
    <SettingsCard title={t("settings.overlay.state")}>
      <ToggleRow label={t("settings.overlay.show")} description={t("settings.overlay.showHint")} value={overlaySettings.visible} onChange={setVisible} />
      <ToggleRow label={t("settings.overlay.autoHide")} description={t("settings.overlay.autoHideHint")} value={config.overlay.hideWhenNotPlaying} onChange={(hidden) => setOverlayHideWhenNotPlaying(hidden).catch((value) => setError(messageOf(value)))} />
      <ToggleRow label={t("settings.overlay.lock")} description={t("settings.overlay.lockHint")} value={overlaySettings.locked} onChange={setLocked} />
      <div className={styles.buttonRow}><button onClick={() => void resetOverlayBounds()}>{t("settings.overlay.resetPosition")}</button></div>
    </SettingsCard>
    <SettingsCard title={t("settings.display.directControl")}>
      <p className={styles.cardHint}>{t("settings.display.directControlHint")}</p>
    </SettingsCard>
    <SettingsCard title={t("settings.app.shortcuts")}>
      <div className={styles.buttonRow}><Link className={styles.buttonLink} to="/settings/player">{t("settings.display.manageShortcuts")}</Link></div>
    </SettingsCard>
  </>;
}
