import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useAppConfig } from "../../../features/config/AppConfigProvider";
import { useUpdates } from "../../../features/update/UpdateProvider";
import { messageOf } from "../../../shared/api";
import { useSettingsContext } from "../shared/SettingsContext";
import styles from "../settings.module.scss";
import { PageHeader, SettingsSection, ToggleRow } from "../shared/components";
import appIcon from "../../../../src-tauri/icons/128x128.png";
import { Button } from "@/components/ui/button";

const links = [
  ["github", "https://github.com/afeibukaixin/Lyrics-Plus"],
  ["releases", "https://github.com/afeibukaixin/Lyrics-Plus/releases"],
  ["issues", "https://github.com/afeibukaixin/Lyrics-Plus/issues"],
] as const;

const REMOTE_CONFIG_URL = "https://raw.githubusercontent.com/afeibukaixin/Lyrics-Plus/main/remote-config.json";

type QqGroupConfig = {
  number: string;
  joinUrl: string;
};

function parseQqGroupConfig(value: unknown): QqGroupConfig | null {
  if (typeof value !== "object" || value === null || !("qqGroup" in value)) return null;
  const qqGroup = value.qqGroup;
  if (typeof qqGroup !== "object" || qqGroup === null) return null;
  if (!("number" in qqGroup) || !("joinUrl" in qqGroup)) return null;

  const number = qqGroup.number;
  const joinUrl = qqGroup.joinUrl;
  if (typeof number !== "string" || !/^\d+$/.test(number)) return null;
  if (typeof joinUrl !== "string") return null;

  try {
    const url = new URL(joinUrl);
    if (url.protocol !== "https:" || url.hostname !== "qm.qq.com") return null;
  } catch {
    return null;
  }

  return { number, joinUrl };
}

export default function AboutSettingsPage() {
  const { config, setAutoCheckUpdates } = useAppConfig();
  const { setError, resettingSection, confirmingReset, resetSection } = useSettingsContext();
  const { availableVersion, checkForUpdates, currentVersion, error, restartToUpdate, status, updateKind } = useUpdates();
  const { t } = useTranslation();
  const busy = status === "checking" || status === "downloading" || status === "installing";
  const [qqGroup, setQqGroup] = useState<QqGroupConfig | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    let active = true;

    void fetch(REMOTE_CONFIG_URL, { cache: "no-store", signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) return null;
        return parseQqGroupConfig(await response.json() as unknown);
      })
      .then((value) => {
        if (active) setQqGroup(value);
      })
      .catch(() => {
        if (active) setQqGroup(null);
      });

    return () => {
      active = false;
      controller.abort();
    };
  }, []);

  const open = (url: string) => {
    void openUrl(url).catch((value) => setError(messageOf(value)));
  };

  return (
    <>
      <PageHeader title={t("settings.about.title")} description={t("settings.about.description")} onReset={() => void resetSection("about")} resetting={resettingSection === "about"} confirming={confirmingReset === "about"} />
      <SettingsSection id="about-updates">
        <div className={styles.aboutHero}>
          <img alt="" src={appIcon} />
          <div><strong>Lyrics Plus</strong><span>{t("settings.about.version", { version: currentVersion })}</span></div>
        </div>
        <ToggleRow
          label={t("settings.about.autoCheck")}
          description={t("settings.about.autoCheckHint")}
          value={config.app.autoCheckUpdates}
          onChange={(enabled) => setAutoCheckUpdates(enabled).catch((value) => setError(messageOf(value)))}
        />
        <div className={styles.buttonRow}>
          {status === "ready" ? (
            <Button size="sm" onClick={() => void restartToUpdate()}>{t(updateKind === "interface" ? "settings.about.refreshNow" : "settings.about.restartNow")}</Button>
          ) : (
            <Button variant="secondary" size="sm" disabled={busy} onClick={() => void checkForUpdates()}>
              {status === "checking" ? t("settings.about.checking") : t("settings.about.checkNow")}
            </Button>
          )}
        </div>
        {status !== "idle" && (
          <p className={styles.cardHint} data-error={Boolean(error) || status === "error"} role={error || status === "error" ? "alert" : undefined}>
            {error ?? (status === "ready" && updateKind === "interface"
              ? t("settings.about.interfaceReadyHint", { version: availableVersion ?? "" })
              : t(`settings.about.status.${status}`, { version: availableVersion ?? "" }))}
          </p>
        )}
      </SettingsSection>
      <SettingsSection id="about-project" title={t("settings.about.project")}>
        <div className={styles.buttonRow}>
          {links.map(([key, url]) => <Button variant="outline" size="sm" key={key} onClick={() => open(url)}>{t(`settings.about.links.${key}`)}</Button>)}
        </div>
        <p className={styles.cardHint}>{t("settings.about.licenseHint")}</p>
      </SettingsSection>
      {qqGroup && (
        <SettingsSection id="about-community" title={t("settings.about.community")}>
          <p className={styles.cardHint}>{t("settings.about.qqGroup", { number: qqGroup.number })}</p>
          <div className={styles.buttonRow}>
            <Button variant="outline" size="sm" onClick={() => open(qqGroup.joinUrl)}>{t("settings.about.joinQqGroup")}</Button>
          </div>
        </SettingsSection>
      )}
    </>
  );
}
