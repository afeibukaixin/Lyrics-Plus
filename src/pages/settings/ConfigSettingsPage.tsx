import ConfigEditor from "../../features/config/ConfigEditor";
import { useTranslation } from "react-i18next";
import { useSettingsContext } from "../settings";
import { PageHeader } from "./components";

export default function ConfigSettingsPage() {
  const { t } = useTranslation();
  const { syncAppliedConfig, setError, setNotice } = useSettingsContext();
  return <><PageHeader title={t("settings.config.title")} description={t("settings.config.description")} /><ConfigEditor onApplied={syncAppliedConfig} setError={setError} setNotice={setNotice} /></>;
}
