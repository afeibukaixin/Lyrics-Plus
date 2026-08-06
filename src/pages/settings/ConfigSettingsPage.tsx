import ConfigEditor from "../../features/config/ConfigEditor";
import { useSettingsContext } from "../settings";

export default function ConfigSettingsPage() {
  const { syncAppliedConfig, setError, setNotice } = useSettingsContext();
  return <ConfigEditor onApplied={syncAppliedConfig} setError={setError} setNotice={setNotice} />;
}
