import { openUrl } from "@tauri-apps/plugin-opener";
import { useTranslation } from "react-i18next";
import { useAppConfig } from "../../features/config/AppConfigProvider";
import { useUpdates } from "../../features/update/UpdateProvider";
import { messageOf } from "../../shared/api";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { SettingsCard, SettingsHeading, ToggleRow } from "./components";
import appIcon from "../../../src-tauri/icons/128x128.png";

const links = [
  ["github", "https://github.com/afeibukaixin/Lyrics-Plus"],
  ["releases", "https://github.com/afeibukaixin/Lyrics-Plus/releases"],
  ["issues", "https://github.com/afeibukaixin/Lyrics-Plus/issues"],
] as const;

export default function AboutSettingsPage() {
  const { config, setAutoCheckUpdates } = useAppConfig();
  const { setError, resettingSection, confirmingReset, resetSection } = useSettingsContext();
  const { availableVersion, checkForUpdates, currentVersion, error, restartToUpdate, status } = useUpdates();
  const { t } = useTranslation();
  const busy = status === "checking" || status === "downloading" || status === "installing";

  const open = (url: string) => {
    void openUrl(url).catch((value) => setError(messageOf(value)));
  };

  return (
    <>
      <SettingsHeading title={t("settings.about.title")} description={t("settings.about.description")} onReset={() => void resetSection("about")} resetting={resettingSection === "about"} confirming={confirmingReset === "about"} />
      <SettingsCard title={t("settings.about.appInfo")}>
        <div className={styles.aboutHero}>
          <img alt="" src={appIcon} />
          <div><strong>Lyrics Plus</strong><span>{t("settings.about.version", { version: currentVersion })}</span></div>
        </div>
      </SettingsCard>
      <SettingsCard title={t("settings.about.updates")}>
        <ToggleRow
          label={t("settings.about.autoCheck")}
          description={t("settings.about.autoCheckHint")}
          value={config.app.autoCheckUpdates}
          onChange={(enabled) => setAutoCheckUpdates(enabled).catch((value) => setError(messageOf(value)))}
        />
        <div className={styles.buttonRow}>
          {status === "ready" ? (
            <button onClick={() => void restartToUpdate()}>{t("settings.about.restartNow")}</button>
          ) : (
            <button disabled={busy} onClick={() => void checkForUpdates()}>
              {status === "checking" ? t("settings.about.checking") : t("settings.about.checkNow")}
            </button>
          )}
        </div>
        {status !== "idle" && (
          <p className={styles.cardHint} data-error={Boolean(error) || status === "error"} role={error || status === "error" ? "alert" : undefined}>
            {error ?? t(`settings.about.status.${status}`, { version: availableVersion ?? "" })}
          </p>
        )}
      </SettingsCard>
      <SettingsCard title={t("settings.about.project")}>
        <p className={styles.cardHint}>{t("settings.about.projectHint")}</p>
        <div className={styles.buttonRow}>
          {links.map(([key, url]) => <button key={key} onClick={() => open(url)}>{t(`settings.about.links.${key}`)}</button>)}
        </div>
      </SettingsCard>
      <SettingsCard title={t("settings.about.license")}>
        <p className={styles.cardHint}>{t("settings.about.licenseHint")}</p>
      </SettingsCard>
    </>
  );
}
