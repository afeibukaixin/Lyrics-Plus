import { invoke } from "./core";
import type { UiUpdateState } from "../types/update";

export const updateApi = {
  getUiUpdateState: () => invoke<UiUpdateState>("get_ui_update_state"),
  checkAndPrepareUiUpdate: () => invoke<UiUpdateState>("check_and_prepare_ui_update"),
  applyPreparedUiUpdate: () => invoke<void>("apply_prepared_ui_update"),
  reportUiReady: (uiVersion: string) => invoke<UiUpdateState>("report_ui_ready", { uiVersion }),
};
