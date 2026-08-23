import { invoke } from "./core";
import type { LegalNoticeStatus } from "./types";

export const legalApi = {
  getLegalNoticeStatus: () => invoke<LegalNoticeStatus>("get_legal_notice_status"),
  acceptLegalNotice: () => invoke<void>("accept_legal_notice"),
  quitApplication: () => invoke<void>("quit_application"),
};
