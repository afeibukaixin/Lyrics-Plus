export type UiUpdateState = {
  appVersion: string;
  coreApiVersion: number;
  source: "embedded" | "hot";
  activeVersion: string;
  preparedVersion: string | null;
  preparedReleaseNotes: string | null;
  pendingVersion: string | null;
  lastResult: string | null;
};
