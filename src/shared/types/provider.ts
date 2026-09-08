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
  autoApplyCandidate: {
    providerId: string;
    id: string;
  } | null;
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
  version: number;
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

export type ProviderErrorKind = "network" | "http" | "invalid_response" | "configuration" | "unauthorized";

export type ProviderStatusDetail =
  | { kind: "not_tested" }
  | { kind: "not_participated" }
  | { kind: "success"; resultCount: number }
  | { kind: "partial_failure"; resultCount: number; errorKind: ProviderErrorKind }
  | { kind: "failure"; errorKind: ProviderErrorKind; statusCode: number | null }
  | { kind: "timeout" }
  | { kind: "cooldown"; retryAfterMs: number | null; requiresConfiguration: boolean };

export type ProviderStatus = {
  providerId: string;
  name: string;
  health: ProviderHealth;
  detail: ProviderStatusDetail;
  checkedAtMs: number | null;
};

export type ProviderTier = "stable" | "exact_id" | "advanced" | "experimental" | "dormant";

export type ProviderAutoBindingPolicy = "allowed" | "exact_id_only" | "user_only" | "disabled";

export type ProviderPersistencePolicy = "download_library" | "cache_only" | "memory_only";

export type ProviderCapabilities = {
  metadataSearch: boolean;
  idLookup: boolean;
  plainText: boolean;
  lineTiming: boolean;
  wordTiming: boolean;
  translation: boolean;
  romanization: boolean;
};

export type ProviderCandidate = {
  providerId: string;
  providerItemId: string;
  title: string;
  artists: string[];
  album: string | null;
  durationMs: number | null;
  versionTags: string[];
  capabilities: ProviderCapabilities;
  source: string;
  lookupKey: string | null;
};

export type ProviderManifest = {
  id: string;
  displayName: string;
  implementationKey: string;
  tier: ProviderTier;
  enabledByDefault: boolean;
  autoBinding: ProviderAutoBindingPolicy;
  capabilities: ProviderCapabilities;
  requiresCredentials: boolean;
  persistence: ProviderPersistencePolicy;
  requestTimeoutMs: number;
  noticeKey: string | null;
};

export type ProviderDescriptor = {
  manifest: ProviderManifest;
  enabled: boolean;
  status: ProviderStatus | null;
};

export type ProviderSettingsView = {
  settings: ProviderSettings;
  statuses: ProviderStatus[];
  manifests: ProviderManifest[];
};
