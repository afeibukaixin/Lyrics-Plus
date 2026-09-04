import ConfigEditor from "../../../features/config/ConfigEditor";
import { useTranslation } from "react-i18next";
import { useSettingsContext } from "../shared/SettingsContext";
import { PageHeader } from "../shared/components";
import styles from "../settings.module.scss";

export default function ConfigSettingsPage() {
  const { t } = useTranslation();
  const { syncAppliedConfig, setError, setNotice } = useSettingsContext();
  return <main className={styles.configPage}><PageHeader title={t("settings.config.title")} description={t("settings.config.description")} /><ConfigEditor onApplied={syncAppliedConfig} setError={setError} setNotice={setNotice} /></main>;
}
