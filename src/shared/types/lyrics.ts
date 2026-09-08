import type { OverlayFontWeight } from "./base";
import type { OverlayStyle } from "./overlay";

export type LyricsLine = {
  startMs: number;
  endMs: number | null;
  text: string;
  words: LyricsWord[] | null;
};

export type LyricsWord = {
  startMs: number;
  endMs: number;
  text: string;
};

export type LyricsTrack = {
  lines: LyricsLine[];
};

export type LyricsDocument = {
  metadata: {
    title: string | null;
    artist: string | null;
    album: string | null;
    source: string;
    originalFormat: string;
    manualSelected: boolean;
  };
  tracks: {
    original: LyricsTrack;
    translation: LyricsTrack | null;
    romanization: LyricsTrack | null;
  };
  offsetMs: number;
  raw: string;
};

export type RecordingView = {
  recordingId: number;
  title: string;
  album: string | null;
  durationMs: number | null;
  versionTags: string[];
};

export type ExternalIdentifier = {
  externalId: number;
  namespace: string;
  idKind: string;
  value: string;
  confidence: number;
  confirmed: boolean;
};

export type ArtistCredit = {
  artistId: number | null;
  rawName: string;
  canonicalName: string;
  creditOrder: number;
  role: string;
  confirmedAliases: string[];
};

export type TrackObservation = {
  observationId: number;
  trackKey: string;
  platform: string;
  rawTitle: string;
  rawArtists: string[];
  rawAlbum: string | null;
  durationMs: number | null;
  observedAt: number;
  recordingId: number;
  splitFromRecordingId: number | null;
};

export type SongAssociationCandidate = {
  kind: "metadata" | "sharedIdentifier" | "originalRelation";
  gates: {
    title: boolean; artist: boolean; duration: boolean; version: boolean;
    currentObservationId: number; candidateObservationId: number;
  };
  scoreWeights: { title: number; artist: number; album: number; duration: number; version: number };
  conflicts: ("title" | "artist" | "duration" | "version" | "platform")[];
  warnings: ("durationMissing" | "versionMissing")[];
  canAssociate: boolean;
  lyricsContentSame: boolean;
  recordingId: number;
  title: string;
  album: string | null;
  durationMs: number | null;
  versionTags: string[];
  observations: TrackObservation[];
  externalIdentifiers: ExternalIdentifier[];
  isOriginalRecording: boolean;
  titleSimilarity: number;
  artistSimilarity: number;
  albumSimilarity: number;
  durationDeltaMs: number | null;
  lyricsSimilarity: number | null;
  isrc: string | null;
  sharedIdentifier: ExternalIdentifier | null;
  confirmedArtistAliases: string[];
  score: number;
  matchReasons: string[];
};

export type RecordingIdentity = {
  recordingId: number;
  title: string;
  album: string | null;
  durationMs: number | null;
  versionTags: string[];
  artistCredits: ArtistCredit[];
  externalIdentifiers: ExternalIdentifier[];
  observations: TrackObservation[];
};

export type LyricAsset = {
  assetId: number;
  sourceKind: "managed" | "local" | "legacy" | "cache";
  sourceName: string;
  providerId: string | null;
  providerItemId: string | null;
  originalFormat: string;
  language: string;
  hasWordTiming: boolean;
  hasTranslation: boolean;
  hasRomanization: boolean;
  contentFingerprint: string;
  rootId: string | null;
  rootName: string | null;
  relativePath: string | null;
  available: boolean;
};

export type LyricsBinding = {
  bindingId: number;
  recordingId: number;
  assetId: number;
  selectionSource: string;
  confidence: number;
  evidence: Record<string, unknown>;
  isDefault: boolean;
  offsetMs: number;
  asset: LyricAsset | null;
};

export type PlatformLyricOverride = {
  overrideId: number;
  recordingId: number;
  platform: string;
  assetId: number | null;
  offsetMs: number;
  asset: LyricAsset | null;
};

export type LyricsContext = {
  trackKey: string;
  platform: string;
  observation: TrackObservation;
  recording: RecordingIdentity;
  bindings: LyricsBinding[];
  platformOverride: PlatformLyricOverride | null;
  currentAsset: LyricAsset | null;
  currentBindingId: number | null;
};

export type LibraryRootView = {
  rootId: string;
  rootKind: "managed" | "local" | "legacy" | "cache";
  displayName: string;
  path: string;
  readOnly: boolean;
  enabled: boolean;
  fileCount: number;
  unavailableCount: number;
  lastScanAt: number | null;
  lastError: string | null;
};

export type LyricsRuntimeStatus = "idle" | "loading" | "ready" | "not_found" | "error";

export type LyricsLoadStatus = "ready" | "missing" | "error";

export type LyricsLoadResponse = {
  status: LyricsLoadStatus;
  document: LyricsDocument | null;
  error: string | null;
};

export type LyricsRuntimeSnapshot = {
  trackKey: string | null;
  document: LyricsDocument | null;
  status: LyricsRuntimeStatus;
  error: string | null;
};

export type LyricsSearchProgress = {
  runId: string;
  stage: string;
  providerId: string | null;
  status: string;
  elapsedMs: number;
  totalElapsedMs: number;
  candidateCount: number;
};

export type LyricsSearchTraceCandidate = {
  providerId: string;
  providerItemId: string;
  title: string;
  artists: string[];
  album: string | null;
  durationMs: number | null;
  versionTags: string[];
  score: number;
  state: string;
  rejectionReason: string | null;
  selectionReason: string | null;
  scoreEvidence: LyricsScoreEvidence | null;
};

export type LyricsScoreEvidence = {
  titleSimilarity: number;
  artistSimilarity: number;
  artistFeaturedComplete: boolean;
  artistMainConflict: boolean;
  spotifyArtistSubset: boolean;
  albumSimilarity: number;
  durationSimilarity: number;
  durationDeltaMs: number | null;
  versionSimilarity: number;
  versionConflict: boolean;
  synced: boolean;
  wordTiming: boolean;
  translation: boolean;
  romanization: boolean;
};

export type LyricsSearchTraceProvider = {
  providerId: string;
  status: string;
  candidateCount: number;
  elapsedMs: number;
};

export type LyricsSearchTraceStage = {
  stage: string;
  providerId: string | null;
  status: string;
  startedAt: number;
  finishedAt: number | null;
  elapsedMs: number;
  candidateCount: number;
};

export type LyricsSearchTrace = {
  runId: string;
  recordingId: number | null;
  intent: string;
  status: string;
  title: string;
  artist: string;
  startedAt: number;
  finishedAt: number | null;
  totalElapsedMs: number | null;
  errorCode: string | null;
  selectedProviderId: string | null;
  selectedProviderItemId: string | null;
  selectionReason: string | null;
  providers: LyricsSearchTraceProvider[];
  stages: LyricsSearchTraceStage[];
  candidates: LyricsSearchTraceCandidate[];
};

export type NotchLayoutMetrics = {
  hasNotch: boolean;
  topInset: number;
  centerGapWidth: number;
};

export type LyricsStyleMode = "desktop" | "statusBar" | "listWindow" | "notch";

export type CompactKaraokeStyle = "sweep" | "highlight";

export type StatusBarAlignment = "left" | "center" | "right";

export type ListLyricsLineKind = "original" | "translation" | "romanization";

export type ListLyricsLineOrder = [
  ListLyricsLineKind,
  ListLyricsLineKind,
  ListLyricsLineKind,
];

export type SupportingLyricsPriority = "translation" | "romanization";

export type CompactLyricsPresentation = {
  layout: "single" | "double";
  doubleLineMode: "rolling" | "alternating";
  showTranslation: boolean;
  showRomanization: boolean;
  supportingPriority: SupportingLyricsPriority;
};

export type DesktopLyricsPresentation = CompactLyricsPresentation & {
  orientation: OverlayStyle["orientation"];
  alignment: OverlayStyle["alignment"];
  primaryLinePosition: OverlayStyle["primaryLinePosition"];
  longText: OverlayStyle["longText"];
  autoCenterWithTranslationOrRomanization: boolean;
};

export type DesktopLyricsAppearance = Omit<
  OverlayStyle,
  | "layout"
  | "doubleLineMode"
  | "orientation"
  | "alignment"
  | "primaryLinePosition"
  | "longText"
  | "secondaryDisplay"
  | "autoCenterWithTranslationOrRomanization"
  | "horizontalMaxWidth"
  | "verticalMaxHeight"
>;

export type NotchSlotContent = "empty" | "title" | "artist" | "artwork" | "spectrum";

export type LyricsBaseAppearance = {
  fontFamily: string;
  activeColor: string;
  inactiveColor: string;
  translationColor: string;
  romanizationColor: string;
  supportingColor: string;
  backgroundColor: string;
};

export type LyricsModeStyleInheritance = {
  inheritFontFamily: boolean;
  inheritColors: boolean;
};

export type LyricsStyleInheritance = Record<LyricsStyleMode, LyricsModeStyleInheritance>;

export type StatusBarLyricsAppearance = {
  fontFamily: string;
  fontSize: number;
  verticalOffset: number;
  fontWeight: OverlayFontWeight;
  secondaryFontWeight: OverlayFontWeight;
  textColor: string;
  inactiveColor: string;
  highlightColor: string;
  translationColor: string;
  romanizationColor: string;
  karaokeStyle: CompactKaraokeStyle;
  width: number;
};

export type ListLyricsAppearance = {
  fontFamily: string;
  fontSize: number;
  fontWeight: OverlayFontWeight;
  secondaryFontScale: number;
  lineHeight: number;
  lineGap: number;
  secondaryLineGap: number;
  activeColor: string;
  inactiveColor: string;
  activeOpacity: number;
  inactiveOpacity: number;
  translationColor: string;
  romanizationColor: string;
  activeBackgroundColor: string;
  backgroundColor: string;
  backgroundOpacity: number;
  backgroundMode: "solid" | "transparent";
  textShadowOffsetX: number;
  textShadowOffsetY: number;
  textShadowBlur: number;
  textShadowColor: string;
  textStrokeWidth: number;
  textStrokeColor: string;
  alignment: "left" | "center" | "right";
};

export type NotchLyricsAppearance = {
  fontFamily: string;
  fontSize: number;
  fontWeight: OverlayFontWeight;
  secondaryFontWeight: OverlayFontWeight;
  activeColor: string;
  inactiveColor: string;
  translationColor: string;
  romanizationColor: string;
  karaokeStyle: CompactKaraokeStyle;
  lineGap: number;
  borderRadius: number;
  expandedBorderRadius: number;
  topBorderRadius: number;
  maxWidth: number;
  expandedMaxWidth: number;
};

export type LyricsMonitor = {
  id: string;
  name: string;
  width: number;
  height: number;
  isPrimary: boolean;
};

export type LyricsDisplayPreferences = {
  desktop: {
    enabled: boolean;
    locked: boolean;
    hideWhenNotPlaying: boolean;
    presentation: DesktopLyricsPresentation;
    appearance: DesktopLyricsAppearance;
  };
  statusBar: {
    enabled: boolean;
    hideWhenNotPlaying: boolean;
    presentation: CompactLyricsPresentation & { alignment: StatusBarAlignment };
    appearance: StatusBarLyricsAppearance;
  };
  listWindow: {
    enabled: boolean;
    alwaysOnTop: boolean;
    locked: boolean;
    showTranslation: boolean;
    showRomanization: boolean;
    lineOrder: ListLyricsLineOrder;
    appearance: ListLyricsAppearance;
  };
  notch: {
    enabled: boolean;
    hideWhenNotPlaying: boolean;
    monitorId: string | null;
    showLyrics: boolean;
    leftSlot: NotchSlotContent;
    rightSlot: NotchSlotContent;
    presentation: CompactLyricsPresentation;
    inlineLyricsOnNonNotch: boolean;
    appearance: NotchLyricsAppearance;
  };
};

export type DesktopLyricsPreferences = LyricsDisplayPreferences["desktop"];
export type StatusBarLyricsPreferences = LyricsDisplayPreferences["statusBar"];
export type ListLyricsPreferences = LyricsDisplayPreferences["listWindow"];
export type NotchLyricsPreferences = LyricsDisplayPreferences["notch"];
