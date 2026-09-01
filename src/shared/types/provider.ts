export type LyricsSearchResult = {
  id: string;
  providerId: string;
  title: string;
  artist: string;
  album: string | null;
  durationMs: number | null;
  source: string;
  synced: boolean;
  hasTranslation: boolean;
  hasWordTiming: boolean;
  hasRomanization: boolean;
  score: number;
  lyrics: string;
};

export type SearchResponse = {
  autoApply: boolean;
  results: LyricsSearchResult[];
  providerStatuses: ProviderStatus[];
  error: string | null;
};

export type ProviderOrderMode = "smart" | "strict";

export type ProviderPreference = {
  id: string;
  enabled: boolean;
};

export type MatchWeights = {
  title: number;
  artist: number;
  album: number;
  duration: number;
};

export type ProviderSettings = {
  mode: ProviderOrderMode;
  providers: ProviderPreference[];
  autoApplyThreshold: number;
  autoSearchDebounceMs: number;
  preferCapabilities: boolean;
  capabilityPreferenceTolerance: number;
  matchWeights: MatchWeights;
  normalizeChinese: boolean;
  titleFilterKeywords: string[];
  amllBaseUrl: string;
};

export type ProviderCredentialView = {
  musixmatchConfigured: boolean;
  musixmatchTokenType: MusixmatchTokenType | null;
};

export type MusixmatchTokenType = "desktopUserToken" | "developerApiKey";

export type ProviderCredentialUpdate = {
  credentials: ProviderCredentialView;
  providerView: ProviderSettingsView;
};

export type ProviderHealth = "unknown" | "available" | "degraded" | "unavailable";

export type ProviderStatus = {
  providerId: string;
  name: string;
  health: ProviderHealth;
  message: string | null;
  checkedAtMs: number | null;
};

export type ProviderSettingsView = {
  settings: ProviderSettings;
  statuses: ProviderStatus[];
};
